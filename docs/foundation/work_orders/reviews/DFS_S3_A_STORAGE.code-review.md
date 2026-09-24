> **Status:** reference — independent code review, 2026-09-24.
# Code review — DFS S3 piece A: storage foundation (`feat/dfs-s3-a-storage`, `2c4dd6e...fd8fa93`)

**Verdict: REQUEST CHANGES (one P1, small fix).**
1. The resolver, `AppLayout`, the durable writer's step order and the fault matrix match work order §3.2/§3.3. The old file stays byte-identical under every injected pre-rename fault (Create/Write/Sync/FullSync/Rename), and each of those tests checks that its fault actually fired.
2. **P1:** the F_FULLFSYNC fallback (plan-review P1-1, the reason for commit `fd8fa93`) never fires on the error macOS really returns. `ENOTSUP` (45) maps to `ErrorKind::Uncategorized` in std, so saving to an SMB share would still fail after F1.
3. Size is fair. `FsPort`/`FaultFs` is what the WO's fault matrix needs. The `main.rs` change is minimal (two functions, 12 lines). The chrono `clock` feature costs about 60 KB and adds no startup work. `time_text` belongs here because the WO puts it in piece A.
4. Nits: an unused `remove_dir` in the frozen port, a slower hand-written CRC-32, a dead `#[allow]`, and the report says 12 methods when the port has 11.

