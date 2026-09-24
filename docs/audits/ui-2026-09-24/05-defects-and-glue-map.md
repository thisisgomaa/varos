> **Status:** reference — UI audit, 2026-09-24 (input to the UI system spec).

# 05 — Consolidated defect backlog + the glue map

Auditor: Claude subagent, read-only. Branch `claude/sweet-cerf-1sg30t` at `4821cf0` (57 commits ahead of `origin/main`). Sources: `docs/PAINS_LOG.md`, `docs/audits/2026-09-24-ASTRA_USER_TEST.md`, `docs/foundation/work_orders/*` and `reviews/*`, `docs/foundation/GATE_LOG.md`, `docs/foundation/STATUS.md`, and a comment/`allow`/`unsafe` scan of `varos-app/src/**` and `varos-core/src/**`. I built nothing, ran no tests and opened no window. Line numbers are on `4821cf0`.

## Executive summary

1. The Astra list is mostly done: 8 of 11 findings (F01, F04–F10) are fixed on this branch. None of them is on `main` yet. Batch 1 (F01, F04–F07, F09) has had **no independent Codex review** (GATE_LOG:365).
2. F02 (new tab) and F03 (Export) are the two open P1s. S1 (in flight) and S6-B (not started) own them. F11 (small grey text) is queued as QW6, which has not started.
3. I found one new, simple bug. The ☰ menu rows New / Open / Save / Export do nothing when you click them, because the code throws the click away (`ui.rs:3470-3474`). The Share and Export buttons are the same. PAINS_LOG P2 marked this as "refuted", but that check only looked at whether the menu opens.
4. There are almost no debt markers: 0 TODO/FIXME/HACK. Debt hides in prose instead ("for now", "stub", "MIRROR", "Ahmed 07-07"). 78 comments name Ahmed. Most of them justify a one-off rather than a rule.
5. The biggest source of glue is values that live in many places. There are two colour homes (`shell/tokens.rs` and 11 overlay colours in `varos-core/src/scene.rs`), with two different "GUIDE" colours. I counted 43 hard-coded corner radii, 28 colour literals and 75 font-size literals (12 sizes).
6. Keyboard shortcuts are defined in 5 files. The artboard name has 3 inline editors that follow 3 different rules. Two of them save an unchanged name, which marks the file unsaved.
7. `cursors.rs` (1,244 lines) is a junk drawer: cursors, the Win32 window frame, the screen eyedropper and mouse polling. 12 of its 14 `unsafe` blocks have no `SAFETY` comment.
8. The sandbox "fake panels" (`shell/registry.rs`, fake values like "266" and "F0B429") are still compiled in as the fallback body for any panel.
9. The docs lag the code. Five branch merges (S1-A, S3-A, S6-A, QW1, QW3) are not in GATE_LOG or STATUS. The STATUS `unsafe` count (27) is stale; I measured 29. PAINS_LOG has at least 5 stale rows.
10. Fix first: honest chrome (every control either works or says why not), one rename component with core-owned guards, "a no-op never marks the file unsaved", and one token home.

## Measurements (all measured on `4821cf0`)

