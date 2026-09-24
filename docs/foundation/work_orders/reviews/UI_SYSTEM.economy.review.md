> **Status:** reference — independent spec review (economy), 2026-09-24.
# UI_SYSTEM.md — independent review: economy, build plan, "world standard"

Reviewer: Claude subagent. I only read; this file is my only write. I read CLAUDE.md, UI_DIRECTION, audits 01–05, DFS S1 §3–§5, S2+S3 §3.7/§4–§5, S4+S6 §3.3/§4–§5, FOUNDATION_CHARTER F5, OWNERSHIP_MAP §5 and the spec in full. I spot-checked `shell/tokens.rs`, `varos-app/Cargo.toml`, `Cargo.lock` and the egui 0.35 sources. I built and ran nothing. I computed the contrast ratios by hand from the `tokens.rs` hex values with the WCAG 2.x formula.

## Verdict — REQUEST CHANGES
1. The core is right and economical: one owner per value, Snap → Request, one command table, one kit, a headless `UiFrame`. Keep it. It needs no new dependency.
2. The plan is not what §1 and Q1 tell Ahmed. U0 is not "new files only", and the hot-file queue leaves out S4-A/B, S6-B/C and S2-E1, which own the same files and APIs (P1-1).
3. "≈24 agent-days" counts pieces, not effort. Five pieces are over the 1-day cap on their face. An honest figure is ≈30–34 agent-days, plus about 30 Codex reviews and 7 hand-test rounds (P2-4).
4. For an Arabic-first product that wants to be a "world standard", the spec says nothing about the things that are expensive to add later: strings, RTL, keyboard layouts and accessible names. All of these are cheapest to add inside the kit and the command table now (P1-2). Performance budgets, empty/busy/error states and undo rules per control are also missing (P2-5, P2-6).
5. About 2 agent-days can be deferred or dropped without losing a law; the missing pieces add back about 1½. The §1 summary must state the cost, what is left out, and that Arabic layer names will look broken (P1-3).

## Findings

### P1 — must change before the spec is accepted

**P1-1 · The hot-file queue is incomplete, and "U0 = new files only" is false.**
Evidence: U0 edits existing files: U0-A edits core `scene.rs`, `varos-render-wgpu` and `cursors.rs` (§5 U0 pieces). · U0-B adds SAFETY comments to `single_instance.rs`. S4-A owns that file (S4+S6 WO §4, S4-A *Owns*). · U0-C edits `ui.rs` `install_fonts`. · S6-B owns `shell/registry.rs` and `shell/boxtree.rs` (`PanelId::Export`, `reveal_panel`). It also owns the `P::Export` arm of the same `ui.rs:1332` host closure that U2-A deletes. · S6-C owns `build_topbar`, `mac_menu.rs` `sync_enabled`, and the `main.rs` `shortcut()` file chords. U1-B deletes `shortcut`. S6-C's parity test is designed to read "the real sources … not a separate declarative table". After U1, that table is the real source. · S4-A/B own the `main.rs` startup and `AboutToWait` hunks, and U5-B moves exactly that code into `route()`. · S2-E1 builds `start_ui.rs` "tokens only" before any kit exists. Yet §4 says Start is "built from kit pieces", and no piece migrates it. S6-B's `export_home.rs` has the same problem. · U3-E adds the `Editor::begin` assert and `live_rev` (core `editor.rs` / `scene.rs`), but the core row does not list it. · U1-A and U1-C edit `ui.rs` (hint literals, `wants_keyboard` at `ui.rs:1149`), but the `ui.rs` row does not list them.

Change (§5 queue): `main.rs`: S1-D → U1-B/C → **S4-A → S4-B** → S2-E2 → S3-F1 → S3-F2 → **S6-B → S6-C** → U4-C → U5-B. · `registry.rs`, `boxtree.rs`: **S6-B** → U2 → U4-A → U4-B → U6. · `ui.rs`: add U0-C, U1-A, U1-C, S6-B (host arm) and S6-C (topbar). · New row `single_instance.rs`: S4-A → U0-B. · Core row: add U0-A (`scene.rs`) and U3-E (`editor.rs` begin assert, `scene.rs` signature).

