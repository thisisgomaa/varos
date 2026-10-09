> **Status:** proposed — UI System **v3**, 2026-10-04, revised in place from v2 (2026-09-27) after the independent review's REQUEST CHANGES ("contracts without values"). Reconciled against `main` @ `221310f`. Every "today" value cites `file:line` at that commit (re-check before a piece starts). Not implementation evidence or independent approval; accepted ADRs and the charter remain authoritative. [`UI_DIRECTION`](../UI_DIRECTION.md) is the visual law; this spec makes it measurable.
# Varos UI System v3

## Current implementation routing — 2026-10-06

Main baseline: `07ff058`. The measured tables and old `file:line` references below remain the dated v3 design baseline, not current source locations. Since then P2 (`fa6bd10`), P3 (`d882454`), top bar 4b (`19611ae`) and polish (`7adee37`) landed. Inter + JetBrains Mono replaced Plex in UI; box glide stays by owner decision 2026-10-06. Close-out 1 (`736feba`) removed band Search and Custom… and brought recovery scanning before frame 0 (bounded 250 ms wait; pending scans still poll). Trackpad control (`2f2bcc0`) and recovery card / in-place Review (`1532637`) are landed, awaiting the owner's eye. The strip entry and split map below describe the earlier baseline; recovery now lives in `recovery_card.rs`.

Cycle-close cleanup ratchet ceilings and measured counts: FontId 16, `.size` 18, raw colours 31, literal radii 20, `ui.rs` 931 lines (includes `recovery_card.rs` in production scan). QW6 panel icon size, remaining P4, P5/P7, P6/F7/F8 measurements and Arabic gate §8 remain open. VIEW_ROTATION is planned/parked. v3's independent document review remains pending; these landings do not approve every proposed contract.

## ملخص للمالك

- ده «دستور الواجهة» بالأرقام: كل لون ومقاس وزرار له قيمة واحدة، ونقدر نختبرها بالإيد وبالاختبارات.
- القرارات اللي إنت خدتها بقت قانون ومش هتتسأل تاني (الجدول تحت): Home بدل ☰ على الماك، أيقونات بدل الكلام (الكلام في التلميح)، خطوط Inter/JetBrains Mono (Plex للعربي)، مفيش حركة أو توهج إلا glide البوكسات، وشكل «الشغّال» لكل نوع زرار.
- أهم قاعدة جديدة: **أي خانة بتكتب فيها تتحفظ لما تسيبها** (Enter أو Tab أو تضغط برّه أو تبدّل تبويب أو تحفظ)، و**Esc يرجّع القديم**. ده بيقفل مشكلة اسم الأرتبورد اللي بيضيع لما تضغط برّه.
- ملف الواجهة الكبير (`ui.rs`، ٨٢١٥ سطر) هيتقسم لملفات صغيرة، نقل بس من غير تغيير سلوك، وفيه عدّاد يمنع إنه يكبر تاني.
- قسنا المخالفات النهارده: ٧٥ مقاس خط مكتوب بإيد، ٤٥ لون مكتوب بإيد، ٢٦ تدوير زوايا بإيد، و٣٩ أيقونة متعرّفة جوه `ui.rs` بعيد عن سجل الأيقونات. العدّاد ده هينزل بس، عمره ما يطلع.
- العربي: الخط موجود بس الكتابة العربية في الخانات لسه مكسورة في egui؛ ده قطعة مستقلة ليك تختار فيها طريق الإصلاح.
- أول ٣ قطع: (١) الورقة دي، (٢) قانون حفظ الخانات + إصلاح خانة الاسم، (٣) أول تقسيم لـ`ui.rs` + سجل أيقونات واحد + FAINT→MUTED + مقاسات QW6 + العدّادات.

## 0. Owner law (decided — not re-opened)

Sources: [decision record 24/25 Sep](../history/STATUS_THROUGH_2026-09-26.md), [UI_DIRECTION](../UI_DIRECTION.md), CLAUDE.md, moderator ruling 2026-10-04.

| # | Law | Measurable form |
|---|---|---|
| L1 | Home replaces the ☰ burger on Mac; native menus stay | Mac app bar cell = kit Home chip (`ui.rs:3510-3520`); ☰ cell only on the compile-only Windows path (`ui.rs:3521+`). |
| L2 | Icons, not text labels, on chrome buttons | Every icon-only control has a tooltip naming it (+ shortcut); visible text only in menus, lists, fields, dialogs. |
| L3 | Inter 400/500/600 + JetBrains Mono 400 bundled; Arabic gated | `shell/fonts.rs`; Plex Arabic is a named diagnostic family only until §8 passes. Mono = numbers, sizes, paths and dates. |
| L4 | Chrome appears instantly; live tab drag has no interpolation; direction bar stays. The box glide on dock/undock is an accepted exception (owner 2026-07-05, reaffirmed 2026-10-06: not a defect) | `animation_time = 0` (`tokens.rs:90`); no time-based interpolation in chrome code (tab drag: `chrome.rs:220` "no time, no interpolation"). |
| L5 | "On" looks | tool = azure block + white icon; icon toggle and icon segment: rest = no fill + MUTED; hover = HOVER + TEXT; on = TOGGLE_WELL (black, below PANEL) + TEXT, no bar. Azure remains focus/selection. Owner 2026-10-08: «زرار منوّر وخلاص، بلاش ألوان كتير». Tabs retain their existing fill. |
| L6 | FAINT → MUTED with QW6 sizes | Informational text never FAINT; panel icon glyph 18 pt in the 26×24 chip; micro-labels 10.5 pt. |
| L7 | Remember the per-user shell layout (owner 2026-10-08); Reset layout lives in the Window menu | Versioned `layout.json`, atomic debounced writes and quit flush; reset touches shell state only. |
| L8 | Tab drag: the tab follows the pointer, others reflow instantly, no drop line | `chrome.rs:208-221` geometry; Esc / focus loss / list change cancels (`ui.rs:3318-3345`). |
| L9 | Field edits commit on blur (Illustrator/Figma convention) | K3 table. |
| L10 | Shortcuts equal Illustrator's | Any new chord is checked against Illustrator before binding. |

