> **Status:** reference — independent code review, 2026-09-24.
# Code review — DFS S3 piece C: recovery snapshot store (`feat/dfs-s3-c-recovery`, `c4d7343...1f50650`)

**Verdict: REQUEST CHANGES (one P1, a small fix).**
1. The write order is right. The blob is published before the manifest, and N-1 is deleted only after N's manifest is confirmed durable. The one `session.lock` is held from the first write until retire. Retire renames the folder to a tombstone, and there is no blob-scan fallback. All 17 tests the WO names are present. Review findings 3–5 are followed.
2. **P1:** `scan` follows a symlink out of `Recovery/` and deletes the target folder's files recursively (reproduced below). This breaks the module's "only under `Recovery/`" law.
3. P2: `scan`'s no-manifest cleanup lets go of the lock before unlinking `session.lock`, and it also deletes folders it can't understand. Two key invariants have no test that catches a mutation.
4. The size is fair: one file, no new dependency, bytes only. The waste is small: an over-wide `pub` surface and a full JSON parse that is labelled a "head" check.

## Gates (measured by this reviewer in the worktree, after `touch`ing all varos `.rs`, `CARGO_TARGET_DIR=target-s3`)
- `cargo test --workspace -j 4`: **432 passed, 0 failed** (reproduced). `storage::recovery`: **22 passed**.
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: clean. `cargo fmt --all -- --check`: exit 0.
- `cargo clippy -p varos-app --all-targets --target x86_64-pc-windows-msvc` and `--target aarch64-apple-darwin` with `-D warnings`: both clean.
- Probes (run in a scratch copy with extra tests, not committed): see P1 and P2-2. Mutation runs: see P2-3.
- Scan cost (release build, warm cache, 32 MiB doc-shaped JSON): read 17 ms + CRC-32 95 ms + `from_slice::<Head>` 45 ms = **about 155 ms per orphan**, about 310 ms when the newest copy is damaged. A debug build takes about 900 ms. A 1 MB document takes under 5 ms.

## Findings

**P1-1 — `scan`/`cleanup_completed`/`remove_tree` follow symlinks, so they can delete user files outside `Recovery/`.**
`recovery.rs:458` accepts any entry that has a valid rid and where `fs.metadata(..).is_dir` is true. `metadata` follows symlinks (`durable.rs:175`). `try_take_lock` then creates `session.lock` in the symlink's target. With no `manifest.json` there, `remove_tree` (`:559-569`) lists the target, removes every file, and recurses into its subfolders (`:562`, `metadata` again follows links).
Probe: `Recovery/abc123 -> <tmp>/UserDocs` holding `precious.vrs` and `sub/more.vrs`. After `store.scan()` **both files are gone** (`outside now=[] sub=[]`).
`cleanup_completed` does the same with a `<rid>.discarded-x` symlink (`:548-552`). `never_writes_outside_recovery_dir` misses all of this because `FaultFs` only records lexical paths, and `Recovery/abc123/...` looks like it is "inside".
Fix: (a) in `scan` and `cleanup_completed`, skip an entry unless `fs.resolve_link(&p)? == p` (the frozen port already has `resolve_link`). (b) Make `remove_tree` flat: session folders never contain folders. Remove only the entries that are not folders and are not links, then `remove_dir`, and never recurse. Add a `#[cfg(unix)]` test `symlinked_rid_is_never_followed` that checks the outside files still exist.

**P2-1 — `scan`'s no-manifest cleanup releases the claim before it unlinks `session.lock` (`:477-478`).**
Once `session.lock` is unlinked, a lock held on its old inode no longer protects the folder.
Interleaving:
- B's scan try-locks a brand-new folder of A, in the gap between A's `create_dir_all` and A's `try_take_lock` (`:284-285`).
- B reads no manifest and drops its lock.
- A's `try_lock` now succeeds on that same inode.
- B's `remove_tree` unlinks `session.lock`, and possibly A's temp file or blob as well.
- A goes on writing while it holds a lock on an inode that has no name. A later scan by a third process C creates a fresh `session.lock` and locks it. C then treats A's **live** session as an orphan, and C's Discard deletes it.
The windows are microseconds wide, but the result is silent loss of a live session's recovery copies. Fix: remove the folder the way retire does. Rename `<rid>` to `<rid>.discarded-<nonce>` **while still holding the claim**, then remove the tombstone. This is one call (`let _ = self.retire(&rid);`) and keeps a single removal path.

**P2-2 — The no-manifest cleanup deletes content it doesn't understand, which breaks "newer-format folders are kept" (module doc `:16`).**
Probe: `Recovery/future-session/{manifest-v2.json, snap-9.json}` was removed entirely by one `scan()`. A later Varos that changes the layout, a user who opens an older build, or a manifest deleted by hand all lose data silently.
Fix: discard only when every entry is a name this build writes (`session.lock`, `snap-<n>.json`, `.*.varos-tmp`). Otherwise list the folder as `Damaged("The recovery record can't be read.")` and keep it. Add a test for this.