Other changes: U0 header: "**starts now** (new files + one install_fonts hunk + CI)" → "U0-C and U0-D start now (new files + one `install_fonts` hunk). U0-A starts after S1-D, U0-B after S4-A." · New §5 line: "When U1/U2 land first, amend the S6-B/S6-C WOs: Export implements `Panel`; Export is a `CommandId` row; `resolve` replaces `sync_enabled`; `reveal_panel` and `WindowCmd::FocusPanel` become one implementation." · New piece **U3-G (S)**: migrate `start_ui.rs` and `export_home.rs` to the kit.

**P1-2 · The kit and the command table hard-code strings, give no semantics, and do not fix the key type. An Arabic-first "standard" cannot add these cheaply later.**
Evidence: **Strings.** English literals sit at every call site: `CommandSpec.label: &'static str`, `Act::Off(&'static str)`, `button(label: &str)`, `section(title: &str)` and `Tip::Plain(&str)` (§3.2–3.3). · **Arabic user text.** Risk 2 defers Arabic, but user content already appears in the chrome: layer, artboard and document names, and Recents (05-D25 already handles bidi marks). egui 0.35 does no shaping, so Arabic names render as isolated letters in LTR order. · **Accessibility.** Risk 8 defers it. But egui 0.35 already pulls in `accesskit` (`Cargo.lock`) and has `Response::widget_info` (egui `response.rs:849`) and `Context::enable_accesskit` (`context.rs:3599`). Only the platform adapter is new: `egui-winit` feature `accesskit`, which brings in `accesskit_winit`. 05-D21 and 03-F-M4 are open P2s. · **Keyboard layouts.** L10's `Chord{key: Key}` does not say whether it matches physical or logical keys. On an Arabic layout, the V key types "ر". Today `main.rs:1278` happens to use physical keys.

Change: New law **L35**: "Every user-visible string comes from one id table (`ui/strings.rs`); no literal at a call site. User text is shown bidi-isolated. Layout code uses leading/trailing, never left/right, except on the canvas." Test: literal gate for `&str` arguments of kit calls, and a fixture of mixed Arabic/Latin names. · New law **L36**: "Every kit component reports `widget_info`: role, label from the table, and on/disabled state with its reason." Test: enable `ctx.enable_accesskit()` and walk the tree; no interactive node may be unlabeled. · Sharpen L10: "Chords match **physical** keys (winit `KeyCode`), so shortcuts work on Arabic and other layouts. Hints show the key cap." · Move Risks 2 and 8 into decisions Q6 and Q7 (see Needs Ahmed). · New piece **U0-E (S) spike**: render a fixture of real Arabic layer names in a kit label. `unicode-bidi` and `rustybuzz` are already in the lock through `usvg`. Report readable or not, with screenshots, before U3-C.

**P1-3 · §1 does not give the owner enough to decide.**
Evidence: §1 never states the total cost, the 7 hand-test rounds, or what gets pushed back. Charter F5 and the STATUS priority list are absorbed without saying so. · §1 never states what is *out*: screen reader, Arabic chrome, readable Arabic names. · "الشكل مش هيتقلب" is not accurate. Q2, Q3 and Q4 change the font, the "on" look of about 20 controls, the guide colours and the radii, and they remove motion Ahmed approved. · "مترتبة عشان ما تخبطش" is false (see P1-1).

Change — add three lines to §1: "التكلفة: حوالي ٣٠ يوم وكيل (مش ٢٤) + مراجعة Codex لكل حتة + ٧ مرات تجربة بإيدك؛ وده بيأجّل الماسكات ٣–٦ وباقي PAINS." · "اللي هتلاحظه: خط جديد في كل حتة، الزرار المتفعّل يبقى خط أزرق تحت بدل مربع أزرق، ألوان الـguides تتغير." · "اللي مش جوه: قارئ الشاشة وواجهة بالعربي — **وأسامي الطبقات بالعربي هتبان حروف مقطّعة لحد تجربة U0-E**."

