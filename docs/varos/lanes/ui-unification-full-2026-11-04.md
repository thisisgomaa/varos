# Complete UI unification pass — 2026-11-04

Status: core migration accepted by the owner in build 20261104.6; aggregate Claude review approved. Owner authorized
remaining UI migration, source guard/gallery and aggregate Claude review; external theme import
and export explicitly deferred. Starting installed baseline was 20261104.4 (side dock fill accepted).
Stage 2 checkpoint 20261104.5 is signed/staged separately and was not installed after scope expanded.

Implementation follows [the design-system guide](../UI_DESIGN_SYSTEM.md). Existing Palette/Tokens
remain color authority. Foundation centralizes typography and control/surface roles. Varos theme
applies once through a small ownership hook. Shared widgets, Home, fonts/preferences, editor
chrome, remaining panel/dialog controls and old presenters consume marked scalar hooks or Varos
helpers. Old box/card tabs and document/panel/subsection tabs share the same font/state ink;
subsection underline and document close/dirty behaviors remain distinct capabilities.

A narrow occurrence-counted source exception manifest covers spinner diameters, typographic T
icon geometry and one-point insertion/underline caps. No external asset, handwritten JS/TS,
engine command, document font or file-format change was added. Input IDs, shortcuts, focus,
commit/cancel, undo and drag logic remain with incumbent callers. Saved panel sizes and the
owner-accepted sizing/picker work are retained.

Independent specialized agents implemented disjoint source areas. A fresh reviewer found no
blocking production issue. Initial UI pass exposed one legacy card test expecting the old
rounded dark tab; its expectation was updated to the approved shared tab surface while retaining
geometry/shadow assertions. Native checkbox paint was explicitly linked to the checkbox role and
covered by a real frame test. Final local gates/captures/build and Claude aggregate review passed. The source guard was
expanded to direct constant painter-radius calls, exposing remaining control backgrounds which
were linked centrally. Exact geometry exceptions were reviewed and counted. Its exception policy
is embedded with the scanner at compile time so an older test binary cannot read a newer policy.
The full gate pass passed after this source-guard increment; previous logs are retained.

Evidence and source snapshots: `plan/ui-unification-full-2026-11-04/` (gitignored).


## Final local verification and installed candidate

- UI: 2,804 unit tests and 2 integration tests passed, zero failed, 28 ignored.
- Full workspace: 11,218 passed, zero failed, 43 ignored across 91 result groups.
- Workspace Clippy/all-targets with `-D warnings`, wasm, fmt and diff checks passed.
- Four headless captures (controls Dark/Light, Home, editor tabs/panels) inspected. Native/shared
  button colors and text-field focus borders now share semantic roles; selected/frameless/explicit
  fills, ComboBox and DragValue retain native behavior, covered by actual frame regressions.
- Release build passed in 4m 06s, `-j 4`, with the same product-version environment as checkpoints.

Build **20261104.6** installed while the app was closed; all five installed files match the signed
manifest and deep/strict codesign verification passed. Build **20261104.4** retained for rollback;
settings retained and no app launch. Stage 2 checkpoint 20261104.5 was never installed.
Receipt/source patch: `dist/ui-unification-full-2026-11-04/` (gitignored).
Patch SHA-256: `880171b236da52249f840713f301c544204be05261880834e34ecc95da71e676` on HEAD `04af06a5080f68ced1d80e29efe515adc1d7363e`.

Read-only upstream fetch completed at `d7690eba02fb3e98e2e854a6a8cf844214852db3`; no upstream
merge or publication performed. Owner tested build 20261104.6 and confirmed the appearance,
controls, drag and shortcuts on 2026-11-04; native acceptance recorded.


## Aggregate Claude review

After quota reset, Claude Fable completed the aggregate source/gate review and approved the
unification with no blocker. Evidence: `plan/ui-unification-full-2026-11-04/claude-review.json`
and `.md`; model reported `claude-fable-5-1`. It identified a pre-existing ui-medium fallback
coverage gap as a nonblocking follow-up. The follow-up fixes that gap and restores the guide's font-fallback invariant. The regular UI fallback chain is seeded into the
medium role, the family identity is centralized and added to catalog coverage. A focused follow-up
review and gates are under `plan/ui-font-followup-2026-11-04/`.


## Font fallback follow-up — signed candidate 20261104.7

Claude Fable separately approved the completed fix with no blocker. Both optional documentation
comments were corrected. The medium UI family inherits the full regular fallback chain while
retaining Inter Medium and Arabic first; a shared family constant and catalog coverage guard it.
No font asset was added. Native acceptance of the core migration remains recorded above; the
additional glyph fallback change is provisional until owner testing.

Final local follow-up gates: 2,805 UI unit tests plus 2 integration tests passed, zero failed,
28 ignored; workspace/all-target Clippy with `-D warnings`, `cargo xtask wasm`, fmt and diff checks
passed. Release build passed with `-j 4` in 4m 02s. Logs and actual Claude review:
`plan/ui-font-followup-2026-11-04/`.

Build 20261104.7 installed after the owner saved and closed the app. All five installed files
match the signed manifest and deep/strict signature verification passed. Build 20261104.6 is
retained for rollback; settings retained, no app launch. Receipts: `dist/ui-font-followup-2026-11-04/`. Source patch SHA-256:
`6d4e5976376f29b3bd75185eead91eef92d7de607d6a0820b107979e38f932c7`,
HEAD `04af06a5080f68ced1d80e29efe515adc1d7363e`. Deep/strict signature verification passed.
No commit, push, publication or app launch was performed.
