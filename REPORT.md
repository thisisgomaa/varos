Lane H — w2-import wave 2; committed pre-review baseline 5f8e19e; fix-round edits uncommitted.
Status: implemented (provisional UI, owner design review pending).
Retains PDF/AI static paths/clips/rotation/dashes, ASCII DXF layers/arcs/polylines/exact clamped splines, clipboard/Open/Place/drop, CLI/Bridge.
Explicit losses/refusals retained: text/images omitted; bitmap prerequisite absent; unsupported PDF profiles/DXF entities/DWG refuse; CLI output never replaces an existing file.
No persisted keys, format bump, migration, new dependencies or attribution changes.

## Fix round
P1: restored non-import file-effect dispatch; distinct-handler regression covers Save/SaveAs/exports/Print/Copy/Cut.
P1: PDF CTM computes both coordinates from original X/Y before crop offsets; non-diagonal regression added.
P2: clipboard/Bridge conversion runs on existing IO worker; cancellation, generation, revision, active-board and busy publication checks preserve atomicity.
Bridge imports return accepted tickets; request_status retains completion report, committed revision and undo count.
P2: provisional desktop PDF page chooser and DXF declared-unit/mm/point choices are carried in Job options before conversion.
Keyboard/menu Paste and Paste in Place always defer to host queue; immediate-action bypass regression added.
Six new headless regressions cover dispatch/receipts, geometry, queued Bridge cancellation, clipboard atomicity, page/unit conversion and shortcut deferral.
PASS: fmt; dependency directions; workspace 1909 passed/0 failed/15 existing ignored; native + Windows clippy -D warnings; ratchets 3/3; Bridge fixtures 99 passed/4 ignored.
Compatibility: all 250 protected fixtures/UI/token/ratchet files byte-identical to b3d39ee; ui.rs remains 811/843.
Evidence: /tmp/w2-import-fix-round-*.log; compatibility /tmp/w2-import-fix-round-compat.json.
Shared-file changes limited to dispatch/dialog seams and tests; main.rs worker routing is delimited; kit/tokens/ratchets unchanged.
Disagreements: none. Sibling image/gradient merges still require semantic integration checks.
No git writes, commit, push, merge, GUI or install; new independent review and owner/native interoperability/complex fidelity tests remain pending.
