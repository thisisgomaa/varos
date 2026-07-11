# Varos Foundation Charter — Architecture & Repository Foundation Program

**Status: ACCEPTED by Ahmed (product owner), 2026-07-11.** This charter is program authority per §3.2. History: v1 planner draft → planner self-review (7 defects fixed) → Codex counter-review (8 points, all incorporated) → v2 accepted.
**Author:** Claude (planner). **Counter-reviewer:** Codex. **Date:** 2026-07-11. **Baseline:** `05b6dc7` (F1 merged).
**Inputs:** `docs/audits/2026-07-11-CODEX-FULL-PROJECT-AUDIT.md`, `docs/audits/2026-07-11-CLAUDE-COUNTER-REVIEW.md` — the program's risk register; committed on acceptance so no clone loses them — plus `INVENTORY.md`, `DEPENDENCY_MAP.md`, `OWNERSHIP_MAP.md`, `NOTES_FOR_CHARTER.md`, and Ahmed's directive: *organize the foundation before features, safety work, or UX repair.*

## 0. Definitions

- **First-party docs:** tracked `.md`/`.html` files authored for Varos. Excludes `varos/vendor/**` (upstream README/licenses/tests are vendor material, never stamped).
- **Stamp:** a banner as the first body line of a doc: `> **Status:** current | historical | reference — <one-line reason / superseded-by link>`. The machine-greppable marker is `**Status:**`; the denominator for "docs stamped" is the first-party doc count derived from `INVENTORY.md`.
- **Link check:** the repo script `tools/check_links` (created in F2a.1) resolving every relative link and heading anchor across first-party docs; "links resolve" in any gate means this script exits clean.

## 1. Goal

Ahmed's words, distilled: **the codebase and repository must be organized well enough to build on for decades without turning into chaos.** This program is not features, not UX repair, not data-safety — those are parked with explicit flags (§8). The end state is operationalized by F8's exit exercise (§5), not by slogan: a stranger can clone Varos, know what is true, run the gates, and make a scoped change without collateral damage.

## 2. Roles (permanent, person-independent)

| Role | Holds it today | May be held tomorrow by |
|---|---|---|
| Product owner | Ahmed | any human owner |
| Planner / reviewer | Claude (planning session) | any competent agent or human |
| Implementer | Codex + production session | any competent agent or human |
| Release maintainer | *(vacant — filled on the parallel governance track, §9.5)* | any human with repo admin |

Rules: the planner does not implement; the implementer does not merge; every merge passes a gate review; product/visual calls belong to the product owner alone. Truth lives in the repository (ADRs, work orders, tests, STATUS, GATE_LOG), never in any individual's memory or chat history.

## 3. Authority and precedence (ends the "five documents claim to be law" fracture)

Once this charter is accepted:

1. **ADRs** (`docs/adr/`) — accepted decisions. Highest authority. After acceptance an ADR is immutable **except**: appending a `Superseded-By:` line, and typo-level corrections that cannot change the decision's meaning. Any real change = a new superseding ADR.
2. **This charter** — program scope, roles, gates.
3. **`docs/foundation/STATUS.md`** *(created in F2a.1)* — the single current-state page: active work order, last gate result, health numbers, trigger flags. Anything that contradicts STATUS is stale.
4. **Specs and plans** — valid only while referenced by an open work order or ADR.
5. **Everything else** — historical or reference; carries a stamp saying so (F2a.2 — historical **and** reference both get stamps).

Any document that self-declares authority outside this ladder is wrong by definition.

## 4. Program invariants (project law during every work order)

- **Behavior-preserving:** no user-visible behavior change unless the work order says otherwise and Ahmed approved it.
- **Gates green where it counts:** every branch tip submitted for gate review and every merge commit passes `cargo test --workspace` + `clippy -D warnings` + `fmt --check`. (Per-intermediate-commit greenness is desirable, not enforced — CI checks tips. Large stages are split into separate branches/orders instead.)
- **Golden compatibility:** the existing golden fixture tests stay green at every gate. The **full round-trip law** — load fixture → save to bytes `A` → reload `A` and assert `Document` equality → save again to `B` and assert `A == B`; never compare current bytes against the original fixture bytes (migration legitimately changes representation) — is implemented as a test in **F3** and becomes a standing invariant from F3 onward.
- **Evidence standard:** every claim in any deliverable carries file:line or a command + output. Fabricated or unverifiable citations fail the gate (established in F1).
- **Gate trail:** every gate review is an entry in `docs/foundation/GATE_LOG.md` ending with a sign-off line: `Sign-off: <role> — PASS|FAIL — <date>`. A merge without a gate entry violates §2.
- **History is preserved:** historical docs are never rewritten — they receive a stamp and keep their text (including old paths and old extensions) verbatim.
- **Small reviewable branches:** one work order = one branch = one gate entry. No `git add -A`; no concurrent orders touching the same files.

### 4b. Session execution policy (operational defaults set by the session operator — not permanent project law)