Also replace "U0 يبدأ دلوقتي لأنه ملفات جديدة بس" → "نص U0 يبدأ دلوقتي (ملفات جديدة)، والنص التاني بعد S1-D وS4-A".

### P2 — should change

**P2-4 · The estimate counts pieces, not days.**
Evidence: 24 = 4+3+2+6+3+3+3 pieces. · U5-B is labelled L, which breaks the cap on its face. · Audits 04 and 05 size the key table L; U1-A calls it M. · U3-A = two sections + `num_field` (176 lines of scrub and modifier logic) + `swatch` (replaces 7 painters) + core `SetObjectBounds` + the artboard equivalent. · U3-C = the 515-line `panel_layers` + virtualising a tree with mixed row kinds + drag-and-drop + stable ids. · U3-E = the 418-line modal + the session + the core assert + `live_rev`. · Almost every piece merged this week needed a REQUEST CHANGES round (STATUS).

Change: pre-split these pieces: U5-B → B1 move-only `route()`; B2 Esc/Enter ladder + `reset_transient_input` + double-click helper. · U3-A → A1 core command + `num_field`; A2 sections + `swatch`. · U3-C → C1 rows, ids, `list_row`; C2 virtualisation + drag-and-drop. · U3-E → E1 core session, begin assert, `live_rev`; E2 modal UI. · U1-A → A1 table + adapters; A2 native menu built from the table.

Then replace "Total ≈ 24 agent-days" → "≈30 pieces ≈ 30–34 agent-days, including one fix round each. The hot-file queue sets the calendar, not the sum."

**P2-5 · No performance budgets.**
Evidence: Risk 7 says "measure … before and after U3-C" and gives no numbers. 01-A6 was never profiled. · The flow "Request → next frame's Snap" adds one frame of lag to every panel edit unless a repaint is requested. · There is no idle rule. Today the drag ghost repaints every frame (02-V-B2).

Change: new law **L37**, measured on the reference Mac with the standard layout and a 10 000-node document: `UiFrame` build p95 ≤ 3 ms. · 0 heap allocations per steady frame after warm-up. · Every dispatched `Request` repaints within the same event, so input becomes visible within 1 frame. · No repaint when nothing changed. · No block on the UI thread longer than 50 ms; I/O runs on S3's worker.

Test: a headless `UiFrame` bench plus a counting `GlobalAlloc` in the test binary (no new crate).

**P2-6 · No contract for states or undo.**
Evidence: §4 has a Disabled row but no Empty, Busy or Error rows. S2 §3.7, S3 and S6-B each invent their own copy and placement: status line, banner or rfd dialog. · Undo is covered only by L9, scrub and rename. Nothing says which controls never create a step (view, tool, selection, layout), or how document preferences behave (S1 F9 left snap and ruler prefs in `Document` without history).

Change — add §4 rows: **Empty**: every panel states its empty state in one line at ≥ 4.5:1, never blank. · **Busy**: work over 100 ms shows status-line text plus Cancel, and input is never blocked. · **Error**: decisions go in a native dialog. Outcomes go in the status line, saying what happened, what is unchanged and what to do (the S6 copy pattern). Nothing fails silently.

Also add an **Undo** column to §4: field commit = 1 step · scrub = 1 · nudge = 1 per press · document-pref toggle follows S1's checkpoint rule · view, tool, selection and layout = never.

**P2-7 · The contrast law is incomplete, and it contradicts S2 §3.7.**
Evidence: MUTED `#8f8a86` on HOVER `#2b2828` = **4.28:1**, which fails 4.5:1 (on PANEL it is 5.1:1, on SURFACE 4.7:1). L29 tests only PANEL and SEAM. · §4 says it adopts S2 §3.7 "exactly", but S2 §3.7 prints informational text in FAINT at 3.26:1 ("saved 14:32", the last-opened column). · There is no non-text rule. LINE on PANEL ≈ 1.2:1.

Change: L29 → "… ≥ 4.5:1 on **every surface token it can sit on** (PANEL, SURFACE, HOVER, ROW_HOVER, INPUT_WELL, ACCENT_TINT, SEAM). Icons and state glyphs ≥ 3:1. Deviations (hairline field edges) are listed by name." · In §4's Start paragraph add: "S2 §3.7's FAINT informational text becomes MUTED." · U3-F fixes the MUTED-on-HOVER pair.