| Metric | Value |
|---|---|
| `ui.rs` / `editor.rs` / `main.rs` lines | 6,533 / 4,892 / 1,854 (STATUS says 5,826 / 4,537, measured 2026-09-23) |
| `TODO`/`FIXME`/`HACK`/`XXX`/`kludge`/`workaround` in app+core | **0** |
| "for now" comments | 5 — `ui.rs:2414`, `ui.rs:3408`, `model.rs:359`, `model.rs:442`, `scene.rs:294` |
| Stub/fake/placeholder/legacy/stand-in/mirror words (per file) | ui.rs 19 · model.rs 19 (legacy migration, legit) · lifecycle.rs 15 (S1-A stub) · durable.rs 14 (fsync fallback, legit) · registry.rs 13 (fake panels) · editor.rs 10 · cursors.rs 7 · main.rs 6 |
| `#[allow(` in varos-app | **30**: 10 `dead_code` S1-A stubs (ui.rs 7, workspace/app_command/lifecycle 1 each) · 9 platform `dead_code` cfg_attr · 8 `too_many_arguments` (all ui.rs) · 2 `deprecated` (main.rs:2 whole-file, :802) · 1 test-only. Core: 3 (clippy) |
| `unsafe` in varos-app | **29** code sites (+1 comment): single_instance 13, cursors 14, durable 1, mac_menu 1. 27/29 are Windows-only. `// SAFETY:` on 16/29; cursors.rs 2/14 |
| Comments naming Ahmed | 78 — ui.rs 33, boxtree.rs 27, tokens.rs 6, registry.rs 2, chrome/main 1 each; core 8 |
| Colour literals outside `tokens.rs` (app) | 28 (ui 22, boxtree 3, cursors 2, main 1) + 11 `Rgba` consts in `varos-core/src/scene.rs:15-26` |
| `CornerRadius::same(<digit>)` literals | 43 (ui 37, boxtree 5, registry 1). Values 1×2, 2×19, 3×7, 4×5, 5×2, 8×4, 10×2, 16×2 → **32 are neither 3 nor 8** |
| Font-size literals in ui.rs | 75 sites, 12 sizes (9.5 … 31) — no type-scale token |
| `FAINT` (3.26:1 on PANEL) uses | ui 22, boxtree 4, registry 4 — incl. informational status-strip text `ui.rs:3624,3659` |
| Files defining shortcuts | 5 — `main.rs:198` (32 key strings), `ui.rs:925` (31 `egui::Key`), `chrome.rs:235-300,324`, `mac_menu.rs:16`, `tokens.rs:16` |
| UI `Op` variants → `EditCommand` variants | 48 (`ui.rs:95-148`) → 56, translated in `apply_ops` `ui.rs:5768` |
| Clickable controls that do nothing | 6 (☰ New/Open/Save/Export, Share, Export) + the ⌘K pill promise |
| Inline editors for an artboard name | 3 (`ui.rs:4505`, `:5125`, `:5433`) |
| In-flight worktrees | 9 (S1-B/C/D, S3-B/C, S5-B/E, QW4, QW5) |

## A. Astra findings — status (not re-listed as open)

| ID | Status | Evidence |
|---|---|---|
| F01 quit guard (P0) | DONE on branch | `953bca2`; no Codex review; hand test = batch 2 #1–2 |
| F04 clipboard, F05/F06 zoom, F07 Direct inspector, F09 duplicate board | DONE on branch | `0e369a6`, `389e542`, `9d0665e`, `abfbaa4`; no Codex review (GATE_LOG:365) |
| F08 thick-stroke hit | DONE on branch | QW1 `b9a8b2c`, reviewed; review P3 follow-ups are open (D16–D17) |
| F10 rename | DONE on branch | QW3 `3c208fa`, reviewed; P2-3, P3-3 and P3-4 are open (D12–D14) |
| Observations: Pen width 2, board-click dirty | DONE | `a1e0e3c`, `83363df` |
| F02 new tab · F03 Export | IN-FLIGHT | S1-B/C/D (tabs, New, honest Export); S6-B later (S6-A writer merged `9bd3389`) |
| F11 small grey controls | QUEUED | QW6, not started |
| ⌘K pill | IN-FLIGHT | QW7 was folded into S1-C |
| Control-bar placement · accessibility tree | OPEN | D20 (P8 + QW8) · D21 |

## B. Open defect table (deduplicated)

Sev = user impact. "Owner" = the piece that fixes it, if one exists.