**P2-3 — Two data-safety invariants are untested; the mutations survive (all 24 tests stay green).**
(a) `:371`: changing `if matches!(outcome, WriteOutcome::Durable)` to `if true` leaves every test green. The "N-1 deleted only after N is durable" promise is not proven. Test: push two `Fault::at(Step::SyncDir).on(&rid)` faults (one fires on the blob's dir sync, one on the manifest's). Write gen 3 over [2,1]. Assert `snap-1.json` still exists. Then do a clean write and assert it is swept.
(b) `:332`: changing `seq <= newest` to `seq < newest` leaves every test green. A repeated seq would then rename over a **listed** blob. Test: write seq 2, then call `write_generation(.., 2, ..)` again. Assert that the returned seq is 3 and that `snap-2.json`'s bytes are unchanged.
(Also surviving, lower value: removing the temp-file branch of `sweep` at `:388`, and removing the head parse at `:415` — see P3-1.)

**P2-4 — Scan cost belongs to F2's plan, not to a new `Unverified` state.**
`scan` costs O(total orphan bytes): about 155 ms per 32 MiB orphan and 310 ms when it falls back (measured above), and each `scan()` call repeats it. `load_best` then reads the same bytes again at Recover. At typical sizes the cost is negligible, and an `Unverified` state would add a row state that E1/F2 must draw for no user benefit. Instead, amend WO §3.8 so F2 runs `cleanup_completed` + `scan` as one `IoWorker` job at launch. Start draws immediately, and the Recovery rows arrive with the completion. Add a note to `scan`'s doc: "reads every orphan's blob; call off the UI thread". C's API needs no change.

**P3-1 — The "head" check parses the whole blob.** `serde_json::from_slice::<Head>` (`:415`) must parse the entire 32 MiB value (about 45 ms), and it proves nothing the size+CRC match hasn't already proved. The CRC was taken over the `doc_to_blob` bytes. Recover's `doc_from_blob` does the real decode, which S5 hardens in place (S5 WO:184), so recovery does not bypass that pipeline (question 4: OK). Replace the check with `blob.starts_with(b"{\"varos\":")`, or drop it and update the WO line.
**P3-2 — The `pub` surface is wider than D/F1/F2 need.** `Manifest`, `MANIFEST_VERSION`, `MANIFEST_FILE`, `LOCK_FILE`, `DISCARDED_MARK`, `GENERATIONS_KEPT` and `valid_rid` have no consumer outside the module (grep). Make them private, since the tests are in-module.
**P3-3 — Some manifest fields are written but can't be read back.** `untitled_number`, `source_fingerprint`, `recovered` and `created` are stored (WO-mandated), but `OrphanEntry`/`Loaded` expose none of them. Moderator: either F2 needs them (then add them to `OrphanEntry`), or cut them from the WO.
**P3-4 — The first generation's folder entry is never synced.** `ensure_live` creates `Recovery/<rid>` (`:284`), but only `<rid>/` itself is dir-synced by `write_replace`. After a power loss on the first generation, the folder can vanish. Fix: `let _ = self.fs.sync_dir(&self.dir)` after creating a new folder.
**P3-5 — The Windows retire gap (compile-only).** `:520-521` drops the lock before the rename. Another process's `scan` can claim the live session in that gap. The rename then fails, the lock can't be taken back, and the session loses its snapshots. On the re-take path, the `claimed` flag also becomes `false`. Record this in the doc comment and STATUS as a known Windows limitation.
**P3-6 — Copy.** `SnapError::reason()` is plain English and never promises "no loss" (question 7: OK). But `read_manifest`/`verify` pass read errors through `io_reason`, so a denied **read** says "Varos isn't allowed to write there." (`:300`, `:404`, `:406`).
**P3-7 — Cross-piece notes (D/F2).** The returned `Generation.seq` can be higher than the seq that was issued (`:331-334`), so D must match completions on its own issued seq. A recovered rid stays `claimed` and is listed by `scan` until its first publish. F2 must filter out rids that are open in sessions, or Start will show the document twice.

## Brief questions answered
- (1) A crash between the blob and the manifest leaves the previous manifest and both of its generations intact. Mid-retire, the atomic rename leaves either a whole folder or a tombstone. The writer's lock is held from its first write until retire. The only unsound spot is P2-1.
- (2) std `try_lock` is `flock` on Linux/macOS, which is per open file description. Each `try_take_lock` does a fresh `open`, so two stores in one process conflict, as the test shows. On Windows `LockFileEx` covers only the bytes of `session.lock`. `scan` never reads a folder it could not lock, and blob reads are unaffected, so there is no failure there. The Windows issue is P3-5.
- (5) Leaving the std lock file out of the log is sound for lexical paths (it is always `<dir>/<valid rid>/session.lock`). But the proof is blind to symlinks (P1-1).
- (6) The tests don't meaningfully duplicate each other. Three tests call `cleanup_completed` twice for no reason, which is harmless.