- Resource budget: sequential work; gentle builds (today `-j 4`); nothing heavy while the machine is in interactive use.
- Verification subagents run on cheaper model tiers than the lead session unless correctness-critical (operator rule, 2026-07-11).

## 5. Work orders

Owners: **P** = planner authors/gates, **I** = implementer executes, **A** = Ahmed decides.

| # | Work order | Owner | Deliverable | Exit gate |
|---|---|---|---|---|
| F1 ✅ | Inventory & classification | I (merged `05b6dc7`) | INVENTORY / DEPENDENCY_MAP / OWNERSHIP_MAP + CI auto-triggers | 160/160 classified; gate recorded in `GATE_LOG.md` §F1 |
| F2a.1 | Policy scaffolding | P authors, I executes | `docs/adr/` skeleton + ADR-0001..0007 drafts (§6 backlog); `STATUS.md` incl. §8 trigger flags; `tools/check_links` script | ADR drafts complete; STATUS live; link check runs clean on the untouched tree |
| F2a.2 | Stamping | I | Every first-party doc carries a `**Status:**` stamp (current, historical, and reference alike) | `grep -l '\*\*Status:\*\*'` count == first-party denominator; zero self-declared laws outside §3 |
| F2a.3 | Current-doc corrections | I | In **current** docs only: personal/temp paths → repo paths; stale claims flagged by the audits fixed (README test counts, CONTRIBUTING CI claim, Constitution alignment per ADR-0001/0007); `.varos` in current docs verified 0 (already true 2026-07-11) | Link check clean; the audits' doc-truth P0 items each closed or stamped |
| F2a.4 | Vendor contract | I | `VENDOR_PATCHES.md` promoted from DEPENDENCY_MAP §4 + a diff-check script vs upstream `egui_tiles 0.16.0` | Script proves exactly the five documented files diverge |
| F2b | Physical doc layout | A approves layout first, then I | Historical/reference docs relocated (`git mv` + the minimal link updates the move forces) | Link check clean post-move; `git log --follow` intact |
| F2c | Dependency-direction CI test | I, immediately after ADR-0005 acceptance (before F3/F4) | CI assertion of ADR-0005 allowed/forbidden edges | Test demonstrably fails on a wrong-edge demo patch; passes on tree |
| F3 | Characterization tests | I | Tests pinning current behavior at the seams to be cut: op dispatch, menu/shortcut parity, gesture/history lifecycle; **plus the §4 golden round-trip test**. No production behavior change; test-only scaffolding (`#[cfg(test)]`, `pub(crate)` seams, or mechanical extraction) is allowed and must be declared in the order | New tests green; each shown once to fail when its pinned behavior is mutated; behavior unchanged |
| F4 | Command boundary — two levels | P+A via ADR-0002, then I | **`EditCommand` in `varos-core`** (deterministic Document/Editor edits, history/undo, headless execution) + **`AppCommand` in `varos-app`** (New/Open/Save/Export, panels, window, dialogs, side effects; delegates edits to `EditCommand`). Closed enums are explicitly NOT the plugin API — plugin/AI adapters, versioning, and query contracts are future work recorded inside ADR-0002 | **Zero direct document/editor mutations from UI; every edit passes through `EditCommand`, every application effect through `AppCommand`**; F3 suite green |
| F5 | Split `ui.rs` by ownership | I, each stage its own branch/order | Modules per OWNERSHIP_MAP §5 regions behind the existing `Ui` facade | Per stage: gates + F3 green; `ui.rs` shrinks monotonically; no region moves without a map row |
| F6 | Split `editor.rs` by domain | I, staged as F5 | Modules per OWNERSHIP_MAP §6 behind the `Editor` facade | Same as F5; `live_transform` 31/31 stays green |
| F7 | Dependency & metadata hygiene | I | `rust-toolchain.toml` + MSRV; `[workspace.package]`; `.gitattributes`; advisory triage doc; CI toolchain pinned; hostile-file hardening (§8); `varos-spike/` fate (A) | `cargo audit` triaged in writing; gates green; CI pinned |
| F8 | Architecture review & program close | P + I cross-review, A accepts | Maps regenerated; dashboard delta; deliberate-non-goals list; **change-impact exercise:** three scripted representative changes (add a menu command; add a tool; add a persisted `Document` field) traced end-to-end, each touching only its owned modules + tests | Both reviewers' sign-offs in GATE_LOG; Ahmed accepts; parked items re-triaged |

Sequence: F2a.1 → F2a.2/3/4 (parallelizable, disjoint files) → F2b → F2c → F3 → F4 → F5 → F6 → F7 → F8. F7 may overlap F5/F6 on disjoint files. **Precondition from NOTES_FOR_CHARTER:** the fate of `codex/p6-header` is decided before F5 starts.

## 6. Target architecture (proposal — each bullet lands only via its ADR)

