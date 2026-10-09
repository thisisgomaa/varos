# Lane p1-autosave

## Fix round
- Reviewed Sol's `final.txt` and the cycle2 lane contract; agree with all findings.
- Checkout discrepancy: no pre-review commit or REPORT.md was present; HEAD is `7b48f2c`, with the implementation uncommitted/untracked.
- P1 save race: manual/inline/Bridge saves carry the pre-publication temporary-file fingerprint through completion; RecentStore forwards it.
- P1 unsafe reads: regular-file descriptors, Unix nonblocking open, 64 KiB streaming hash, native-reader 256 MiB ceiling; pinned reads share the bound.
- P2 retry loop: lock contention is distinct from invalidation; both defer an expired capture by 30 seconds without warning on contention.
- P3 Recent test: lifecycle completion uses a configured real RecentStore and compares its actual entries and persisted file bytes.
- Fix files: `storage/durable.rs`, `file_ports.rs`, `file_jobs.rs`, `lifecycle.rs`, `recent_files.rs`, `bridge_fs.rs`, `autosave_io.rs`, `autosave_host.rs`.
- Regression tests: external replacement after manual/Bridge publication, fresh Bridge baseline, FIFO/directory/oversize refusal, fake-clock contention/invalidation backoff, real Recent invariance.
- PASS: `cargo fmt --all --check`; `python3 ../tools/check_dep_directions.py`; `git diff --check`.
- PASS: `cargo test --offline --workspace -j 2 --no-fail-fast`: 1,531 passed, 0 failed, 15 ignored (85 suites).
- PASS: native and `--target x86_64-pc-windows-msvc` `cargo clippy --offline --workspace --all-targets -j 2 -- -D warnings`.
- PASS: unchanged ratchets and frozen Bridge fixture checks within the workspace run; fixture files unchanged.
- PASS: final publication follow-up tests (2) and file-job tests (4), including fresh Bridge identity, after the last publication adjustment.
- Evidence: `/tmp/autosave-workspace.log`, `/tmp/autosave-clippy-native.log`, `/tmp/autosave-clippy-windows.log`, `/tmp/fix-round-{published,file-jobs}.log`.
- No commit, push, GUI run, or installation performed; provisional UI/owner hand testing and merged-lane integration review remain pending.
