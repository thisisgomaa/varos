> **Status:** proposed — documentation-only UI planning slice, requested by Ahmed on 2026-09-27; independent review/merge pending.
# UI v2 planning revision

Branch: `codex/ui-system-plan`. Baseline: `601fd7c` (merged project-health PR #1).

## Problem and outcome

The v1 UI proposal required a broad framework before Start and contained unresolved lib/bin, responder, transaction, cache and panel-size contradictions. Five independent reviews requested changes. Since then S1 and owner decisions changed the baseline.

This slice replaces the active proposal with bounded contracts and sequential file ownership, preserves the old body in history, and assigns every review finding a design response or named later piece. Existing owner decisions remain separate from unapproved behavior changes. Start/recovery use the current host/lifecycle path; the next implementation piece remains S5-C, not a UI rewrite.

## Scope

- [UI System v2](../../specs/UI_SYSTEM.md): current proposal and minimum Start controls.
- [Review disposition](reviews/UI_SYSTEM.v2.disposition.md): implementer response, not independent approval.
- [Historical v1](../../history/UI_SYSTEM_V1_THROUGH_2026-09-27.md): exact prior body after the status banner.
- PLAN/STATUS and E2/S6 work orders: sequence and ownership alignment; PR #1 state corrected to merged.
- No Rust, dependencies, assets, vendor, accepted ADR or runtime changes.

## Acceptance and limits

All 86 numbered findings (architecture 11, input 14, visual 13, panels 27, economy 21) map to a response. No reviewer recommendation is silently recorded as an owner decision. Typed-field settlement and canvas Tab behavior stay explicit future choices; font/Home decisions are not reopened. Historical body preservation, relative links/anchors and diff whitespace are checked. Run repository-required local tests/clippy/format before committing; exact evidence goes to [GATE_LOG](../GATE_LOG.md).

The original reviews keep their REQUEST CHANGES verdicts. A self-review of this revision cannot replace independent merge review. No new visual test, performance measurement, hosted CI success or implementation completion is claimed. Push the good work branch under the standing repository rule; do not merge this planning revision without the separate review.