Layout persistence law (owner 2026-10-08): per-user `<data root>/layout.json`, envelope `{version:1, app_build, layout}`.
Persist the complete docked tree: split shares, panel membership/visibility and active tabs, plus rail/control-bar visibility.
The side column span and box rectangles are derived from the saved tree and current viewport; drag ghosts are transient (no persistent floating/collapsed boxes exist today).
Write through the existing atomic replacement helper after one second without changes, compare canonical hashes, and flush pending changes on confirmed quit; stop after three failures until the next real change, logging once.
Restore before frame zero; repair duplicate panels (keep the first in tree order) and stale active tabs; reject non-dockable panels, grids, incompatible versions and unsafe trees, silently use the standard layout and quarantine bytes as `layout.json.bad`. Read errors leave the file untouched. The ☰ switch swaps panels when its target is already open.
Deliberate L7 decision (2026-10-08): version 1 stores egui_tiles' serde format with quarantine; an egui_tiles upgrade may silently reset layouts (accepted for now).
Window menu “Reset layout” and `VAROS_RESET_LAYOUT=1` restore the standard shell and remove the file; documents, Start page and `window.txt` are independent.

## K1 — Library and command boundary

1. `shell/kit` (lib) knows no binary type: it takes widget key, label/icon, state, availability (+reason) and returns a `ControlResponse`/value (`shell/kit/mod.rs:13-53`). No `AppCommand`, no I/O, no `Editor` mutation in kit.
2. The binary translates kit results to `AppCommand` once (E2 adapter for Start; `app_command.rs`). Open/Recent/Locate/OS-open use one open path (`AppCommand::OpenPaths`).
3. Later U1-A adds `CommandId/Request/CmdCtx` (Edit/App only; owned context, resolved outside the egui frame and from the native menu). Until then the current `AppCommand` + `host::DocAction` remain the contract; transient setters follow F4.

## K2 — Responder and keyboard law

**Precedence** (first owner wins; the key goes nowhere else):

| # | Context (who owns it) | Keys it takes | Today on main | Shortcuts yield? |
|---|---|---|---|---|
| 1 | Command keys — `host::Keyboard::key` | ⌘N ⌘O ⌘S ⇧⌘S ⌘W ⌘Q, Ctrl+Tab: press → `AppCommand` queued; repeats + release swallowed | `host.rs:117-125,179-197`; `main.rs:683-699`; runs BEFORE the field check (`main.rs:1438-1444`, guard at `:1442`) | Never — ⌘S inside a field saves (after the K3 commit). |
| 2 | Home / Start — `StartModel` | Tab/⇧Tab, ↑↓ in lists, Enter/Space, Delete + Backspace on a Recent row | `start.rs:93+`; native-menu keys dropped on Home (`main.rs:1050-1051`) | All document shortcuts yield. |
| 3 | Open kit menu — `kit::menu` | ↑↓ move, Enter/Space choose, Esc close; also consumes Tab | `shell/kit/mod.rs:291-335` | Everything behind it yields. |
| 4 | Focused text/number field — `gui.wants_keyboard()` | All non-command keys (text undo/copy/paste/select-all go to the field) | `main.rs:1449`; native ⌘-rows forwarded to egui (`main.rs:1052-1056`); Plain rows (Edit ▸ Delete) never while typing (`main.rs:1066-1067`) | Canvas shortcuts yield. |
| 5 | Colour picker v3 (modeless) | Esc over the panel closes; armed Esc reverts the sample and disarms (second Esc closes); Enter commits the focused K3 field | `ui/picker/mod.rs`, `main.rs` | Canvas/tools remain live; armed canvas Esc only cancels sampling; otherwise canvas Esc deselects; focused fields, popovers and tab drags consume Esc first. |
| 6 | Live tab drag | Esc = cancel drag (nothing committed) | `main.rs:1454`; `ui.rs:3318-3345` | Esc does not also deselect. |
| 7 | Canvas — `DocAction::Key` | Space = pan/reposition (`main.rs:1456-1462`); other presses → `raise_doc` (`main.rs:1463-1466`) | S1 router in `main.rs` until U5 | — |
| 8 | Native menu (Mac) — `host::menu_route` | `App` → queue; `Key` → rows 2/4/7 above; `Plain` → canvas only; `Snap` → view | `host.rs:357`; `main.rs:1040-1075` | Same precedence as the keyboard path. |

**Rules**

1. `host::Keyboard` (`host.rs:108-115`) is the ONE record of held modifiers, Space and command keys down. It is mirrored into the active tab's `Editor::mods/space` and cleared on `WindowEvent::Focused(false)` (`main.rs:1191-1196` → `Keyboard::focus_lost`, `host.rs:143`); a release never leaks into the next tab.
2. macOS egui focus seed: `egui_focus_seed(window_key, egui_focused) = egui_focused || window_key` (`ui.rs:3315-3317`) only RAISES egui's focus; losing focus stays winit's `Focused(false)` path. Test `window_focus_tests` (`ui.rs:8106`).
3. `host::ActionQueue` (`host.rs:236-290`) is the ONLY dispatcher: FIFO; every pointer press/release leaves a mark; a Ui-frame command is inserted at its release mark (`chrome_frame`); drained at `AboutToWait`. A `DocAction` may run inline only when the queue is empty and no pointer mark is pending. No second queue, no direct dispatch from paint code.
4. Every source re-checks enablement at drain; a menu may show stale state, the drain decides. Request a repaint after any command that changes what is shown.
5. Keys are physical `KeyCode`; primary (⌘/Ctrl), physical Control, Alt and Shift stay distinct; the platform lives in the adapter. Mac hint order ⌃⌥⇧⌘. No native accelerator for plain keys.
6. Canvas Tab is reserved for Illustrator's Hide/Show Panels; until built it does nothing and must not move egui focus onto a button (audit 04). Fix owner: piece P5.
7. Test fixtures build a simple owned `InputEvent`; never a private Winit `KeyEvent`, never an `EventLoop`.

## K3 — Field edit transaction law

