# Panel grip and Home isolation review

Status: implementation in progress, owner acceptance pending. 2026-11-01.

## Scope
Update only the existing Varos Panels Preview 2 bundle; retain documents and workspace settings. Toolbar dimensions remain unchanged.

- Compact group grip reduced from 8 to 4 logical points; remove 2-point padding above and below icon rows.
- Host, group and inline Control movement uses click-and-drag intent instead of beginning on pointer press. A click or small hand jitter must not detach a panel; deliberate dragging remains available.
- Home must render without editor docks, floating panels or Color popovers. Returning to the document restores the same workspace. Discard transient panel gestures and flyouts on Home without changing saved state.

## Verification
Passed: 102 panel-system regressions (4 optional captures ignored), all 8 Home tests, scoped UI/app clippy with warnings denied, formatting and diff whitespace checks. Release build passed for desktop app and CLI. Native Home capture passed; the native same-document return exposed an existing Home latch when reactivating the already-active file. Follow-up clears Home after successful open/activation and adds regression coverage. Final verification pending. No full CI or commit claimed. Owner pointer acceptance remains pending.

## Owner-requested promotion

On 2026-11-01 the owner authorized closing this slice and promoting it to the final fork. The same compiled app and CLI (verified after stripping signature metadata from temporary copies) are installed at `../reference/artcraft/spikes/Varos.app`, bundle `com.varos.app`, bundle version 0.20.2/build 20261101.10. The unchanged runtime diagnostic version is 0.20.2-panels.1. Preview 2 is closed. Approved panel/toolbar state transferred to the final UI preferences; unrelated preferences, agent data and the legacy `/Applications/Varos.app` are preserved. Rollback app and UI snapshots are in `dist/panels-final-2026-11-01/`.

Both Home isolation and same-document return passed native verification in Preview 2 before promotion. The final app launch is currently awaiting a startup filesystem read; final native verification is not yet claimed. See `installed.json` and follow-up verification in that directory for the latest status.

Final verification completed after the owner granted macOS Documents access: the existing final fork resumed (PID 53999), both saved documents remained open, Home painted without editor docks or hit targets, and reopening the same active document restored the exact panel layout. Native screenshots were visually checked. Preview 2 remains closed; legacy application hashes remain unchanged. This closes the requested local promotion slice; full CI/commit and exhaustive owner pointer acceptance are not claimed.
