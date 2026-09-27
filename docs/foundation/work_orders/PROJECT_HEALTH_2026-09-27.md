> **Status:** current — first implementation slice of the project review accepted by Ahmed on 2026-09-27.
# Project health: current state and platform gates

Branch: `codex/project-health`. Baseline: `5ee21d4`. Product behavior is unchanged.

## Problem and outcome

Entry-point docs still describe Windows-first and old test totals. STATUS mixes current truth with old branch states. CI only targets Windows, and the dependency-direction checker requires PowerShell unavailable on the primary Mac.

This slice provides one short current-state page, preserves the earlier page in history, distinguishes tested modules from integrated features, and adds full Mac gates with Windows compile-only checks. The architecture checker moves to standard-library Python; the old PowerShell entry point delegates to it so there is one implementation of the rules.

## Scope and acceptance

- Update README, CONTRIBUTING and STATUS without changing accepted ADRs or historical decisions.
- Preserve the original status body in `docs/history/STATUS_THROUGH_2026-09-26.md`.
- Check the same four-crate dependency edges and `egui_tiles` confinement on both CI platforms.
- Demonstrate rejection of reverse edges, platform dependencies in core, Winit in renderer, missing workspace members and `egui_tiles` use outside the adapter.
- Run macOS tests/build/clippy/fmt, Windows-target clippy and the portable gate locally. Record limits honestly: hosted CI and the PowerShell compatibility wrapper require their respective environments.
- Keep library recovery/Start work marked pending until E2/F1/F2 complete. The existing S2/S3 work order defines their acceptance tests and UI prerequisite; this slice does not enable automatic recovery.

## Next implementation boundary

Follow `docs/PLAN.md`: reconcile UI v2 and complete S5-C/D/E before the next broad UI implementation. UI System U0 must supply the minimum shared controls for E2; avoid implementing the entire proposed UI framework just to ship Start. E2 then owns Start/recents integration, followed by F1 and F2 in order. UI/Editor extraction is incremental and behavior-preserving. No broad rewrite, new crate or new framework is part of this order.

## Scope extension — owner request, 2026-09-27

Before publishing the branch, the owner requested review, organization and updating of the program plans. This slice now also provides `docs/PLAN.md`, reconciles work-order states and UI owner decisions, marks the old plan map historical, and corrects the wire contract's current-vs-pending claims. Accepted ADRs and historical bodies remain unchanged. No runtime feature is implemented by this documentation work.

Publishing is authorized: push the branch and open a draft PR after validation. Independent review is still required before merge.

## Evidence

Local results and unverified environments are recorded in [GATE_LOG](../GATE_LOG.md). Independent merge review remains separate from implementation validation.