Applies to every text and number field: `num_field` (`ui.rs:1762-1939`), `name_field` (`ui.rs:5342-5376`), Layers rename (`ui.rs` panel_layers), picker hex, Layers search (search commits nothing to the document).

| Event while a field has focus | Valid text | Invalid text |
|---|---|---|
| Enter | Commit → ONE undo step; field blurs | Keep focus; inline reason under the field |
| Tab / ⇧Tab | Commit; focus to next/previous field of the same panel (wraps; never a button) | Keep focus; reason |
| Click elsewhere (blur) | Commit | Keep focus; reason; the click is not delivered |
| Tab switch / Save / Close / Quit | Commit first, then the command (Close computes dirty after the commit) | Command does not run; focus + reason stay |
| Selection change removes the field | Commit to the object it was editing, then switch | Revert silently (the field is gone; no trap) |
| Esc | Revert to the value at focus-in; no history; blur | Same |
| Arrows / scrub inside the field | Live preview; the whole focus session = ONE undo step | — |

1. Unchanged text commits nothing: no undo step, no dirty (closes audit 01 B6: unchanged artboard rename marks dirty).
2. Invalid = number text that does not parse; a name that is empty after trim. Out-of-range numbers clamp (today's behaviour, `ui.rs:1878`), they are not invalid.
3. Reason text uses token `ERROR` (to add, §K5) at `T_SMALL`; it is cleared by the next valid keystroke or Esc.
4. The buffer is keyed by `SessionId` + field (`doc_id`, `ui.rs:1738`); the host can read it to commit before lifecycle commands. `settle_field_edits` (`ui.rs:1746-1755`) changes from **discard** to **commit-valid**.
5. Core owns the transaction (U3-T: `begin/update/commit/cancel(owner)`, foreign `begin` refused, no nested pending). P2 may ship rules 1-4 on today's `EditCommand`s; the one-undo arrow/scrub session needs U3-T.

**Live bugs this closes** (read from code; each gets a red test first): (a) `name_field` computes `editing = has_focus(id)` at frame start (`ui.rs:5344`); on the click-away frame focus is already gone, so `buf` is rebuilt from `value` and `lost_focus()` returns the ORIGINAL text — typed text dropped (PAINS_LOG "name-field click-away"). (b) Every blur commits even unchanged text (B6). (c) Esc in `num_field`: egui surrenders focus, which reaches the `lost_focus()` commit branch (`ui.rs:1870`) — Esc commits instead of reverting.

**Owner hand test (P2):** select an artboard → type a new name → click the empty canvas: the name stays. ⌘Z: the old name returns in one step. Type again, press Esc: old name, no `*`. Type in W, press ⌘S: the file saves with the new width. Clear the name and press Enter: the field keeps focus and says why.

## K4 — State, reads and performance budgets

1. `DocUi` state is chosen in `set_tabs(active_id)` and dropped on tab close; widget ids are salted by `SessionId`; same-id document replacement needs a new generation (U2-D).
2. Invalidation keys: session+generation, committed revision, selection, live geometry. Layers rows already reuse on pointer-only frames (`ui.rs:1334-1340`, `LayerRowsCache` `ui.rs:308`).
3. No measurement + optimisation + state move in one diff.

**Measurement protocol (U2-P)**

| Item | Value |
|---|---|
| Machine | The owner's Apple-Silicon Mac; record model, chip, RAM, macOS, display scale, refresh rate |
| Build | `--release`, `VAROS_PERF=1` (prints `scene_path` / `full_frame`, `main.rs:1569-1576`); U2-P adds `ui_build` (egui `run` + tessellation) and per-panel rebuild counters |
| Scene | Generated fixture of 10,000 paths on 1 artboard (generator + hash committed with U2-P), plus a 10-path small file |
| Runs | Idle 10 s · ⌘A once · drag selection 5 s · tab switch ×20 · Layers open/closed; 3 runs each, ≥ 300 frames |
| Report | p50 / p95 / max per metric, frame count, rebuild counts, device line — appended to GATE_LOG |

**Targets — provisional until U2-P measures**

| Metric | Target |
|---|---|
| `ui_build` p95, 10k scene, every run | ≤ 8 ms |
| Panel rebuilds while idle | 0 per frame |
| Heap allocations per idle frame in panels | 0 (counting allocator, perf build only) |
| Regression vs U2-P baseline | none beyond measured run-to-run spread |

Every later UI piece reports these numbers (or "not affected: why") in its GATE_LOG block.

## K5 — Tokens, components, icons, accessibility

`shell/tokens.rs` is the only runtime source; no doc↔Rust sync parsers. Contrast = WCAG 2.x ratio, computed on the real (composited) background; text ≥ 4.5:1, icons/focus/UI parts ≥ 3:1.

**Colours**

| Token | Value | Used for | Contrast on its background | Status |
|---|---|---|---|---|
| `SEAM` | #000000 (4b, 2026-10-05; was #0e0d0d) | the one backdrop: band, seams, status line, Start's void, NSWindow background | TEXT 16.9 · MUTED 6.15 · DISABLED 3.92 | `tokens.rs` |
| `BG` | #141313 | board base | TEXT 14.9 · MUTED 5.43 | `tokens.rs:27` |
| `INPUT_WELL` | #171515 | focused number field | TEXT 14.6 | `tokens.rs:37` |
| `RULER_BG` | #181616 | rulers | MUTED 5.28 | `tokens.rs:50` |
| `PANEL` | #1b1919 | boxes, active doc tab, disabled kit fill | TEXT 14.1 · MUTED 5.12 · FAINT **3.26 ✗** | `tokens.rs:28`; alias `SOLID_PANEL` (`ui.rs:24`) → remove |
| `SURFACE` | #242121 | fields, controls, active panel pill, popups | TEXT 12.9 · MUTED 4.68 · FAINT **2.98 ✗** | `tokens.rs:29`; aliases `BG_SURFACE`, `SWATCH_WELL` (`ui.rs:25`) → remove |
| `ROW_HOVER` | #262323 | list-row hover | MUTED 4.56 (thin) | `tokens.rs:36` |
| `HOVER` | #2b2828 | hover/pressed fill | TEXT 11.8 · MUTED **4.28 ✗** → text on HOVER is TEXT | `tokens.rs:30` |
| `ACCENT_TINT` | azure α34 → #192835 on PANEL | selected row | TEXT 12.1 · MUTED **4.40 ✗** → TEXT | `tokens.rs:43` |
| `LINE` | #2c2929 | 1 px hairline (separation only) | 1.21 on PANEL — decorative, never the only cue | `tokens.rs:31`; alias `BORDER` → remove |
| `LINE2` | #3b3735 | stronger hairline, kit control border | 1.49 on PANEL | `tokens.rs:32`; alias `BORDER_2` → remove |
| `TEXT` | #e9e6e3 | primary text, icons on hover/on | ≥ 11.8 on every chrome fill | `tokens.rs:33` |
| `MUTED` | #8f8a86 | secondary text, icons at rest | 4.56-5.68 on SEAM…ROW_HOVER | `tokens.rs:34` |
| `FAINT` | #6e6a66 | disabled text / placeholder ONLY | 2.73-3.62 (text-exempt only when disabled) | `tokens.rs:35`; → rename `DISABLED` once informational uses = 0 |
| `ACCENT` | #0c8ce9 | selection, active, focus | 4.95 on PANEL · 4.14 on HOVER (UI ✓) · white icon on it 3.53 (UI ✓, **text ✗**) | `tokens.rs:38` |
| `AGENT` | #c76a20 | agent canvas activity only (owner 2026-10-08); azure stays human selection/focus | 3.81 on white · 4.58 on PANEL | `tokens.rs` |
| `ACCENT_HOVER` | #2b9df4 | hovered primary button | white text **2.90 ✗** | `tokens.rs:39` |
| `ACCENT_SEL` | azure α60 | text selection | — | `tokens.rs:42` |
| `GUIDE` | #ff54a8 | smart guides (reserved) | 5.91 on PANEL | `tokens.rs:44` |
| `CLOSE_RED` | #c42b1c | window-close hover | white 5.66 | `tokens.rs:51` |
| `NONE_RED` | #e05c5c | "none" paint slash | 4.88 on PANEL | `tokens.rs:52` |
| `ERROR` | #e05c5c (same value) | field reason, failure notice | 4.88 on PANEL · 4.45 on SURFACE → reason sits on PANEL | **to add** (P2) |
| `AMBER`, `NAVY`, `DOT_GRID`, `VOID_HOVER` | see file | samples / grid / void hover (white α.04 on SEAM = #181717) | — | `tokens.rs:48-54` |
| Picker/thumbnail greys | `from_gray(24…242)`, black/white α | checkerboard, knobs over artwork, thumbs | over user colour: exempt, must be tokens | literals `ui.rs:2093-2351, 4272-4815` → `CHECKER_*`, `KNOB_*`, `THUMB_*` |

**Sizes, radii, spacing, type**

| Token | Value | Used for | Status |
|---|---|---|---|
| `R` / `RBOX` / `RCAP` | 3 / 8 / 11 | controls & menu rows / boxes, popup frames (kit menu `kit/mod.rs:352`; legacy `MENU_R` 4 `ui.rs:3235` → owner Q1), doc tab chips / capsules (panel tab pills, scroll chevrons, switch track) | `tokens.rs:57-59`; `R`'s comment says "tabs" → fix; 26 literal radii in `ui.rs` (2×12, 3×7, 4×5, 5×2) → map to R |
| `SEAM_GAP` | 12 | equal void between boxes | `tokens.rs:60` (owner 07-04) |
| `KIT_MIN_TARGET` | 24 | minimum hit target | `tokens.rs:148` |
| `KIT_CONTROL_H` / `KIT_ROW_H` | 32 / 56 | kit button height / two-line row | `tokens.rs:149-150` |
| `ICON` (new) | 18 | panel icon glyph in 26×24 chip (QW6) | to add; today 14-17 literals (`ui.rs:1989,2006,4379,3116`) |
| `ICON_SMALL` (= `KIT_ICON`) | 16 | kit, rail, menus | `tokens.rs:151` → rename |
| `ICON_RASTER` | 32 | raster size of every Lucide icon | `tokens.rs:159` |
| `KIT_PAD` / `KIT_GAP` / `KIT_TEXT_GAP` | 8 / 8 / 4 | the 4/8 beat | `tokens.rs:152-154`; literals `MENU_ROW_H 26`, `MENU_GUTTER 28`, `RULER 18`, row 26 (`ui.rs:3232-3235,5741,4453`) → tokens |
| `KIT_STROKE` / `KIT_FOCUS_STROKE` | 1 / 2 | hairline / focus ring | `tokens.rs:155-156` |
| Scrollbar | 8 bar · 6 floating · 24 min handle | overlay bars | `tokens.rs:94-97` |
| Type roles | Heading 13.5 · Body 13 · Button 12.5 · Small 11 · Mono 12.5 | egui text styles | `tokens.rs:77-81` |
| `T_MICRO` (new) | 10.5 | section micro-labels (QW6; today `.size(10.0)` ×10) | to add |
| Start sizes | 26 / 20 / 14 | Start title / section / file | `tokens.rs:139-141` |

**Offenders today and the ratchet** (baseline counted by grep at `221310f`; P3's
`shell/ratchet_tests.rs` now discovers production `ui.rs` + `recovery_card.rs` + `ui/**/*.rs` with `read_dir` at test
time, excludes files named `tests.rs`, and asserts `count <= CEILING`; a piece that removes
offenders lowers the ceiling in the same commit; inline `#[cfg(test)]` helpers remain counted;
raising a ceiling is a review failure):

| Pattern | `ui.rs` | other files |
|---|---|---|
| `FontId::proportional/monospace(<number>)` | 41 (9 distinct sizes 9.5-14) | boxtree 8 · registry 7 |
| `.size(<number>)` on `RichText` | 34 | — |
| raw `Color32` (`from_rgb*`, `from_gray`, `from_*_alpha`, `WHITE`, `BLACK`) | 45 (TRANSPARENT not counted) | boxtree 3 |
| `CornerRadius::same(<number>)` | 26 | boxtree 5 · registry 1 |
| inline Lucide `const IC_*` | 39 (`ui.rs:26-83`) | kit registry holds 6 |
| `FAINT` uses | 26 | — |
| token aliases (`LINE as BORDER` …) | 5 (`ui.rs:22-25`) | — |
| `ui.rs` total lines | 8215 | — |

**Component inventory** (one row per component; a11y gate = target ≥ 24 pt · text 4.5:1 · UI 3:1 · focus visible · tooltip on icon-only)

| Component | Rest | Hover / pressed | On | Disabled (+reason tooltip) | Focus-visible | Size · hit | A11y gate today | Lives today · kit? |
|---|---|---|---|---|---|---|---|---|
| Button (text) | SURFACE + LINE2 border, TEXT | HOVER / HOVER | SURFACE, no border | PANEL, MUTED, reason ✓ | 2 px ACCENT inset, keyboard only | h 32 · ≥ 24 | ✓ | `kit::action` (`kit/mod.rs:53,83-216`) · kit |
| Icon button | as button, icon MUTED→TEXT | HOVER | — | as button | as button | 32×32 kit; 26×24 legacy | kit ✓; legacy no focus ring ✗ | kit `action(icon_only)`; legacy `icon_btn` `ui.rs:1998` · ad hoc |
| Tool button (rail) | none, MUTED icon | HOVER | ACCENT block, white icon (3.53 UI ✓) | — | ✗ none | 30×30 | ✗ focus, ✗ tooltip check | `icon_button` `ui.rs:3006` · ad hoc |
| Icon toggle | none, MUTED | HOVER | **TOGGLE_WELL + TEXT (owner 2026-10-08)** | — | ✗ | 24×24 | ✗ focus | `icon_toggle` `ui.rs:1979` · ad hoc |
| Switch | track SURFACE | row HOVER | track ACCENT | — | ✗ | 32×18 track in 26 row | ✗ focus | `toggle_row` `ui.rs:5378` · ad hoc |
| Segment | SURFACE, MUTED 11 | HOVER | **today ACCENT + white text (3.53 ✗) → law: SURFACE fill, TEXT** | — | ✗ | h 22 ✗ (< 24) | ✗ | `seg_btn` `ui.rs:5170` · ad hoc |
| Doc tab chip (+ live drag) | bare MUTED 12 | VOID_HOVER | PANEL block, TEXT, RBOX | — | ✗ | h 28 | ✗ focus | `tab_item` `ui.rs:3180`; drag `ui.rs:3318-3421`, `chrome.rs:208+` · ad hoc |
| Panel tab pill | bare MUTED | HOVER | SURFACE pill, RCAP | — | ✗ | h 22 ✗ | ✗ | `shell/boxtree.rs:584` · ad hoc |
| Home chip | as icon button | HOVER | SURFACE (on Home) | — | ✓ | 36×40 cell | ✓ | kit (`ui.rs:3510-3520`) |
| List row | PANEL, TEXT + MUTED detail | HOVER, detail→TEXT | SURFACE | PANEL, MUTED | ✓ | h 56 / 72 | ✓ | `kit::list_row`, `document_row` · kit |
| Layers row | none | ROW_HOVER | ACCENT_TINT + 2 px ACCENT bar | — | ✗ | h 26 | MUTED on tint 4.40 ✗ | `panel_layers` `ui.rs:4394-4910` · ad hoc |
| Menu row | none, TEXT | HOVER full-bleed | ✓ mark | MUTED/FAINT + reason | kit ✓ / legacy ✗ | h 32 kit · 26 legacy | legacy ✗ | `kit::menu_row` `kit/mod.rs:392`; legacy `menu_row` `ui.rs:3238-3313` · both |
| Numeric field | SURFACE, value 13 centred, label FAINT 11.5 ✗ | — (✗ none) | editing: INPUT_WELL + 1 px ACCENT | ✗ none | ACCENT border | h 25 | label ✗ (FAINT) | `num_field` `ui.rs:1762` · ad hoc |
| Text field | SURFACE + LINE | ✗ none | editing: ACCENT border | ✗ none | ACCENT border | h 26 | ✓ text | `name_field` `ui.rs:5342`; search/rename/hex vary · ad hoc |
| Colour picker v3 (modeless) | PANEL + LINE, cached angular ring and rotating HSV triangle | live per-gesture preview | TOGGLE_WELL tabs; ACCENT focused target only | Gradient tab: engine pending; Harmony implemented | K3 field focus ring | 240 pt Board hand; 32 pt header, 28 pt swatch row | Owner native hand test pending; compact glyph hits retain Figma sizing | `ui/picker/{mod,panel,wheel,sliders,modes,harmony,harmony_rules,mini,cluster,fields,drawer}.rs`; kit fields/icons, L7 layout |
| Colour swatch | colour + LINE2 | white border | ACCENT ring (active target) | — | ✗ | 15-17 ✗ (< 24 hit) | ✗ target | `swatch_strip` `ui.rs:2122`, `ctl_chip` `ui.rs:4095` · ad hoc |
| Section heading / panel header | MUTED micro 10 `.strong()` | — | — | — | — | — | MUTED ✓; size → 10.5 | `kit::section_heading`; inline `ui.rs:4950,5051,5061` · both |
| Scrollbar | invisible until body hover | 6→8 px handle | — | — | — | 24 min handle | egui | `tokens.rs:94-97` · egui |
| Notice / strip | MUTED text; strip on SEAM | — | — | — | — | — | ✓ | `kit::notice` `kit/mod.rs:232`; recovery strip `ui.rs:3740` |
| Agent presence | none without an edit session | — | 1.5-pt page outline + title-style label; staggered fading 1-pt object bounds, AGENT | — | human azure wins | canvas-only, no hit target | full-strength UI contrast ≥ 3:1; headless clock/pacing tests | `agent_presence.rs`, `ui/canvas_overlay.rs` · host overlay |
| Native dialogs (Open/Save/Save changes?/errors) | **native by law** (rfd + OS sheets) | OS | OS | OS | OS | OS | OS | `file_ports.rs`, `main.rs` · native, never re-drawn in egui |

Colour picker v3 slices 1–4: Sliders persists its mode in the additive `picker.mode`
layout preference (serde default HSB). HSB/HSL/RGB conversions are exact; CMYK is naive
subtractive sRGB and Lab uses sRGB linearisation and XYZ D65, with no colour management,
ICC profiles or gamut mapping. Gradients vary one channel at the current other channel
values and cache meshes per mode/colour. Web uses R/G/B tracks stepped to 00/33/66/99/CC/FF;
editing a channel snaps that channel to its nearest web-safe value. Switching modes is
read-only, so a non-web-safe colour remains intact until edited. Wheel readout follows the
chosen mode (Web shows hex). Numeric fields reuse K3 including ↑/↓ ±1 and Shift ±10;
steps that snap to an unchanged Web value produce no undo step.

Slice 4 (2026-10-09): Harmony's eight original SVG toggles persist `picker.harmony`
with a serde default of Complementary. `harmony_rules.rs` ports the hue offsets and
Mono brightness clamps from `da05aca` verbatim. Original linked colours come first;
six 20 pt result chips are completed with tones using that same brightness progression.
The old eighth glyph/rule was None, not Shades: Shades uses `harmony-none.svg` and
aliases the recovered Mono brightness rule. This is an explicit contract/asset mismatch,
not a newly invented colour algorithm. Clicking a result chip is a change-only atomic step.

Drawer Recent is MRU; Board scans paths belonging to the active artboard using the
existing outline-overlap membership; Document scans every path. Fill then stroke,
first appearance, epsilon dedupe, cap 36 (Recent keeps its existing cap 12).
These are derived artwork colours, not a saved library. Empty chips use MUTED stripes.
The drawer tab and expanded state remain layout preferences.

The Artboard panel's page-colour chip opens Mini at its anchor, targeting a stable
artboard ID. Mini is the same state machine with a 192 pt configuration: shared Wheel,
hex/alpha/pipette row and seven recent chips plus drawer; no header tabs, readout,
default buttons or target cluster. Opening either configuration finishes the previous
configuration's gesture, so only one owns a transaction. Mini closes on Esc or a press
outside its rect/anchor (K3 fields consume Esc first; an armed canvas click accepts a
sample). Mini is transient; only the full panel's open state persists. A deleted page
closes Mini. Dark tokens replace Figma's white chrome; Mini omits the numeric alpha
box to match the compact reference. Native visual/interaction validation remains pending.

Only an accepting canvas click commits an eyedropper preview. Disarm, close, Esc,
field/target focus, panel press, selection change, document keys and Bridge mutations
cancel unaccepted samples via `PickerCancel`, with no history/default/recent change.
Completed drags remain committed. Page-colour targets resolve stable artboard IDs;
a missing ID cancels and falls back to the current Paint target in the full panel (Mini closes). Idle and armed idle
request no repaint timer. Existing older builds reject this branch's additive layout
fields and quarantine `layout.json` as `.bad`; compatibility behaviour is retained.

**State rules**: precedence disabled > on > pressed > hover > rest; focus-visible is an overlay drawn on top of any state (also on ACCENT: ring outside the block). Text on HOVER or ACCENT_TINT is TEXT. Every disabled control says why in its tooltip. A row marked ✗ is closed when it moves into kit (U3-K), not by local patching.

**Icon rules**

1. One registry: `shell/kit/icons.rs` (`Icon` enum, `kit/icons.rs:8-29`). P3 moves the 39 `IC_*` strings and `lucide()/load_icon` (`ui.rs:26-83, 871-895, 1590-1605`) into it; nothing else defines SVG text.
2. Lucide first (ISC licence beside `assets/icons/`). When Lucide lacks a glyph, draw an original on the 24-grid, 2 px stroke, round caps — from scratch, never traced from another product. Cursors keep their own rule (no hand-drawn cursors).
3. Raster once at `ICON_RASTER`, white, tinted at paint; draw size only from `ICON` / `ICON_SMALL`.
4. Every icon-only control has a tooltip (name + shortcut via `shortcut_label`).

**Fonts**: Start v2 L1 supersedes U0-A's Latin chain: static Inter Regular 400 / Medium 500 /
SemiBold 600 (`ui-400`, `ui-500`, `ui-600`) and JetBrains Mono Regular 400 (`mono-400`).
Proportional defaults to Inter Regular; Monospace defaults to JetBrains Mono; both retain the
Noto symbol fallbacks for ⌘⌥⇧⌃ and arrows. Semantic constructors in `shell/tokens.rs` are the
one home for h1 30/600, h2 18/600, button 15/500, name 14/600, body 13/400, small 12/400,
tag 11/500 and mono 11/400. IBM Plex Sans Arabic remains named-diagnostic-only behind §8.

## Agent presence — owner decision 2026-10-08

Azure = human selection/focus. `AGENT` = anything an agent is doing, including
artboard and object feedback. The single new colour role is `AGENT #C76A20`
(burnt orange; full-strength outline contrast 3.81:1 on white, 4.58:1 on PANEL).
Values, stroke widths and label dimensions live only in `shell/tokens.rs`.
This replaces the earlier band/status chip proposals; no panel or band changes.

The host maintains logical Bridge sessions by existing client id, with sanitized
label (24 Unicode scalars, no controls), profile id, document, last artboard,
internal dispatch state and reveal queue. Existing FIFO edit dispatch prepares presence;
a successful new commit's receipt completes it. Failed/no-op/replayed batches
restore previous feedback. Target page references win, otherwise the active page.
Host reset and document close clear state; after four seconds without edits the
session feedback expires. One-call sockets provide no logical disconnect event,
so socket close cannot honestly mean the agent session ended.

The page has a flat 1.5-pt AGENT outline flush outside its bounds
after each committed edit (4-s hold). Accept/handle/complete run synchronously
on the UI thread, so in-flight dispatch is never visible. Non-interactive labels sit at its top-right
above the page, leaving the existing settings dots clear; same-page
labels stack upward by profile id. The label uses the existing 11-pt Inter title style,
PANEL fill, TEXT ink, R radius and AGENT hairline. No shadows or shell layout work.

After the atomic commit (still one undo step), created/changed objects reveal
1-pt AGENT world-bounds rectangles in operation order; deleted objects never
paint. The artwork is complete immediately; only feedback appears in staggered
steps of min(25 ms, 1500 ms / N). Each outline fades for 900 ms. New work collapses
remaining old stagger and finishes old highlights within 120 ms, then begins the
new sequence. Human object/direct/group selection suppresses agent rectangles.

Short canvas feedback fades are an owner-approved motion exception. One clipped
overlay pass draws bounds, without path tessellation. Repaint deadlines use the
existing egui/display-refresh pacing; static glow schedules only expiry, and no
session/effect schedules no repaint. Capped receipts use local document data,
never extra calls, to recover touched ids. No Bridge wire changes.

## K6 — Panels, size and motion

1. One registry + `match` for panels (`shell/registry.rs`), no `Panel` trait. Every domain has one Section home; bar/menu items are mirrors that open, reveal and expand that home.
2. Board exactly once, never closable or tabbable. Window-menu ticks mean visible/frontmost.
3. U4-S size model: min/preferred/max in logical points, one Fill slot; tree normalised on mutation, not per frame; board rect excludes rulers.
4. Motion: instant chrome (L4); the box glide stays (owner decision 2026-10-06 — U4-M dropped, no ADR needed); `scroll_animation` none; wheel smoothing residue is measured, not denied.
5. Named workspaces stay deferred; per-user shell persistence and Window menu “Reset layout” follow L7 (owner 2026-10-08).

## 7. `ui.rs` split plan

Target: `varos-app/src/ui/` (binary crate first; moving to lib needs U1 types). Ranges at `221310f`, recomputed before each move.

| Module | Moves (current lines) |
|---|---|
| `ui/mod.rs` | `Op` 95-156, `Ui` + frame 798-1586, `apply_ops` 5985-6070 |
| `shell/kit/icons.rs` | `IC_*` 26-83, `TopIcons` 257, `LayerIcons` + `load_icon*` 855-895, `lucide*` 1590-1605, `DockIcons` 4345, `AbIcons` 5333, `dump_tool_icons` 6071 |
| `ui/style.rs` | fonts/style/zoom 1606-1630 |
| `ui/menus.rs` | legacy dropdown 1636-1714, menu rows 3229-3313 (later replaced by `kit::menu`) |
| `ui/controls.rs` | `Lab`, `doc_id`, `mini_btn`, `refpoint`, `icon_toggle`, `icon_btn`, `hsep` 1716-2030; `info_row/action_row/seg_btn` 5126-5192; `toggle_row/pill_btn` 5378-5420 |
| `ui/fields.rs` | `settle_field_edits` 1746-1755, `num_field` 1762-1939, `dim_field` 4077, `name_field` 5342-5376 |
| `ui/picker/{mod,panel,wheel,sliders,modes,harmony,harmony_rules,mini,cluster,fields,drawer}.rs` | Colour picker v3: modeless hand/lifetime, cached wheel math, target cluster/paint mirrors, K3 fields, swatch drawer |
| `ui/rail.rs` | `icon_button/divider` 3006-3034, `board_rail` 3851-3900 |
| `ui/topbar.rs` (tab strip) | caption buttons 3039-3228, focus seed + `TabDrag` 3315-3421, `build_topbar` 3422-3685, void/recovery/status 3692-3850 |
| `ui/ctlbar.rs` | `board_ctlbar` 3901-4076, chips/fill-stroke/shape slot 4089-4342 |
| `ui/layers.rs` | rows + cache + thumbs 265-624, `col_toggle` 4359, `panel_layers` 4394-4924 |
| `ui/properties.rs` | properties/document 4925-5125, align/pathfinder 5193-5330, artboard 5421-5738 |
| `ui/snap.rs` · `ui/rulers.rs` | `Snap/AbSnap/AbInfo` 625-797 · rulers/crosshair/HUD 5739-5982 |
| tests | each `mod *_tests` (6130-8215) moves with the code it tests |

1. One extraction = one **move-only** commit: `git diff --color-moved=zebra` shows only moved blocks plus `mod`/`use`/visibility lines; gates green (`cargo test --workspace`, clippy Mac + Windows target, fmt).
2. No behaviour, rename or style change rides in a move commit.
3. No new feature may add lines to `ui.rs`; new code goes to its target module or kit. Ratchet test `ui_rs_only_shrinks` (ceiling 8215, lowered with every move).
4. One owner of `ui.rs` at a time; moves never run parallel to a piece editing the same range.

## 8. Arabic / RTL gate (owner piece)

Fact: egui 0.35 shapes Arabic into wrong clusters (`لوحة أولى`: 9 scalars → 14 glyph records), wrong caret mapping and word order (U0-A record). Only `egui_tiles` is vendored today.

| Path | What | Cost / risk |
|---|---|---|
| A — patch egui | Vendor `epaint` text layout; add bidi (UBA) + HarfBuzz-class shaping; patch ledger + hash check like `tools/check_vendor_patches.ps1` | Smallest UI change; carries a fork through every egui upgrade |
| B — own shaping layer | Our shaper (bidi + shaping crate) produces galleys for labels; our own text field for editing | No fork; we own caret, selection, IME and clipboard — larger build |

Acceptance names (both paths): `arabic_name_shapes_joined_clusters`, `mixed_bidi_visual_order_matches_uba` ("Logo شعار v2"), `rtl_caret_moves_by_grapheme`, `arabic_selection_copy_paste_roundtrip`, `stored_text_unchanged`; owner hand check: type an Arabic and a mixed name in Layers rename, artboard name and Save As, and read them in tabs and Recent. **Off until it passes**: Plex Arabic in any field/label fallback; any claim of Arabic UI support; canvas Arabic text tool.

**Owner chose path B on 2026-10-07 → [ADR-0012](../adr/ADR-0012-ui-text-engine.md)** (proposed — awaiting owner): one `varos-text` engine for UI and canvas, `kit::text` painter for user strings, our own editor inside the K3 field law; pieces T1–T4.

## 9. Pieces (in order)

| # | Piece | Owns | Acceptance (owner one sentence + tests) | Defers |
|---|---|---|---|---|
| P1 | This spec v3 + UI_DIRECTION reconciliation | `docs/` only | Owner reads the summary; independent review APPROVE | — |
| P2 | Field commit law + `name_field` fix | `ui.rs` fields, `settle_field_edits`, `host` settle, `tokens.rs` `ERROR` | K3 hand test; tests `name_field_commits_typed_text_on_click_away`, `esc_reverts_without_history`, `unchanged_commit_is_not_dirty`, `invalid_keeps_focus_with_reason`, `save_and_tab_switch_commit_first`, `selection_change_commits_to_old_target` | one-undo arrow/scrub session (U3-T) **Landed on `main` 2026-10-04 in `fa6bd10`** (Codex Sol, 3 review rounds; GATE_LOG 2026-10-04 "UI_SYSTEM P2"): one session in `shell/kit/field.rs`, app glue `ui/fields.rs`, all six tests green; owner hand test of K3 not recorded. |
| P3 | Ratchets + single icon registry + first move-only split + FAINT→MUTED + QW6 sizes | `shell/ratchet_tests.rs`, `kit/icons.rs`, `ui/fields.rs`, `tokens.rs` (`ICON`, `T_MICRO`) | "Panel icons look bigger and grey labels are readable, nothing moved"; split, registry consolidation, FAINT rename, and exact ratchets **landed on `main` 2026-10-05 in `d882454`** (Opus review, 2 rounds; GATE_LOG 2026-10-05 "UI P3"). `T_MICRO = 10.5` and the `.size(10.0)` replacements later landed in the polish pass (`7adee37`). **Open:** QW6 panel icon size 18 (`ICON_LG = 18` exists for kit icon buttons and Align/Pathfinder; the rest of the panel icons not verified). | other modules |
| P4 | On-state + radius conformance | `icon_toggle`, `seg_btn`, radius literals, `R` comment | "Toggles show TOGGLE_WELL + TEXT (owner 2026-10-08), segments a grey block"; tests on fills per state | — |
| P5 | K2 fixes: canvas Tab no-op, focus rings on legacy controls | `main.rs` router, `ui/controls.rs` | "Tab on the canvas does nothing"; key-routing tests | Hide Panels |
| P6 | U2-P measure | perf counters, fixture generator | K4 table filled with real numbers | any cache work |
| P7 | Remaining moves (§7), one module per commit | `ui/*` | "Nothing changed"; `ui_rs_only_shrinks` lowered each time | — |
| P8 | Arabic gate (§8) | **path B chosen 2026-10-07 → [ADR-0012](../adr/ADR-0012-ui-text-engine.md)**: T1 `varos-text` crate · T2 read-only labels via `kit::text` · T3 editable fields (own editor in K3) · T4 canvas TextBox reuse (ADR-0010 P2) | §8 names + owner hand check | full RTL layout |
| then | U1-A/B/C → U2-O/D → U3-T/K/A/B/C → U4-S/M/P → U5-A/B → U6 (v2 order) | per v2 ownership; one owner per shared file | piece tests + owner window check | Workspaces, size-model persistence, pinch zoom, cache optimisation before P6, screen reader, vendor-neutral standard |

## Spec history (kept in place)

| Date | Change |
|---|---|
| 2026-09-27 | v2: K1-K6 contracts; v1 kept in [history](../history/UI_SYSTEM_V1_THROUGH_2026-09-27.md); [disposition](../foundation/work_orders/reviews/UI_SYSTEM.v2.disposition.md). |
| 2026-09-27 | U0-A fonts ([record](../foundation/work_orders/UI_U0_A_FONTS.md)): Plex Sans/Mono + Noto Symbols, sizes moved to tokens unchanged, Arabic diagnostic only. |
| 2026-09-27 | U0-B/C kit ([record](../foundation/work_orders/UI_U0_BC_KIT.md)): action/icon button, Home, list row, heading, notice; CPU tests 1×/2×. |
| 2026-09-27 | E2 ([record](../foundation/work_orders/DFS_S2_E2_START_INTEGRATION.md)): Start/Home/Recent on one open path; splash removed, GPU failure path kept. |
| 2026-10-04 | Batch review: kit menu + Lucide kit icons merged to main (GATE_LOG 2026-10-04). v3: values, tables, ratchets, K3 commit-on-blur law, split plan, pieces. |
| 2026-10-04 | P2 built: K3 in `kit::field` (one edit session for every text/number field), `ui/fields.rs` glue, `ERROR` + field tokens; Pathfinder buttons disabled with the core's reason. Known gaps in GATE_LOG. |
| 2026-10-05 | P3 worktree status after independent review: `ui.rs` 8,507→979 lines; real modules use `bar.rs` / `control_bar.rs` rather than the plan's `topbar.rs` / `ctlbar.rs`; tests remain consolidated in the real child module `ui/tests.rs`. `TopIcons`, `LayerIcons`, `DockIcons`, and `dump_tool_icons` remain outside `kit/icons.rs`; no separate `AbIcons` type exists on this branch. Legacy SVG sources are consolidated in `shell/kit/icons.rs` and raster/cache routes through `shell/svg.rs`. This owner-scoped piece necessarily lands the FAINT→MUTED/DISABLED rename and icon token substitutions beside the otherwise move-only split; keeping those requested visible changes separate would leave the intermediate tree inconsistent with P3 acceptance. Measured production-code ceilings (tests excluded): FontId 26, RichText size 33, raw colour 42, corner radius 26, `ui.rs` 979; `IC_` identifiers are forbidden outside the registry. QW6 `ICON`/`T_MICRO` work remains open. |
| 2026-10-06 | Status sync: P2 landed in `fa6bd10` (2026-10-04), P3 in `d882454` (2026-10-05); top bar 4b (`19611ae`) and the 12-item polish pass (`7adee37`, grey-segment half of P4, `T_MICRO = 10.5`) landed and were seen by the owner 2026-10-06. QW6 panel icon size stays open. This spec's own header status (proposed) is unchanged. |