**P2-8 · Focus is invisible on the current tool, and ⌃F6 clashes with macOS.**
Evidence: The current tool is an ACCENT fill, and focus is a 1 pt ACCENT outline *inset*. On that button the outline disappears (§4 "Button" row vs "Keyboard focus look"). · By default, macOS binds ⌃F6 to "Move focus to the floating window". This is the same class of clash as ⌥ vs Mission Control, which the spec already avoided. Verify on the Mac.

Change: Focus look → "1 pt FOCUS outline **outset** by 1 pt, visible on an ACCENT fill, ≥ 3:1 against both neighbours." · L13: "⌃F6" → "F6 / ⇧F6 (checked against macOS default shortcuts)".

**P2-9 · U6-C's "vendor-neutral standard" would not be credible as written.**
Evidence: Its content ("laws, token schema, kit contract, gates") is Varos and egui types: `Snap`, `EditCommand`, `egui::Vec2`. · "A test that its token table matches `tokens.rs`" makes Varos values normative in a document that is meant to be neutral. · Compared with Apple HIG, Material, Blender's principles and Figma's engineering posts on keyboard layouts and budgets, it has no chapters on accessibility, i18n/RTL, performance, states, undo, platform conventions, privacy/diagnostics, or conformance levels.

Change: U6-C → "**After V1**, extract `DESKTOP_UI_STANDARD.md`: normative MUST/SHOULD clauses, each with a test method. Chapters: ownership & commands · input & focus (physical keys, a list of system-shortcut clashes) · accessibility (keyboard-complete except drawing; focus distinct from selection; contrast on every surface; names and roles via AccessKit; targets ≥ 24 pt or a spacing exception; no meaning by colour alone, e.g. magenta vs cyan guides) · localisation & RTL · performance budgets · states & feedback · undo semantics · privacy & diagnostics · deviation register. Varos values go in an appendix, 'Varos profile'. No doc-vs-token test."

**P2-10 · Layout persistence ties a user file to the vendored fork's format.**
Evidence: §3.5 persists the fork's `Tree<PanelId>` serde and bumps `version` "whenever the fork changes it", so a third party controls a user-facing format. · UI_DIRECTION lists Workspaces as future work. · The bug parts (03-F-B3 duplicates, no Reset) do not need persistence.

Change: U4-A keeps uniqueness and Reset Workspace. Persistence moves to the Workspaces wave, stored in our own `LayoutFile { slots, shares }`. This saves about ½ day and one file format.

**P2-11 · Rect goldens will churn with every look tweak.**
Evidence: L26 and L32 snapshot rects at 0.5 pt for every component × 5 states and every panel. The owner tweaks looks often (78 "Ahmed MM-DD" comments, 05-G26), so each tweak would rewrite dozens of files and bury reviews.

Change: keep geometry goldens for **kit components only**. Panels assert the AccessKit tree from L36 (role, label, state, order). One investment then serves both a11y and tests.

**P2-12 · The spec quietly takes over charter F5.**
Evidence: FOUNDATION_CHARTER F5 says "Split `ui.rs` by ownership … per OWNERSHIP_MAP §5 … no region moves without a map row", with the `codex/p6-header` precondition. The spec re-plans the split (§3.1), never names F5, and puts p6-header "outside".

Change: add to the §5 header: "Supersedes charter F5 and the `ui.rs` half of OWNERSHIP_MAP §5. The p6-header precondition applies to U2. Update the map rows as each piece lands."

