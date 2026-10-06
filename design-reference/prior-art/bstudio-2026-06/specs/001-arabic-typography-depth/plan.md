# Implementation Plan: Arabic Typography Depth + Universal Font Loading

**Branch**: `ui-redesign-port` | **Date**: 2026-06-10 | **Spec**: [spec.md](./spec.md) | **Research**: [research.md](./research.md)

## Summary

Un-park the moat: any Arabic font — Google Fonts catalog, uploaded .ttf/.otf, or the user's device
fonts — shapes through the REAL engine (harfrust+skrifa), with a proper picker (previews, badges),
a true variable-weight slider, tracking that respects joining, harakat v1 (toggle/size/color), and
canvas==export at any weight. Recon surprise: the wght core (B25) is already fixed — the work
concentrates in (a) a runtime font registry + identity/persistence, (b) the export pipeline catching
up to the weighted canvas path, (c) typography UI.

## Technical Context

**Crates touched**: `bstudio-web` (registry, render options, svg export pipeline), `bstudio-text`
(tracking param, mark mask plumbing), NO render-wasm (emsdk) work.
**Build**: light wasm-pack per iteration (~6s) → `src/engine/` (vite hashes; no version ritual).
**JS**: new `src/fonts/fontRegistry.ts` (sources, IndexedDB cache, doc font-table replay), picker
component, weight slider, harakat controls; `textLayout.ts` cache keys; `exporters.ts` untouched
except weight flows already passing through engine calls.
**Persistence**: doc `setting:fonts` table (003 settings layer) + IndexedDB `bstudio-fonts` (upload
bytes by SHA-256, google cache). Constraints: no push; live stays green; suite + QA per wave;
Chrome-first (device fonts Chromium-only by nature).

## Constitution check (v1.1)

I moat-front by Ahmed's order (003 done) ✓ · II no inert controls (the preview-only device list
becomes REAL or is removed; warn badges are working UI) ✓ · III specs/ + backlog updated with code ✓
· IV per-wave Playwright + QA screenshots; failure-direction probes (bad font, offline google,
revoked device permission) ✓ · V OSS-first (harfrust/skrifa/google-fonts repo; no scratch shapers) ✓
· VI light pipeline only; commits local ✓ · AI-ready: font registration/selection all bus-reachable
(`updateShape({fontFamily})`; registry exposed via editorState wrapper) ✓.

## Waves (= tracker tasks #3–#8)

**W1 — Engine registry + upload (US1 spine)**: R1 registry + probe report; `register_font` export;
JS fontRegistry.ts + doc font-table replay (R2); upload entry in the picker v0; FontFace edit-preview
(R8); persistence round-trip incl. reopen.
**W2 — Device fonts real (US1)**: blob() → register; replace the inert deviceFont/"d:" path
(model cleanup); permission UX + revoked fallback.
**W3 — Google catalog + picker (US1)**: curated Arabic catalog JSON (committed); fetch+cache;
full picker UI (search/sections/lazy previews/badges) replacing the `<select>`.
**W4 — Weight slider + tracking + export parity (US2+US3)**: svg.rs weighted/bidi export pipeline +
outline.rs weight + JS cache keys (R5); slider gated by wght axis; tracking via join-safe boundaries
(R6).
**W5 — Harakat v1 + warn wiring (US4)**: mark mask render/export options (R7); per-shape
harakat fields through the bus; "بدون كشيدة" warn badge + first-use toast (R1 report).
**W6 (tracker #8) — Verification + handoff**: per-wave Playwright specs consolidated; QA visual
matrix (each source × shaping × export); docs/backlog/board sync; Ahmed gate = the calligraphic
poster session (US5) on his return.

## Risks

| Risk | Mitigation |
|---|---|
| harfrust/skrifa parse divergence on exotic fonts | dual-parse at registration; reject with a clear UI error, never per-call crashes |
| Google raw URLs drift (repo paths) | curated catalog pins exact commit-ish URLs + runtime 404 → badge + retry path; catalog regeneration documented |
| Device permission revoked / non-Chromium | graceful fallback to Amiri + chip (R2); picker section hidden when unsupported |
| Export pipeline switch (svg.rs) regresses kashida exports | golden SVG fixtures before/after at 400 (must be identical) + new 700 fixtures |
| Tracking breaks joining on edge scripts | flag-derived boundaries only + failure-direction tests on ligature-heavy samples |