| ID | Source | Symptom | Sev | Area | Evidence | Owner |
|---|---|---|---|---|---|---|
| D01 | Astra F02 | `+` makes a label, not a document; New does nothing | P1 | tabs | `ui.rs:3444-3448` pushes `"Untitled-N"` only | S1-B/C/D |
| D02 | Astra F03 | Export button and ☰ Export… silent | P1 | top bar | `ui.rs:3411` rect used only for caption exclusion; `:3474` click discarded | S1-C (honest) → S6-B |
| D03 | **new** (this audit) | ☰ New/Open/Save rows do nothing on click | P2 | top bar | `ui.rs:3470-3472`: `menu_row()` returns `bool`, and the result is dropped. PAINS_LOG:189 "refuted" is wrong | S1-C must cover it |
| D04 | STATUS | Share button dead | P3 | top bar | `ui.rs:3410` | S1-C (disabled + reason) |
| D05 | Astra obs. | Search pill advertises ⌘K; no path | P3 | top bar | `ui.rs:3412`, `:3215`; no `KeyK` in `apply_key` | S1-C (QW7) |
| D06 | Astra F11 | Info text in FAINT fails 4.5:1 (3.26 / 2.98) | P3 | tokens | `tokens.rs:33-34`; status strip `ui.rs:3624,3638,3659`; Pathfinder caption | QW6 (queued) |
| D07 | PAINS P13 | Open-path round caps are 24-gons (~14 px facets at 4000 %) | P3 | render | `tess.rs` `stroke_cap` | QW4 (in flight) |
| D08 | STATUS gaps | Mac Edit menu lacks Select All / Deselect / Delete | P2 | menus | `chrome.rs:245-259` | QW5 (in flight, APPROVE W/ NITS) |
| D09 | STATUS gaps | Mac menus lack New, Export, Duplicate | P2 | menus | `chrome.rs:235-300` | S1 / S6 / Needs Ahmed |
| D10 | STATUS gaps | Mac never remembers window size/position | P3 | host | `main.rs:450-453` `if cfg!(macos) return None` | QW2-lite — **unblocked** (S3-A merged `580436f`) |
| D11 | STATUS gaps | Screen eyedropper off on Mac | P3 | tools | `cursors.rs:1022`, `ui.rs:2414` | deferred (Needs Ahmed) |
| D12 | QW3 rev. P3-4, GATE_LOG:373 | Board name: Esc commits; unchanged/empty name still bumps `rev` → file shows unsaved | P2 | rename | `ui.rs:5153` (always commits), `ui.rs:5439-5441`; core `editor.rs:2214` `ab_rename` has no guard | none |
| D13 | QW3 rev. P2-3 | `RenameNode` on a path leaf writes an invisible `Node::name`; F10 can come back through any new caller | P2 | core | `editor.rs:4505` (uses `trim`, not `clean_name`); choice made in `apply_ops` | none |
| D14 | QW3 rev. P3-3 | Rename field can reopen after Undo when its row disappeared without a press | P3 | layers | no `rename = None` after the row loop (`ui.rs:4325`) | none |
| D15 | QW3 rev. P3-6 | Escape on an open menu also deselects on canvas | P3 | menus | `ui.rs:1652` closes but does not consume (reported; not re-verified) | none |
| D16 | QW1 rev. P3-1 | Hover hit 13.5 ms at 25k segments; `transform_hit` calls `path_under` twice per hover | P2 perf | core | `main.rs:94-97`; no bbox reject | P11.3 "measure first" |
| D17 | QW1 rev. P3-3 | Anchor-drag snap samples 25 points; hit uses the exact curve → they disagree on curves | P3 | snap | `editor.rs:2972` vs `model.rs:2006` | none |
| D18 | GATE_LOG:209 | Scene F cost (500 translucent same-colour strokes = 1,000 draw calls) unmeasured | P3 perf | render | GATE_LOG:209 | none |
| D19 | PAINS P12 | Pen on a middle anchor deletes it and **opens** the path | P2 | pen | `tools/pen.rs:38-41` | Needs Ahmed |
| D20 | PAINS P8 + Astra | Control bar width jumps; Fit puts art under the bar and rail | P2 | canvas | `ui.rs:3730-3731`, `main.rs:347` | QW8 after S1; anchoring = Ahmed |
| D21 | Astra obs. | Canvas controls invisible to assistive tech | P2 | a11y | Astra §B note | none (QW §8 out of scope) |
| D22 | Astra #10 | No mask authoring route | feature | masks | no mask command in `command.rs` | masks stages 3–6 |
| D23 | GATE_LOG | Board-handle drag without movement bumps `rev` | P3 | core | GATE_LOG:373 | none |
| D24 | code | Panic breadcrumb writes a cwd-relative `target/panic.txt` (`/` in a bundle) | P3 | host | `main.rs:719-721` | none |
| D25 | QW3 rev. P3-2 (fixed) → watch | Rename trims bidi marks in `clean_name` only; `layer_rename`/`ab_rename` still use `trim` | P3 | Arabic | `editor.rs:2216`, `:4507` | none |
| D26 | process | Batch 1 has no Codex review; 5 merges missing from GATE_LOG/STATUS; STATUS `unsafe`=27 stale | P1 (merge) | docs | GATE_LOG:365; `grep -c QW1 GATE_LOG` = 0 | moderator |
| D27 | docs | PAINS_LOG stale rows: :84 A7, :85, :196, :198, :199; :189 P2 | P3 | docs | QW §2 table + D03 | moderator hygiene |