### P3 — nits
- **P3-13** · The doc-vs-token tests (§3.4, U6-C) parse Markdown and HTML only to catch drift, which is gold-plating. Instead, UI_DIRECTION cites token **names**, values live only in `tokens.rs`, and the mockup is stamped reference. Delete both tests.
- **P3-14** · L34's `grep "266"/"F0B429" = 0` guards a one-time deletion. It is not a law. Drop it; keep the SAFETY lint and the debug-only flags.
- **P3-15** · Merge L5 and L7 into L4, L16 into L15, and L30 into L29. Turn L31, L33 and L34 into one "E. Gates" list rather than laws. That gives 28 laws with the same tests.
- **P3-16** · L2's grep `ed\.` matches "used." and "speed.". Use `\bed\.`, or rely on the type check (panels only have `&Snap` in scope).
- **P3-17** · §3.2 says "S1 names unchanged", but U6-A renames `ToggleDock` → `ToggleBar`. Drop the rename or state the exception.
- **P3-18** · Q5 "عمود يمين ثابت 274" reads as "can't resize", which clashes with UI_DIRECTION "Boxes resize freely". Change to: "defaults to 274 pt, keeps its points when the window resizes, stays drag-resizable".
- **P3-19** · L34 removes `VAROS_PERF` and the dump flags from release builds with nothing in their place. Add a **Help ▸ Copy Diagnostics** item: version, commit, OS, GPU adapter, scale, loaded fonts, layout resets and crash-log path, all local, no network. Size S.
- **P3-20** · `INLINE_BTN` 22, `SW` 18 and `SW_S` 16 are below 24 pt (WCAG 2.5.8). Either record the spacing exception, or make the hit rect larger than the painted one.
- **P3-21** · U5-C (pinch) is a feature, not part of the system. Keep it (S), but it could be a quick win outside U5.

## Laws: load-bearing vs gold-plating
- **Load-bearing** (each traces to a P1/P2 defect): L1, L3, L4+L5, L6, L7, L9–L17, L19–L23, L24 (ids), L25–L28, L29 (once P2-7 is applied), L30, L31, L33, and the SAFETY half of L34.
- **Keep:** L18, L24 (virtualisation), and L32 reshaped per P2-11.
- **Gold-plating:** the doc-vs-token tests, the fake-value grep, per-panel rect goldens, and persisting the fork's tree format.
- **Missing:** L35 (strings and RTL), L36 (semantics), L37 (budgets).
- **Contradictions found:** L29 vs S2 §3.7; focus look vs the "on" fill; "S1 names unchanged" vs the U6-A rename; Q5 wording vs UI_DIRECTION.

## Economy — cut, merge, defer (net ≈ 0 to −1 agent-day, with far more coverage)
- **Defer:** layout persistence (−½), standard-doc extraction (−½ now), panel rect goldens (−½ to −1 of churn).
- **Drop:** doc-vs-token tests (−½).
- **Merge:** the vendor check into the existing CI job, instead of a new job.
- **Add:** U0-E Arabic spike (+½), U3-G migrate Start and Export to the kit (+1), `widget_info` inside U0-D/U3 (≈0).
- **Architecture:** no one-consumer abstraction worth cutting. `Panel` has 7 implementers and `Density` has 2 users. `homes.rs` is mostly metadata for tests; it is acceptable because it drives the "…"-to-home jump.

## The five decisions
Q1–Q5 are the right visual questions, but none of them touches the moat or the standard. Missing trade-offs: **Q1** omits the opportunity cost (masks 3–6, the rest of PAINS) and the F5 takeover. · **Q2** omits the binary-size cost (≈0.8–1 MB for 5 TTFs, an estimate to measure). It also omits the real reason to pick Plex: it has a matching Arabic family.

## Needs Ahmed (recommended default)
1. **Q6 Arabic in V1:** menus and labels stay English, but Arabic names you type (layers, artboards, files) must read correctly. Run spike U0-E first. *Default: yes.*
2. **Q7 Screen reader:** the kit labels everything now at no cost. Turning VoiceOver on (new crate `accesskit_winit`) is one ½-day piece after U3. *Default: yes, after U3.*
3. **Mirrored right-to-left UI:** not in V1. Code uses leading/trailing so it stays possible. *Default: later.*
4. **Layout memory:** fix duplicates and add Reset now; remember the layout later with Workspaces. *Default: later.*
5. **Public "standard" document:** after V1, with the chapters in P2-9. *Default: after V1.*
6. **Nudge undo:** each arrow press = one undo step (as in Illustrator), or grouped. *Default: one step per press.*