## Gates (measured by this reviewer, in the worktree, tree clean at `fd8fa93`)
- `cargo test --workspace -j 4`: **372 passed, 0 failed** (reproduced). Storage lib tests: **26 passed**.
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: clean. `cargo fmt --all -- --check`: exit 0.
- `cargo clippy -p varos-app --all-targets` on `x86_64-pc-windows-msvc` and `aarch64-apple-darwin`: clean (compile-only).
- `Cargo.lock` gains only the `chrono` and `libc` edges; `chrono 0.4.45` and `libc` were already present, so no new crate.
- chrono `clock` size probe (standalone release binary, stripped, Linux): 353,984 → 417,368 bytes, **+62 KB**, and only once `Local` is actually called. `time_text` has no caller yet, so the shipped binary is unchanged today. `Local` reads `/etc/localtime` lazily on first use. No tz database is embedded (that would be `chrono-tz`, which is not used), and there is no startup cost.
- CRC-32 speed probe (this file's `crc32`, `rustc -O`, 32 MiB): **94 ms** on this VM.

## Findings

**1. P1 — The macOS full-sync fallback misses `ENOTSUP`, so the P1-1 fix is not in effect.** `durable.rs:48-50`
- `full_sync_unsupported` matches `ErrorKind::Unsupported | InvalidInput` or raw `ENOTTY` (25).
- std 1.94.1 `library/std/src/sys/io/error/unix.rs` maps `libc::EOPNOTSUPP => Unsupported` and `ENOSYS => Unsupported`, but it has **no arm for `ENOTSUP`**.
- On macOS `ENOTSUP = 45` and `EOPNOTSUPP = 102` are different numbers (libc `unix/bsd/apple/mod.rs`). So a real `ENOTSUP` from `fcntl(F_FULLFSYNC)` becomes `Uncategorized`: no fallback, `WriteError::Sync`, and "Couldn't save".
- `ENOTSUP` is exactly the SMB case. Go's `internal/poll/fd_fsync_darwin.go` falls back on `syscall.ENOTSUP` "in scenarios such as SMB mounts".
- The same predicate drives the directory sync (`:138`, `:349`): once F1 wires this in, every save to such a volume would come back `ReplacedUnconfirmed`, with the "couldn't confirm" message and a document that stays dirty.
- Why tests are green: `full_sync_unsupported_falls_back_to_plain_fsync` (`:659-686`) injects `io::Error::from(ErrorKind::Unsupported)`, which is a kind and never passes through errno decoding. So it cannot see the bug.
- On Linux, 95 decodes as `Unsupported` (measured). Only the Mac numbers are wrong.
- The commit message says "an ENOTSUP/ENOTTY/EINVAL from the full sync now retries". That is not true on macOS.

**Fix (pick one; the first is simpler and matches SQLite exactly):**
- (a) In `full_sync` (`:79-81`), fall back to `plain_fsync` on **any** F_FULLFSYNC error. SQLite `os_unix.c` does `if( rc ) rc = fsync(fd);`, and a real I/O error still surfaces from the plain `fsync`.
- (b) Or make the predicate errno-based. Add `const ENOTSUP_MAC: i32 = 45; const EOPNOTSUPP_MAC: i32 = 102;`, each checked with the existing `const _: () = assert!(… == libc::…)` pattern, and match `raw_os_error()` against `{45, 102, 25, 22}` when `cfg!(target_os = "macos")`.
- Either way, keep the predicate for the `sync_dir` outcome (`:349`) and have it accept raw `ENOTSUP` there too.
- Add a test that builds the error with `io::Error::from_raw_os_error(45)`, not from an `ErrorKind`. For (b), put the errno table in a pure `fn unsupported_errno(os: Os, raw: i32) -> bool` so the Mac numbers are tested on Linux.
- Keep the Mac hand check (a USB stick or SMB share), and also confirm that a save to the internal APFS disk returns `Durable`, not `ReplacedUnconfirmed`. Linux CI cannot prove either.

**2. P3 — `FsPort` has 11 methods, not the "12" reported; `remove_dir` has no basis in the WO.** `durable.rs:101-119`
- `read`, `create_dir_all` and `read_dir` have no caller in A, but they are part of the WO's frozen §3.3 list, so keep them.
- `remove_dir` (empty folders only) is not in the WO list. Plan-review P2-4 changes retire to `rename` + `remove_dir_all`, so C will likely need something else.
- Fix: drop `remove_dir` (and `Step::RemoveDir` / `Op::RemoveDir`) and let C add the operation it actually uses. Correct the count in the hand-off.

**3. P3 — The hand-written CRC-32 duplicates `crc32fast`, which is already compiled.** `checksum.rs:1-33`
- The WO asked for the in-house version, and ADR-0005 governs crate edges, not external packages. So this is allowed.
- But `crc32fast 1.5.0` is already built via `flate2` (lopdf/png), so a direct dependency adds no crate.
- It uses SIMD: roughly 2–5 ms against the measured 94 ms per 32 MiB snapshot. It also removes the table code.
- Moderator's call. If kept, nothing else changes.

**4. P3 — A dead `#[allow(deprecated)]` with a misleading comment.** `paths.rs:73`
- `std::env::home_dir()` compiles with no warning on 1.94.1 (checked). Remove the attribute and the comment.

**5. P3 — The temp file holds the new bytes with default permissions until after the write.** `durable.rs:318-333`
- For a `0600` document, the temp is `0644` (umask) for the whole write/sync.
- If the best-effort `set_permissions` (`:332`) fails, the replaced file stays `0644` without any notice.
- Fix: copy the permissions right after `create_new`, before writing.
- Separately, ownership and xattrs (such as Finder tags) are not carried over. That is not a regression (today's `write_atomic` behaves the same). Note it in `docs/`.

**6. P3 — Small text slips.**
- `durable.rs:288`: "≤ 173 bytes" should be 172 (1 + 128 + 1 + 32 + 10).
- `io_reason` (`:232-260`) always ends with ".", but the spec's Open/Export copy is "{reason}. Your…", which would print "..". Tell F1/S6 to strip the trailing period, or drop it here and add it in the Save copy.
- `resolve_link` errors are reported as `WriteError::Create` (`:297`). Acceptable, but the name is off.

**7. P3 — WO deviations to record.**
- The WO (§4 A) says "crash log via `write_replace`", but `main.rs:457-463` still uses `fs::write`. Plain write is the better choice inside a panic hook (no fsync stall in a dying process), so amend the WO rather than the code.
- The Windows crash log moves to `%APPDATA%\Varos\Logs\crash.txt`, per spec §145.
- `docs/foundation/MAC_SHELL_PORT.md:41` ("no crash file" on Mac) is now stale. Update it at merge.

Checked and fine:
- The temp-name cap cuts on a UTF-8 boundary (`is_char_boundary` step-back; tested with 2-byte Arabic letters where byte 128 falls mid-letter).
- Windows limits are counted in UTF-16 units, and 128 bytes is at most 128 units.
- A symlinked destination replaces the target, not the link.
- Resolver: no working-directory fallback, and a relative override is refused.
- `main.rs`: `win_state_path` stays `None` on macOS.
- `FaultFs` has to live in the lib: the bin's tests cannot see `cfg(test)` items in the lib.
- No core, UI, GPU or EventLoop code is touched. No proprietary material. Commits are conventional and scoped.