## C. Glue map — every special case, one-off, hard-coded value and dead path found

Verdict: **R** = make it a rule (lint/test), **C** = make it a component, **D** = delete.

| # | Where | What / why it exists | Class | Verdict |
|---|---|---|---|---|
| G01 | `ui.rs:3444-3461` | Fake tabs: a `Vec<String>` of labels, not documents. From the pre-DFS "look first" stage | Glue (dead path) | **D** — S1-C/D |
| G02 | `ui.rs:3408-3412`, `:3470-3474` | "Visual MIRRORS for now": look landed before wiring (comment at :3408) | Glue | **R1** + S1-C |
| G03 | `ui.rs:831-835`, `:1169-1196`; `workspace.rs:18`; `app_command.rs:13`; `lifecycle.rs:9,81-126` | 10 `allow(dead_code)` and a 9-`unreachable!` stub, from S1-A | Glue (planned) | **D** by S1-B/D (R8) |
| G04 | `shell/registry.rs:61-312` | Sandbox dummy bodies (fake "266", "F0B429", "85 %"). Still the fallback in `boxtree.rs:339,450` when the host returns `false` (`ui.rs:1401`). Swatches/History/Assets exist but are hidden (`registry.rs:23`, Ahmed 07-08) | Glue (dead in prod) | **D** bodies; keep ids/titles/min sizes (R12) |
| G05 | `shell/boxtree.rs:130` `to_json` | Only a test calls it; the layout is never persisted | Dead path | **D** or wire with Workspaces |
| G06 | `ui.rs:4505-4533` / `:5125-5157` / `:5433-5442` + `editor.rs:2214` | Three artboard-name editors with three rule sets (Esc cancels vs commits; empty/unchanged ignored vs committed) | Glue (dup widget) | **C** `inline_rename` + guards in core (R5, R6) |
| G07 | `editor.rs:4505`; `ui.rs` `apply_ops` `LayerRename` arm | The Path-vs-Node name choice lives in the app | Architecture | **R**: core resolves (QW3 P2-3) |
| G08 | `ui.rs:4596-4607` | Hand-made double-click: 0.4 s literal keyed on (id, sec), because egui missed `click_and_drag` rows (P4, 07-11). QW3 later found the real cause in core | Glue | **C** one `double_click()` using the OS interval, or **D** after a test |
| G09 | `ui.rs:5803`, `:5816` | `zone` 0/1/2 magic ints → `DropPos`, duplicated | Glue | **D** (carry `DropPos` in the `Op`) |
| G10 | `ui.rs:95-148` + `:5768` | A second command vocabulary (48 `Op`) translated to `EditCommand` | Architecture | **R**: panels emit `EditCommand` / a `ViewCmd` |
| G11 | `varos-core/src/scene.rs:15-26` vs `tokens.rs:37-43` | Two colour homes. `ACCENT` duplicated. `GUIDE` is pink in tokens (unused, "keep, Ahmed 07-08") but cyan in core. `SNAP_GUIDE` green and `SEG_HI` cyan make second and third "selection" hues | Inconsistency | **R2** — the core overlay takes a palette from the app |
| G12 | `ui.rs:3282-3285` | Menu metrics in ui.rs, not tokens; `MENU_R = 4` is outside the 3/8 law (07-07) | Glue | move to tokens |
| G13 | 43 radius / 28 colour / 75 font-size literals (see measurements) | Hand-painted widgets pick values inline | Glue | **R2/R3** |
| G14 | `ui.rs:2961-3095` | Splash: static card for 1.55 s at every launch (Ahmed 07-08). Radii 16/10. Leftover `ca = 1.0` fade variable; doc comment still says "Fades into the editor" (`:2971`) | Inconsistency ("answers instantly") | **D** when S2 Start lands, or show only while loading |
| G15 | `ui.rs:3517`, `:3521` | Indentation done with leading spaces inside labels | Glue | **C** `check_row(indent)` |
| G16 | `cursors.rs` 435-497, 499-748, 1022 | Cursors + Win32 window frame (`nccalcsize`/`nchittest`/cloak/brush) + screen eyedropper + mouse polling in one file | Architecture | split modules (R11); SAFETY lint (R9) |
| G17 | `cursors.rs:1022`, `chrome.rs:104`, `main.rs:451`, `ui.rs:2403-2414`, `tokens.rs:16` | Platform differences scattered as ad-hoc `cfg!` consts and tooltip text | Missing rule | **R7** one `PlatformCaps` |
| G18 | `main.rs:733-751`, `ui.rs:5854-5874`, `cursors.rs:229`, `main.rs:1404` | Dev tools in the shipping binary: `--dump-cursors`, `--dump-tool-icons` (hard-codes panel `#1f1f22`, off-ramp), `--preview`, `VAROS_CURSORS_AI`, `VAROS_PERF` | Glue | **R14** `examples/` or `cfg(debug_assertions)` |
| G19 | 5 shortcut files (measurements) + `ui.rs:4065` "(X)", `:4104` "(D)" | Each surface restates the key | Glue | **R4** one table |
| G20 | `editor.rs:354` vs `scene.rs:979`; `editor.rs:2972` vs `model.rs:2006` | Duplicate Liang–Barsky helper (forced by QW1's Owns); sampled vs exact curve distance | Glue | **R13** one `geom` home |
| G21 | `editor.rs:774` `7.0/ppu`, `:794` `22.0/ppu` | Literal screen tolerances beside the named ones at `editor.rs:11-18` | Glue | named consts |
| G22 | `editor.rs:2092`, `ab_rename`, board-handle drag | Each site patches "don't bump `rev` here" by hand (Astra 09-24) | Missing rule | **R5** |
| G23 | `boxtree.rs` 5× `is_board()`, `:180`, `:684` | The Board is special-cased, although UI_DIRECTION says "No box is special-cased in code" | Inconsistency (law drift) | amend the law (Board = one non-tabbable box type) |
| G24 | `ui.rs` 8× `too_many_arguments` (416, 1689, 3346, 3715, 4173, 4208, 5387, 5540 — the last with no reason) | "Split deferred with ui.rs" | Architecture | panels take one context struct |
| G25 | `main.rs:2` `#![allow(deprecated)]` | A whole-file allow for winit 0.30's closure loop | Glue | scope it to the item; migrate to `ApplicationHandler` |
| G26 | 78 "Ahmed MM-DD" comments (e.g. `ui.rs:1404,1412,3682`, `boxtree.rs:684`, `tokens.rs:57-59`) | Decisions live in comments; few are pinned by a test | Missing rule | **R10** |
| G27 | `ui.rs:3597-3609` | Status strip `exact_size(31.0)`; its doc comment says "h 25" | Inconsistency | tokens + doc fix |

**What S1 must leave behind (top bar is being replaced, not audited as broken):** zero discarded clicks in `build_topbar` (G02, D03), Share/Export/⌘K disabled with a reason, the 10 S1-A `allow(dead_code)` gone (G03), tabs read from `Workspace` (G01), the `shortcut()` path unified with `apply_key` (a start on G19), and no new radius/colour literal in the new tab strip.

## D. Needs Ahmed — consolidated (each has a working default unless marked "eye")

| # | Item (source) | Recommended default |
|---|---|---|
| 1 | P12 Pen on a middle anchor (PAINS, QW §5) | Illustrator: Pen removes it and joins the neighbours; Delete keeps A32 |
| 2 | P8 control-bar anchoring (QW §5) | (b) fixed left edge |
| 3 | Duplicate menu row (QW §5) | nothing now; later Paste in Front/Back |
| 4 | QW6 look; resting icon grey (QW §5, review) | keep MUTED; FAINT→MUTED only on info text |
| 5 | Marquee vs painted band; rotate ring vs own thick band (QW review) | marquee touches the band; own band never blocks rotate |
| 6 | Mac window memory (QW §5) | QW2-lite now (unblocked) |
| 7 | Does activating a board count as unsaved? (GATE_LOG batch 1) | no (S1: view vs content dirty) |
| 8 | Mac screen eyedropper permission prompt (QW §5) | defer |
| 9 | S1: red light / ⌘W = whole Quit until the Start page exists | yes |
| 10 | S1: fill/stroke/weight per tab or shared | per tab |
| 11 | S1: tab overflow | active tab visible + Ctrl+Tab |
| 12 | S1: Share shown disabled | yes |
| 13 | S2/3: red light keeps the app running (spec §2) | defer |
| 14 | S2/3: Recovery switch location | Document section, "Recovery (all documents)" |
| 15 | S4/6: export name when the doc came from a `.pdf` | `<name> export.pdf` |
| 16 | S5: accept ADR-0008; bump format for every feature; broken-mask file; run the corpus command on the Mac | accept; yes; refuse unless the corpus hits; required |
| 17 | QW3: an emptied rename field keeps the old name (Illustrator claim unverified) | keep old |
| 18 | QW5: "Delete" vs Illustrator "Clear" | Delete |
| 19 | **New (this audit)**: radius law — UI_DIRECTION "0–4 px, panels square" vs CLAUDE.md "3/8" vs `RCAP 11`, splash 16/10 | 3 / 8 / 11 (pill only), written into UI_DIRECTION |
| 20 | **New**: seam — UI_DIRECTION "~6 px" vs `SEAM_GAP 12` (07-04) | 12, update the doc |
| 21 | **New**: 1.55 s splash vs "answers instantly" | remove when the Start page lands |
| 22 | **New**: guide colours — tokens pink vs core cyan + green | one set, in tokens |
| 23 | Eye: hand-test batches 1 (16), 2 (10), 3 (7) | — |
| 24 | Eye: #2 clipped icons; P4 rename (now QW3); A7 rotation feel; A3 "which menu is too long?"; P5 "how did you open it twice?" | — |
| 25 | Eye: `codex/p6-header` fate (charter precondition for F5; branch not in this checkout) | — |
| 26 | Eye: canvas artboard name (double-click, size chip); A14 bar, A16 icons, A17 picker, A19 stroke options, "Codex colours" | — |
| 27 | Program: cargo-audit bumps; MCP ADR; online (ADR-0001 amendment); Compositor picks | — |

## E. Rules this suggests (each one testable)

1. **R1** Every clickable control either dispatches a named command or is drawn disabled with a reason. A test walks the ☰ menu, top bar and native menu and fails on a click that changes nothing.
2. **R2** Colours, radii, sizes, fonts and timings appear only in `shell/tokens.rs`. The core canvas overlay receives its palette from the app. A gate greps for `Color32::from_*`, `CornerRadius::same(<digit>)` and `FontId::*(<digit>)` outside tokens and fails.
3. **R3** Corner radius ∈ {`R`, `RBOX`, `RCAP`} and font size ∈ a ≤ 6-step scale. Any text that carries information has ≥ 4.5:1 contrast (a token test computes it). FAINT is only for disabled or placeholder text.
4. **R4** Every shortcut is declared once in one table. `apply_key`, the native menu, egui key mapping, labels and tooltips all derive from it. No `"KeyX"` string literal appears outside that table.
5. **R5** An `EditCommand` that changes nothing does not bump `rev` and adds no undo step. A property test runs every command with identity arguments.
6. **R6** Text entry goes through one inline-edit component: Enter or blur commits, Escape cancels, an unchanged value is a no-op. No raw `TextEdit::singleline` appears outside it.
7. **R7** Platform differences come from one `PlatformCaps` value. The UI renders "unavailable on this platform" from it. No `cfg` in panel code.
8. **R8** Every stub, allow or workaround carries `DEBT(<piece-id>)`. A gate lists them, and closing a piece fails while its markers remain.
9. **R9** `clippy::undocumented_unsafe_blocks = "deny"` in varos-app.
10. **R10** A design decision gets a dated entry in `docs/` and an id (e.g. `UI-D-0707-3`). The code comment cites the id, and a named test pins the behaviour.
11. **R11** A module owns one concern, and new files stay ≤ 1,500 lines (`cursors.rs` ≠ window frame ≠ screen sampling).
12. **R12** No production path renders fake data. Sandboxes live in `#[cfg(test)]` or `examples/`.
13. **R13** Hit distance, rect–segment tests and curve-nearest exist once in `varos-core::geom`. Callers never re-implement them.
14. **R14** Dev entry points (dump flags, A/B env switches) compile only in debug builds or examples.

## F. What to fix first (not already owned by an in-flight piece)

Precondition: land the in-flight S1 / QW4 / QW5 work, and run the missing Codex review on batch 1 (D26).

| # | Fix | Closes | Size |
|---|---|---|---|
| 1 | R1 dead-chrome test + wiring or disabling the ☰ rows inside S1-C | D02–D05, G02 | S |
| 2 | One `inline_rename` component; rename guards (`clean_name`, no-op, Esc) in core; `layer_rename` resolves path leaves | D12–D14, D25, G06–G08 | M |
| 3 | R5 no-op commands never bump `rev` (property test over `EditCommand`) | D12, D23, G22 | M |
| 4 | One token home: core overlay palette, menu metrics, a single guide colour, radius and type scale | G11–G13, G27 | M |
| 5 | QW6 + move status-strip info text off FAINT | D06 | S |
| 6 | Hover perf: bbox reject in `path_under`; ring distance before `path_under` in `transform_hit` | D16 | S |
| 7 | QW2-lite window memory; drop the cwd panic breadcrumb; dev flags out of release | D10, D24, G18 | S |
| 8 | Delete the registry fake bodies and dead `to_json`; test that the host renders every `DOCKABLE` | G04, G05 | S |
| 9 | Split `cursors.rs`; add `PlatformCaps`; turn on the SAFETY lint | G16, G17, R9 | M |
| 10 | One shortcut table (after S1-D removes `shortcut()`) | G19 | L |

## Limits (honest)

- I opened no window. D15 and the double-click need (G08) are taken from reviews and not re-verified. D03 is verified by reading the code only (the returned `bool` is dropped): a click cannot reach any handler.
- `codex/p6-header` is not in this checkout, so I could not inspect it. Counts are `grep`-based. The literal counts include some tiny 1–2 px glyph radii that may deserve their own token rather than 3/8.