- **Crate layout stays: 4 crates + vendored fork.** No new crates, no rewrite (both audits agree).
- **Dependency directions become enforced law:** DEPENDENCY_MAP §1 tables graduate to ADR-0005 + the F2c CI test — a wrong edge fails the build, not a review.
- **Two-level command boundary (ADR-0002):** `EditCommand` (core) + `AppCommand` (app) as specified in F4. Direct `ed.doc` writes from UI (today inside `apply_ops` at `ui.rs:5437,5446` and `set_stroke_width` at `ui.rs:5463`) end with F4.
- **`Ui` and `Editor` stay facades** — modules move behind them; public surfaces shrink later, deliberately, not during the splits.
- **Vendor fork contract (ADR-0006 / `VENDOR_PATCHES.md`):** upstream base `62ac747`, five patched files, rebase procedure, diff-check script.
- **Schema law resolution (ADR-0004):** today "single schema" is CLAUDE.md hard law while the Constitution defers it; the honest current state is "serde structs, not an introspectable registry." The ADR states what V1 promises and what is future.

**ADR backlog:** 0001 native GPU UI stack (records the 2026-06-27 pivot; drops Tauri/web-panels; CPU fallback = future option, GPU failure = readable error) · 0002 two-level application command boundary (`EditCommand` core / `AppCommand` app) · 0003 V1 container: `.vrs` extension + PDF with embedded JSON · 0004 schema policy V1 · 0005 enforced crate dependency directions · 0006 vendored `egui_tiles` patch contract · 0007 update policy: visible and user-controlled, silent auto-update rejected.

## 7. Health dashboard (updated in STATUS.md at every gate)

| Metric | Baseline `05b6dc7` | Direction |
|---|---:|---|
| `ui.rs` lines | 5,563 | ↓ (F5) |
| `editor.rs` lines | 4,533 | ↓ (F6) |
| Workspace tests | 223 | ↑ (F3) |
| `unsafe` sites (app crates) | 27 | = or ↓, all documented |
| Direct external deps | 23 | = or ↓ |
| `cargo audit` | 3 vulns + 3 unmaintained | 0 untriaged |
| `.varos` refs in **current** docs | 0 (verified 2026-07-11) | stays 0; the 51 refs in historical/reference docs remain there, under stamps |
| First-party docs stamped | 0 / denominator per §0 | 100% (F2a.2) |
| Link check | script absent | exists (F2a.1) and clean at every gate |
| CI | auto-triggers on; **runner blocked by account billing hold** (run `29162831354`: 2s, zero steps) | green required checks (needs Ahmed: GitHub verification + branch protection) |

## 8. Parked risk register (not forgotten — flagged)

The two audit reports in `docs/audits/` are the risk register (tracked on acceptance). **This table deliberately supersedes both audits' "trust repair first" (Phase 1) sequencing — Ahmed's 2026-07-11 directive: foundation before user-facing repair.**

Triggers are **dated boolean flags in `STATUS.md`**, flipped only by Ahmed (self-judged phrases like "real design work" become auditable the day he flips the flag):

| Parked item | STATUS.md flag that un-parks it |
|---|---|
| Close-without-save guard, autosave/recovery, save fsync/`ReplaceFileW` protocol | `flag.design-work-started` — and it then jumps the queue ahead of everything |
| Hostile-file hardening (recursion/cycle guards, decompress cap, `ids` re-derivation) | Scheduled inside F7 regardless; `flag.external-testers` forces it earlier |
| Fake tabs removal, dead-command wiring/hiding | `flag.release-milestone` |
| Mid-gesture undo corruption, Esc-cancel, DPI-change handling | `flag.dogfooding` |
| Product sequence: Masks 3-6 → PNG export → SVG export → SVG import (proposed by Codex 2026-07-11, planner concurs; the audit's §15 import-first order is superseded by this proposal) | Program close (F8) — **Ahmed ratifies the order then** |

## 9. Decisions Ahmed already made (recorded here, formalized as ADRs in F2a.1)

1. `.vrs` is the official V1 extension; historical docs keep `.varos` under stamps.
2. PDF + embedded JSON is the **V1** container — a version decision, not an eternal promise; formal lock lands with `VRS_FORMAT.md` after hardening.
3. Constitution amendments: Tauri/web-panels removed (native GPU UI); CPU fallback = future option, not a promise; silent auto-update **rejected** — updates visible and controllable.
4. Fake tabs: remove (when their flag fires); File menu wires to existing actions; Search/Share hidden until real.
5. GitHub organization + ≥2 human admins + 2FA + branch protection: yes, parallel track, does not block the program.

## 10. Mutual-review record and what remains for Ahmed

**Resolved in mutual review (2026-07-11):** command boundary = two levels (Codex's design, planner concurs — formalized in ADR-0002 for Ahmed's acceptance) · dependency-direction CI test = F2c, right after ADR-0005, not F7 · stamp-before-move (F2a before F2b) confirmed · golden round-trip law defined and scheduled (F3) · F2a split into four sub-orders · metrics given markers, denominators, and tooling.

**Ahmed decides (at acceptance):** ① accept this charter as authority; ② approve committing the two audit reports to `docs/audits/` as the tracked risk register; ③ (later, before F2b) choose the physical docs layout.
