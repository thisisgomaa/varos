> **Status:** implemented in lane G — provisional UI; owner design review, independent review and native acceptance pending.
# Autosave to the open file

- Date: 2026-10-09
- Authority: [PLAN](../../PLAN.md) slice 1.10 and owner decisions; [ADR-0008](../../adr/ADR-0008-vrs-format-versioning.md), [ADR-0009](../../adr/ADR-0009-varos-bridge.md), latest [ADR-0011](../../adr/ADR-0011-bridge-connection-and-trust.md) amendments.
- Dependencies: slice 0.6 Revert (landed); minimal settings extension now, full preferences in 9.1. No `.vrs` format bump.
- Owner intent: autosave writes into the open file itself; recovery copies remain separate.

## Preference and timing

Recommend enabled by default (also PLAN's stated default), with an editable **2-minute inactivity interval**. Proposed accepted range: 30 seconds–30 minutes, default 120 seconds; reject invalid values rather than silently clamping.
This interval is N: a dirty document becomes due at `last_committed_content_change + N`, not every two minutes regardless of interaction. Continuous editing postpones file autosave; recovery still protects long editing sessions on its own cadence.
Every committed human or Bridge/agent mutation, including undo/redo to changed content, resets the deadline. Selection, pan/zoom, hover and status changes do not. An undo that returns to the saved checkpoint cancels pending autosave.
Track each tab independently, including background tabs; use monotonic time and the committed revision. A gesture's provisional revisions must not be mistaken for settled changes.
Enabling or changing the interval starts a fresh interval for dirty tabs; disabling cancels queued attempts, not an already completed atomic replacement. Explicit ⌘S remains available with autosave disabled.
Extend `storage/settings.rs` with `autosave_enabled` and `autosave_interval_seconds`, preserving `recovery_enabled`; keep preferences outside Document.
Introduce a settings-version migration from today's version 1 (recovery switch only), supplying defaults without resetting the owner's recovery choice. Unknown newer settings remain untouched; preserve existing corrupt-file warning/backup behavior and use the durable settings writer.

## Eligibility and exclusions

All conditions must hold when capturing a job and again at admission to publication:
- Autosave enabled; tab exists, is dirty, has a backing file and is writable (including writer ownership).
- Idle ≥ N seconds since the last committed content change.
- No open transaction, canvas gesture, color-picker drag, unresolved field edit or Bridge batch in flight; never force-commit/cancel these to make autosave eligible.
- No save, Save As, Save a Copy, export or Revert/close transition in flight; serialize against the shared file-job lane.
- No recovery snapshot/manifest write or retirement in flight; serialize that lane too, without starving recovery.
- Source identity and content fingerprint still match the version accepted at Open or the last confirmed save; missing/unreadable files are conflicts, not permission to recreate them.

Untitled and pathless recovered tabs continue recovery only; autosave never opens Save As spontaneously. Read-only tabs never become autosave candidates.
If a deadline expires while blocked, wait for the corresponding transaction/job completion event and reevaluate all predicates. Do not poll an expired deadline. New committed content starts a new N-second interval; removing a non-edit blocker may allow an already-due attempt.
Use `ControlFlow::WaitUntil` for the earliest eligible deadline and `Wait` when there is none. No periodic redraw, idle timer churn or repeated warning modal.

## Disk conflict: never overwrite another app's change

Reuse the existing `file_ports.rs` warning: **“‘{name}’ was changed by another app.”** / **“Saving now would replace those changes.”** with Save As / Replace Anyway / Cancel.
On conflict, abort the autosave attempt and latch that tab as conflict-paused; show the warning on the UI thread when safe (defer presentation through a gesture/modal). Keep dirty state and recovery copies. Do not acknowledge a new fingerprint merely to unblock a timer.
Save As and Replace Anyway are explicit manual actions through the existing lifecycle, never automatic branches. Cancel keeps autosave paused for that tab. Resume only after an explicit successful save/reload establishes a new baseline; then restart the interval.
Recheck identity/fingerprint on the worker after serialization and immediately before replacement, including canonical destination/parent identity and aliases. A queued job's earlier check is insufficient.
Today's `durable::Fingerprint` contains only length and mtime: it cannot detect same-size/same-time replacement. Slice 1.10 must strengthen the baseline/check with content hash and file identity, reusing Bridge's guarded-save boundary where applicable.
Cooperative writer leases serialize Varos hosts, not unrelated apps. A stat/hash followed by ordinary rename still has a race against an uncooperative writer; do not claim an absolute filesystem compare-and-swap guarantee. Before enabling a destination, establish the strongest supported coordination/identity-checked publication path and document the residual race; refuse automatic writes where safe ownership/identity cannot be established.
A change detected at any check always refuses publication and follows the same warning. Never call the unchecked replacement writer directly from the timer.

## Capture, durable write and completion

Capture a settled immutable snapshot plus session ID, lifecycle generation, revision, ticket, canonical destination and expected disk baseline on the owning thread. Pin all resources needed by that snapshot; future ADR-0014 images require the sibling asset store too.
Serialization, validation and disk I/O run on the existing IO worker/FileJobs infrastructure. Emit exactly the native `.vrs` bytes that ⌘S would emit for the same captured snapshot and options (`varos_pdf::write_pdf_checked` today), not recovery JSON or pure export PDF.
Reuse `file_ports::durable_save` and `storage::durable::write_replace` through the guarded destination adapter: exclusive same-folder temp, preserved permissions, full write and sync, atomic replacement, directory sync and best-effort temp cleanup. Do not add a parallel writer or UI-thread fallback.
Use a publication permit coordinated with transaction/gesture/Bridge admission. If interaction or revision changes while encoding, invalidate the queued publication and retry after the new idle interval; never replace the file during an open edit. Acquire/check the permit at the final write boundary, without holding the UI thread across encoding or disk sync.
The permit design must cover the check-to-rename window: serialize that short publication window with edit admission, then release it promptly. Human input takes priority before publication admission; verify native latency rather than claiming a long worker lock is harmless.
At most one file job per session/destination; coalesce obsolete timer requests. Explicit Save takes priority; Save As/copy/Revert/close use existing barriers and invalidate obsolete autosave captures. Ignore closed-tab, stale-ticket and old-generation completions.

On **Durable**, make the written snapshot the checkpoint and refresh file identity/fingerprint from the committed result. Clear the dirty dot only if current content equals it; later edits stay dirty and receive their own deadline. Preserve undo/redo and the tab's backing path.
Autosave leaves **Recent unchanged**, including order, timestamps, cached summary and thumbnail. Existing `lifecycle::save_done` calls `remember`/`rendered`, so introduce explicit completion origin/policy instead of blindly routing autosave through all manual-save side effects.
Do not re-fingerprint an unrelated later disk replacement and label it our baseline; tie the completion baseline to the bytes/identity published, then detect any subsequent external change.
On pre-replacement error, old file stays intact, document dirty, prior checkpoint and recovery copies intact. Show a concise error and back off at least 30 seconds; no hot retry loop. Retry still obeys idle, interaction and disk-conflict guards.
On **ReplacedUnconfirmed**, acknowledge that replacement happened but durability is uncertain; retain dirty/unconfirmed state and recovery, show the existing save-confirmation warning, and pause automatic retry until explicit resolution. Never say “Saved” or retry an overwrite silently.

## Revert, Save a Copy and recovery

Revert (F12) remains the escape hatch for **unsaved changes since the latest successful file save, including autosave**. Existing `lifecycle::revert` asks confirmation, rereads current disk bytes, resets history on success, and leaves the workspace unchanged on cancel/read failure.
Once autosave has replaced the file, Revert cannot recover the last manually saved version. It is disabled when clean under today's `can_revert`; undo remains available after autosave. Do not promise a pre-autosave version history; that is a separate deferred feature.
Entering Revert invalidates queued autosaves and crosses the file-job barrier before reading. The user must be able to confirm/cancel without a timer overwriting the target underneath the dialog. Successful Revert establishes a new disk baseline; Cancel resumes normal eligibility.
Save a Copy retains its landed semantics: separate destination, no backing-path/checkpoint/dirty/Recent change. It does not satisfy a pending autosave of the backing file; resume only after copy completion and revalidation.

Keep recovery enabled independently with its existing 30-second first-unprotected-change cadence, two generations, manifest-last durability, CRC/decoded fallback and pathless Recover behavior. Autosave does not replace or disable this subsystem.
Current `storage/scheduler.rs` retires copies whenever a session becomes clean. **Required change:** distinguish an autosave checkpoint from explicit save/discard/clean shutdown retirement, so autosave does not erase its recovery fallback merely by clearing the dot.
Retain the bounded recovery generations through autosave success/failure; recovery is still crash protection, not unlimited version history. Explicit lifecycle retirement remains allowed after its normal successful-save/discard/clean-close barriers; no retirement while either write is pending.
Avoid offering a redundant recovered document after a crash when its generation exactly matches the confirmed backing file; retain differing/uncertain generations for the existing recovery choice, never discard them based on mtime alone.
The two schedulers share worker arbitration but have separate deadlines, state and preferences. Recovery gets priority when both are due; completion wakes autosave for revalidation. An error in one must not falsely mark the other successful.

## Visible contract — controls only; Figma later

Use existing kit controls, spacing and muted status tokens. No new raw colors, fonts, panels or animation; owner Figma choice is required before visible implementation.
Settings row controls:
- “Autosave to file” on/off switch, default on.
- “After inactivity” interval control, default “2 minutes”, disabled when off.
- Helper text: “Saves changes to the open file. Recovery copies stay on.” (reflect recovery off if disabled.)

Muted status hint states:
- “Autosaving…” only while an actual job is running.
- “Saved automatically at 14:32” only after confirmed durable completion.
- “Autosave waiting for edit to finish” when due and interaction-blocked.
- “Autosave paused — file changed in another app” for latched conflict.
- “Couldn't autosave — {reason}” / “Save needs confirmation” using existing error/warning treatment when action is needed.
Do not show an autosave success for recovery, export or Save a Copy. Coalesce transient hints; idle clean tabs need no permanent countdown.

## Required verification

1. Fake-clock scheduler: default on/120 s, editable bounds, every content edit resets N, non-content actions do not; disabling/re-enabling, continuous edits and undo-to-clean.
2. Each blocker separately and combined: transaction, drag, picker, invalid field, staged/committing Bridge batch, read-only, pathless, save/export/copy/recovery/retire/Revert/close. Zero writes while blocked.
3. Bridge edits reset timing only on atomic commit; rejected/no-op batches do not dirty the document. Autosave never captures or publishes a partial batch.
4. Queue/capture/encode/publication races: new edit invalidates capture, final permit cannot overlap interaction, session switch/close/reopen/stale ticket cannot update the wrong checkpoint.
5. Native serialization parity with ⌘S; durable completion clears dot only for saved content; undo/redo preserved; Recent byte-for-byte unchanged; Save a Copy still leaves dirty state.
6. External change before capture, during encode and at replacement; same-size/same-mtime edits, delete/recreate, rename, hard-link/symlink aliases, changed parent, unreadable and read-only target. Warning once, no automatic Replace Anyway, original external bytes preserved on detected conflict.
7. Fault injection at temp creation/write/sync/rename/directory sync; disk full and permissions; correct failed versus ReplacedUnconfirmed outcome, bounded retry, no checkpoint lie or lost recovery.
8. Recovery and autosave become due together; no overlapping writes, no starvation; two generations survive autosave, corrupt-newest fallback works, crash/relaunch deduplicates only proven equal content.
9. Revert before autosave, after autosave plus another edit, conflict Revert, cancel and read refusal; verify latest-disk semantics and no pre-autosave-history claim.
10. Settings v1 migration, missing/corrupt/newer file, persisted off/interval, independent recovery setting; clean idle event loop has no recurring redraw/upload wakeups.
11. Owner native test after implementation: edit a saved file, wait, observe dirty dot clear, relaunch and see changes; then external-edit conflict, long gesture, Bridge batch and Revert exercises.

## Open implementation decisions

Confirm the proposed interval range and status wording in the later Figma round; default-on and saving into the file are already owner decisions.
Specify and independently review the publication-permit/disk-coordination mechanism before enabling autosave, especially its residual external-writer race and input latency. These are acceptance gates, not implemented guarantees.
Verify recovery retention/deduplication against the existing recovery lifecycle before changing clean-session retirement. The implementation note below records the branch work; installation and native runtime acceptance are not claimed.

## Implementation note — 2026-10-09, lane G

Owner authorized provisional kit controls tonight, overriding the earlier Figma prerequisite.
The default is on, 120 seconds; the interval accepts integer seconds from 30 through 1800.
The event thread owns deadlines and immutable capture; the existing FIFO IO worker owns encoding,
validation and publication. Recovery is observed first and its in-flight jobs exclude autosave.
Each event's edit admission shares a mutex with the final rename. Input/menu/Bridge admission
invalidates the captured permit before mutation. The worker uses try-lock (never waits for input);
encoding, hashing and sync run outside the admission lock. Within it: parent/leaf identity and
metadata recheck, then rename. A superseded capture rechecks eligibility after edit admission; IO failures back off at least 30 seconds without shortening a newer edit deadline.

On supported local Unix volumes the existing pinned-directory adapter refuses directory/final
symlinks, multiply-linked targets, read-only files and changed content (SHA-256), inode or parent.
Autosave takes a nonblocking advisory directory flock for cooperative writer ownership; inability
to establish it refuses the write. Windows automatic publication is refused. The app's one worker
serializes manual/Bridge saves, exports and recovery. Uncooperative external writers can still race
the final identity/stat check and rename; this is not filesystem compare-and-swap. No absolute
external-write exclusion or native input latency claim is made. Independent review and native
latency/long-gesture acceptance remain required before release.

The completion baseline is the temporary file's bytes/identity before publication, not a later
fingerprint of an unrelated replacement. Autosave completion never calls Recent/thumbnail APIs.
Recovery's bounded generations survive autosave checkpoints; explicit saves/reverts/clean close
retain their existing retirement semantics. Launch hides redundant recovery choices only after
valid decoding and content equality with a stable backing file; copies are not deleted by this check.
