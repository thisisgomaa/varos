> **Status:** reference — independent code review, 2026-09-24.
# Code review — DFS S3 piece B: recents + settings store (`feat/dfs-s3-b-recents`, `c4d7343...e8cd0f7`)

**Verdict: APPROVE WITH NITS.**
1. `recents.rs`/`settings.rs` match work order §3.4: atomic JSON with a version field, 20-item dedupe by `dev_ino`-else-path (review finding P2-7, the exact `FileKey::same_file` rule), `record`-only construction so Recent can't be polluted, `.bad` aside on corrupt input, version-mismatch left untouched. `save_is_atomic_under_fault` proves the old `recent.json` survives a `Rename` fault.
2. The private `same_file` copy is real waste but is architecturally forced: `storage` is `pub mod` in `lib.rs`, `workspace` is `mod`-only in `main.rs` (bin-private), so the lib literally cannot see `FileKey`. Confirmed by reading, not assumed.
3. One overclaim: the report says "12 new tests"; there are 11 (`recents.rs` 9, `settings.rs` 2, both counted by `#[test]` grep and by running `cargo test -p varos-app --lib`).
4. Two real (small) correctness gaps: the "newer version" warning text is asserted, not derived — it will mislabel an *older* file once `RECENTS_VERSION`/`SETTINGS_VERSION` ever bumps past 1; and `corrupt()` silently destroys a pre-existing `.bad` sibling before writing a new one, so a second corruption event erases forensic evidence of the first.
5. Nothing here touches `varos-core`, GPU/window code, or an `EventLoop`; no proprietary material; diff is additive-only (`mod.rs` + 2 new files, 438 lines).

## Gates (measured by this reviewer, in the worktree, tree clean at `e8cd0f7`, after `touch`ing all `varos/crates/**/*.rs`)
- `cargo test --workspace -j 4`: **421 passed, 0 failed** (reproduced twice). `cargo test -p varos-app --lib`: **46 passed**, of which `storage::recents::tests::*` = 9, `storage::settings::tests::*` = 2.
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: clean. `cargo fmt --all -- --check`: exit 0.
- `cargo check --workspace --target x86_64-pc-windows-msvc` and `--target aarch64-apple-darwin`: clean. `cargo clippy -p varos-app --all-targets --target aarch64-apple-darwin -- -D warnings`: clean.

## Findings

**1. P2 — The version-mismatch warning always says "newer version", even when a future file is actually older.** `recents.rs:150-158`, `settings.rs:66-73`
```rust
if probe.version != RECENTS_VERSION {
    return (Recents::default(), Some(format!(
        "The recent documents list was saved by a newer version of Varos (format {}); it was left unchanged.",
        probe.version)));
}
```
- The check is `!=`, not `>`. Today `RECENTS_VERSION == SETTINGS_VERSION == 1` and no older format ever existed, so the bug is latent, not reachable — that is why the review's question 4 ("is an older version migrated or refused, and is that right") has no failing test to point to yet.
- The moment either constant is bumped to 2, a file still carrying `"version":1` from an older Varos build hits this same branch and is told it came from a *newer* version — backwards, and confusing for Ahmed if he ever downgrades or opens an old data dir.
- The behavior itself (leave the file completely untouched, load as empty, warn) is the right, conservative call for both directions — no migration exists or is asked for by the spec — only the wording is wrong.
- Fix: `if probe.version > RECENTS_VERSION { "…saved by a newer version…" } else if probe.version < RECENTS_VERSION { "…saved by an older version of Varos (format {}); it was left unchanged." } ` (older still gets no migration — that's correct until a real migration is written — just say it accurately). Cheap to fix now while it's still zero-risk; add a test that forges `"version":0` and checks the "older" wording once the fix lands.

**2. P3 — `corrupt()` overwrites a pre-existing `.bad` sibling instead of preserving it.** `recents.rs:165-171`, `settings.rs:81-86`
```rust
fn corrupt(fs: &dyn FsPort, path: &Path, bytes: &[u8]) -> (Recents, Option<String>) {
    let bad = bad_path(path);
    let _ = fs.remove_file(&bad);   // <- destroys whatever was there
    let _ = fs.rename(path, &bad);
    ...
}
```
- The module doc claims "never silently overwritten on first read", which is true for the *current* file being moved aside, but the unconditional `remove_file(&bad)` means a second corruption event (two crashes/bad writes before anyone looks at the first `.bad`) silently loses the first one's bytes.
- Low real-world odds (recents/settings are re-derivable, not user documents), so this is a nit, not a blocker. No test exercises the "second corruption" path either way.
- Fix if it's worth the code: only remove/replace `.bad` if it doesn't exist yet (`fs.rename` will already fail if `.bad` exists and the target platform doesn't overwrite — but Unix `rename` **does** overwrite, so the guard has to be an explicit existence check via `fs.metadata`).

**3. P3 — `corrupt()`'s `bytes` parameter is dead.** `recents.rs:165`, `:166`
- `fn corrupt(fs: &dyn FsPort, path: &Path, bytes: &[u8])` immediately does `let _ = bytes;` and never uses it — the rename moves the file already on disk, it doesn't need the bytes in hand. `settings.rs`'s `corrupt()` has no such parameter, so the two modules are inconsistent for no reason.
- Fix: drop the parameter and the two call sites' `&bytes` argument.

**4. Checked and fine (no test gap, no fix needed):**
- Dedupe on re-save with a changed inode: `same_file` short-circuits on `a_path == b_path` before ever looking at `dev_ino`, so `record(path, id(1))` then `record(path, id(2))` (the exact "file re-saved, new inode" case) merges into one entry regardless of what the ids are — this is provable from the code, and the existing `dedupe_by_dev_ino_else_exact_path` test (path-match branch with a *changed* identity, id→None) already kills a `||`→`&&` mutation on that branch. A dedicated test naming the "re-save changes inode" scenario explicitly would read better, but nothing is actually uncovered.
- `.bad` cannot loop: once `corrupt()` renames `recent.json` away, the path no longer exists, so the next `load()` takes the `NotFound` branch (empty, no warning) — `corrupt()` is never re-entered by loading the same broken file twice.
- Unknown-version file is provably left byte-identical (`unknown_version_is_not_overwritten_silently` reads the file back and diffs it against the original bytes).
- Waste (recommend, not required): the duplicated `same_file` rule (here and in `workspace::FileKey::same_file`) could live in one tiny module inside the lib crate (e.g. `storage::identity`) that both `recents.rs` and the bin's `workspace.rs` import, instead of two copies of the same three-line rule kept in sync by hand. Not done here because the lib genuinely cannot see `workspace.rs` today; this is a suggestion for whoever next touches file identity, not a requirement of this piece.
- Schema: `RecentEntry`/`Settings`' on-disk field names (`version`, `items`, `path`, `name`, `last_opened`, `file_id`, `recovery_enabled`) are already documented in prose in the work order §3.4 and match the code exactly. This is app data, not the `.vrs` document format, so it is correctly out of scope for `VRS_FORMAT.md`/ADR-0004.
- No `varos-core`, GPU, window, or `EventLoop` code touched; no shadows/animations/tokens concerns (no UI in this piece); commit is a single conventional commit adding only the two new files plus a 4-line `mod.rs` edit.
