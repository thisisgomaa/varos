> **Status:** reference — spec/proposal for Ahmed's decision (charter §3, level 5); not an accepted decision.
# Varos UI System

Date: 2026-09-24 · Owner: Ahmed · Evidence baseline: the five audits `docs/audits/ui-2026-09-24/01…05` (read at `4821cf0`) + branch `claude/sweet-cerf-1sg30t` head `a28f82d` (QW5 merged; S1-B/C/D, S2/S3 wave 2, S5, S6 still in flight).
One system, not a list of fixes. Nothing here is implemented, scheduled or accepted by this document. Finding ids are cited as `01-B1` (audit 01, finding B1), `02-V-I4`, `03-F-B1`, `04-M1`, `05-D12` / `05-G06` / `05-R5`; the audits hold the file:line evidence, this spec does not repeat it.
It builds **on** DFS S1's frozen API (`AppCommand`, `SessionId`, `TabView`, `Workspace`, `Lifecycle`, `Ui::{set_tabs, take_app_commands, settle, document_switched}`) and S2 §3.7's Start page, never against them. `docs/UI_DIRECTION.md` stays the visual law; where the law contradicts itself, §7 Q4 proposes one resolution.

## 1. ملخص بالمصري

الأساس قوي، بس الواجهة اتبنت حتة حتة: خمس مراجعات لقت نفس الزرار مرسوم 15 مرة، ونفس الاختصار مكتوب في 5 ملفات، وبانلات بتوري أرقام مختلفة لنفس الحاجة.
النظام ده قانون واحد للبرنامج كله: كل قيمة ليها صاحب واحد، وكل زرار أو منيو أو اختصار بيبعت أمر من جدول واحد، وكل بانل بيتبني من نفس القطع (زرار، خانة رقم، سواتش، منيو، صف).
المنيو والكنترول بار بقوا «مرايات» بس: نفس الأمر ونفس الرقم ونفس الاختصار المكتوب، ومفيش زرار ميت — يا يشتغل يا يبان مقفول ويقولك ليه.
ليه دلوقتي؟ فيه باجات حقيقية: Cancel في الـColor Picker ممكن ما يرجّعش اللون، زرار Tab بيقفل كل الاختصارات، على الريتنا مقاسات المسك نص الحجم، وعلى الماك خطوط القانون مش بتتحمّل أصلًا.
الشكل مش هيتقلب: الأسود الدافي والأزرق المشرط وقواعد UI_DIRECTION زي ما هي؛ الجديد إن كل قاعدة ليها تيست أو فحص CI، مش بالذمة. «خلص» = كل بانل بيترسم في تيست من غير شباك، صفر لون أو مقاس بره التوكنز، صفر زرار ميت، وإنت جربت كل مرحلة بإيدك.
الشغل 7 مراحل U0…U6، كل حتة فيها يوم وكيل بالكتير، ومترتبة عشان ما تخبطش في شغل الملفات (S1/S2/S3) اللي شغال دلوقتي.
**قرار ١:** نعتمد النظام ده كأولوية واحدة بعد S1 على طول (U0 يبدأ دلوقتي لأنه ملفات جديدة بس)؟ المقترح: آه.
**قرار ٢:** نحط الخطوط جوه البرنامج نفسه: IBM Plex Sans للواجهة، IBM Plex Mono للأرقام، IBM Plex Sans Arabic محجوز للعربي — كلها رخصة OFL مجانية، ونبطّل ندوّر على خطوط ويندوز؟ المقترح: آه.
**قرار ٣:** نشيل الحركتين (الشبح اللي بيتزحلق ورا الماوس + الـglide بعد ما البوكس ينزل) والتوهج الأزرق، والسبلاش 1.55 ثانية لما صفحة البداية تنزل؟ المقترح: آه — ده رجوع عن موافقة 07-05، عشان «البرنامج يرد فورًا».
**قرار ٤:** التناقضات تتحل كده: زوايا 2 للعلامات الصغيرة / 3 للكنترولز / 8 للبوكسات / 11 لتابات البوكس بس، الفاصل 12، الـsmart guides وردي `#ff54a8` والـruler guides سماوي، والأخضر يتشال؛ و«المختار» في المجموعات = خلفية SURFACE وخط أزرق تحتها، مش أزرق مليان؟ المقترح: آه، ونكتبها في UI_DIRECTION في نفس الكوميت.
**قرار ٥:** ترتيب البانلات يتحفظ ويرجع زي ما سبته، مع أمر Reset Workspace، وعمود يمين ثابت 274، وأقل مقاس شباك 800×560 من غير ما حاجة تتقص؟ المقترح: آه.

## 2. The laws

Each law is one imperative, testable sentence; the **Test** column is how the gate proves it. Scope: all of `varos-app` UI code (`ui.rs` and the new `ui/`, `shell/`, `chrome.rs`, `main.rs` routing) and the canvas-overlay seam in core.

### A. Ownership & state flow
| # | Law | Test | From |
|---|---|---|---|
| L1 | Give every value exactly one owner — Document, Session (per tab: `Editor` + `View` + `DocUi`), App-UI (layout, hand visibility) or Host (window, platform) — and never copy it into another owner each frame. | A frame with no input leaves `editor.rev`, `doc.snap` and every Session field unchanged and emits zero `Request`s. | 01-A1, 01-M1, 03-F-G5, 04-A4 |
| L2 | Panels, hands and overlays read one immutable `Snap` built once per frame by `ui/snap.rs` (the only UI file that names `Editor`) and emit `Request`s; they never call an `Editor` method. | CI grep: `Editor` / `ed\.` in `ui/{panels,hands,overlays}` and `shell/kit` = 0. | 01-A3, 01 rules 1–2, 03-F-A1 |
| L3 | Send every UI mutation as exactly one `Request` — `Edit(EditCommand)`, `Transient(TransientCmd)` (only the Editor interfaces F4_DESIGN declares), `View(ViewCmd)` or `App(AppCommand)` — through the one outbox. | `fit_request`, `win_action`, `Op` and `apply_ops` do not exist (grep = 0). | 01-A3, 01-M7, 04-G4, 05-G10 |
| L4 | Model every multi-frame edit as an explicit `EditSession`: a modal session (colour picker) refuses canvas input and document commands, lifecycle commands settle it by Cancel, and a non-modal one (scrub, drag, rename) commits before any foreign `Request` runs. | Headless: picker open → canvas click → Cancel restores the colour; ⌘Z while open is refused; ⌘O cancels the picker first. | 01-B1, 01-A2, 04-B1, 04-A1, 04 rule 7 |
| L5 | Make `Editor::begin` debug-assert that no transaction is open. | A core test that nests `begin` panics in debug. | 01-M2, 04-A1 |
| L6 | Deliver a pointer release to the surface that received its press, and never to the Editor unless the press went to the canvas. | `route()` test: press on a panel, release on the canvas → no `pointer_up`. | 04-A2, 04-B1 |
| L7 | Bump a `live_rev` on every uncommitted live mutation and include it in the scene-cache key. | Page-colour live preview test: scene signature changes on each live frame. | 01-B3, 01-M3 |
| L8 | Keep per-document UI state in `DocUi` keyed by `SessionId` (swapped by `document_switched`) and app-wide UI state (layout, hands) out of it. | Switch tabs: Layers collapse/search belong to each tab; the layout does not change. | 01-A7, 03 "S1 must leave" |
| L9 | Make a command that changes nothing add no undo step and leave dirtiness unchanged. | Property test over every `EditCommand` with identity arguments: `rev` and history length unchanged. | 01-B6, 05-R5, 05-D12, 05-D23, 05-G22 |

### B. Commands & input
| # | Law | Test | From |
|---|---|---|---|
| L10 | Declare every argument-free user action once as a `CommandId` row in `ui/commands.rs` (label, per-platform chords, menu path, home), and render every key hint, accelerator, tooltip, status hint and window title from that table. | Grep: no `"Key[A-Z0-9]"` string or hint literal (`"(V)"`, `"Shift+O"`, `"Alt"`) outside the table. | 04 rule 1, 04-I3, 04-M4, 05-R4, 05-G19, 02-V-I9 |
| L11 | Route keys, native menu rows, in-app menus, buttons and the tab strip through one `dispatch(Request)`, never through a synthetic keystroke, and mirror each native row's enabled state from `resolve`. | `MenuCmd::Key` deleted; a focused text field + File ▸ Save / View ▸ Rulers still act; a disabled command's native row is disabled. | 04-I1, 04-I5, 04-G1, 01-A3, DFS §4 |
| L12 | Give every `CommandId` a visible home or menu row, and make every clickable control carry a `CommandId`/`Request` or be drawn disabled with a reason. | Table walk: each id has chord-or-menu and a `HomeId`; the kit's `Act` type has no "nothing" variant; menu walk: every row resolves or is `Off(reason)`. | 04-M4, 05-R1, 02-V-B3, 01-M6, 05-D02–D05 |
| L13 | Let canvas shortcuts yield only while a text editor has focus (`text_edit_focused`), let Tab move focus only among fields of the focused box, and let ⌃F6 cycle focus between boxes (Esc returns it to the canvas). | Press Tab with nothing focused, then V → Selection tool is active. | 04-B4, 03-F-M4, 02-V-M3 |
| L14 | Let one Escape (and one Enter) be consumed by exactly one owner, in the order text field → open popup → modal session → live gesture (cancel restores the pre-gesture state) → selection; Enter never clears the selection. | One test per state presses Esc once and asserts exactly one layer changed. | 04-M3, 04-B5, 04-B10, 04-I6, 01-B4, 01-M5 |
| L15 | Write every canvas tolerance, mark size, threshold and wheel step in logical px and convert it once, with the live scale factor (one source: winit `ScaleFactorChanged` = egui `pixels_per_point`) and the zoom. | Same gesture at scale 1 and 2 and zoom 0.05 / 1 / 40 gives the same result; `main.rs` holds no startup `scale`. | 04-M1, 04-B2, 04-B6, 01-B2 |
| L16 | Use one drag threshold (logical px of screen travel) for every tool, page and panel drag, and one double-click helper (OS interval, 500 ms fallback, logical-px radius). | A sub-threshold jitter on an object is a click (no undo step); canvas, caption, Layers and swatches call the same helper. | 04-B2, 04-B3, 04-I2, 05-G08 |
| L17 | Call one `reset_transient_input()` (modifiers, Space, pan, gesture) on focus loss and after every native dialog. | Dialog → Cancel → S does not pick Scale; ⌘-Tab away holding Space leaves no stuck hand. | 04-B7, 04 rule 11 |
| L18 | Keep input routing a pure `route(event, &InputState, &FocusSnap) -> Vec<Action>` with no `cfg(target_os)`, and read every platform difference from one `Platform` value. | `route` has headless tests for every gesture in audit 04; grep `cfg(target_os` in `ui/` and `route` = 0. | 04-A3, 04 rules 12–13, 05-R7, 05-G17 |

### C. Homes & panels
| # | Law | Test | From |
|---|---|---|---|
| L19 | Register every editable domain once in `ui/homes.rs` (`HomeId` → section, owner panel, command family), and allow each `PanelId` in the layout at most once. | Test fails if two homes claim a family; ☰ "change panel" to an open panel focuses it instead of duplicating it. | 03 rules 1/6, 03-F-B3, 03-F-M1 |
| L20 | Never make a home depend on the current tool or on another panel's empty state: Properties always stacks Transform, Appearance, Artboard, Document and Snapping as collapsible sections, disabled with a reason when not applicable. | Render Properties with each tool × {nothing, object, anchor} selected: all five section headers present. | 03-F-A2, 03 rule 13, 03 ONE-HOME map |
| L21 | Write every panel to the `Panel` contract — `ui(local, ui, &Snap, &mut Out)`, declared `min_size` and `ScrollPolicy`, no OS/global calls, safe to run twice per frame — and let the box own margins and the only vertical scroll. | Each panel renders from `Snap::default()` in a bare `egui::Context`; one `ScrollArea` per box (grep). | 03-F-A1, 03 rules 4–5, 03-F-B2, 03-F-G4, 01-A5, 01-I5 |
| L22 | Enforce each panel's minimum at layout time and make the standard layout meet every minimum at an 800×560 window. | Headless: every panel at its minimum has no horizontal overflow; the default tree at 800×560 satisfies all minimums. | 03-F-B6, 03-F-M2, 03 rules 7–8 |
| L23 | Make a mirror call its home's field function with a `Density`, and resolve constraints (ratio lock, reference point, artboard lock) inside the core command. | Reference point = centre, lock on: bar and Properties show the same X, and typing W in either keeps the ratio. | 03-F-B1, 03 rules 2–3, 01-I1, 01 rule 8 |
| L24 | Key every stateful widget by a stable domain id (node id, field id), never by display text, tooltip or screen position, and virtualise every list with unbounded rows. | Picker tooltips show words, not `cm-h`; a Layers click across a wheel-scroll lands; 10 000-row list builds ≤ visible rows. | 01-B5, 01-B7, 01-M4, 01-A6 |

### D. Visual
| # | Law | Test | From |
|---|---|---|---|
| L25 | Keep every colour, radius, size, spacing, font and timing literal in `shell/tokens.rs` and `shell/kit/`, and pass canvas-overlay colours into core and the renderer as a plain palette. | CI literal gate (L31); `varos-core/src/scene.rs`, `render-wgpu`, `cursors.rs` hold no copy of BG/ACCENT/guide colours. | 02-V-M1, 02 rules 1–2/12, 02-V-A1, 05-R2, 05-G11–G13 |
| L26 | Paint controls only through kit components, each implementing rest, hover, on, disabled (with a reason tooltip) and focus through one `look(kind, state)`. | Golden per component × state; panels contain no `painter().rect*` except content (thumbnails, rulers, canvas). | 02-V-G1, 02-V-A2, 02-V-M3, 02-V-M4, 02 rules 3–4/9, 01-G1 |
| L27 | Use azure only through ACCENT tokens for selection, the current tool/on state and keyboard focus — never for primary buttons, gradients, glows or pointer tracking. | Golden token names: ACCENT appears only in states `on`, `selected`, `focus`. | 02-V-I3, 02-V-I4, UI_DIRECTION rule 4 |
| L28 | Allow no motion: `animation_time` stays 0, and no UI code or vendored code interpolates, eases or glides toward a target. | Test asserts `animation_time == 0`; grep for per-frame lerps (`+= (… - …) *`) and the fork's glide state = 0. | 02-V-B2, 03-F-I4, 01-I3, 02 rule 10 |
| L29 | Draw every number in `T_NUM` (bundled mono), every section label in `T_MICRO` (uppercase, +1.2 tracking), and all informational text at ≥ 4.5:1 contrast (FAINT only for disabled/placeholder). | Token test computes contrast of each text token on PANEL/SEAM; goldens show `T_NUM` in every numeric field. | 02-V-I1, 02-V-I2, 05-R3, 05-D06, Astra F11 |
| L30 | Bundle the fonts in the binary, identical on every platform, and never read OS font paths. | Grep `Fonts/` = 0; test: `FontDefinitions` holds `varos-ui` / `varos-mono` before any OS call. | 02-V-B1, 02 rule 11, 01-G4 |

### E. Quality gates
| # | Law | Test | From |
|---|---|---|---|
| L31 | Run `tools/check_ui_literals.sh` in CI with a per-file allowlist that may only shrink. | CI fails when a count rises or a new file appears with literals. | 02-V-M5, 05-R2 |
| L32 | Render every panel and every kit state headlessly into a golden text snapshot, building its `Snap` without an `Editor`. | `cargo test` compares `tests/golden/ui/*.txt`; coverage test: every `PanelId` in the registry has a golden. | 01 "headless-testable", 01-A4, 03 measurements, 02-V-A2 |
| L33 | Run the egui_tiles vendor-delta check and the `egui_tiles` import-confinement check in CI on every push, cross-platform (no PowerShell dependency). | CI job runs `tools/check_vendor.sh` on Linux and macOS. | 03-F-A4, 03 rule 15, ADR-0006 |
| L34 | Deny `clippy::undocumented_unsafe_blocks` in `varos-app`, and keep fake data and dev entry points out of release builds. | Clippy gate; `--dump-*`/`VAROS_*` behind `cfg(debug_assertions)`; grep fake values (`"266"`, `"F0B429"`) = 0. | 05-R9, 05-R12, 05-R14, 05-G04, 05-G18 |

## 3. Architecture

**Flow (one direction):** winit event → `route()` → canvas `Action` | egui input → `UiFrame` reads `Snap` → panels/hands/menus emit `Request` (or a `CommandId` resolved to one) → host `dispatch()` → `EditCommand` / declared Editor interface / active session's `View` / S1 `Lifecycle::run(AppCommand)` → next frame's `Snap`.

### 3.1 Module layout (`varos-app`)
`ui.rs` stays where it is and shrinks; new submodules live in `src/ui/` (Rust allows `ui.rs` + `ui/`), so no mass move collides with in-flight DFS pieces. Reusable, window-free pieces live in the lib next to the tokens.
```
lib  shell/tokens.rs        the ONE token home (grows: type, space, heights, icons, state, canvas palette, timings)
     shell/fonts.rs         bundled font install (assets/fonts/*.ttf + OFL.txt)
     shell/kit/{look,button,icon_button,segmented,field,inline_edit,swatch,menu,row,section,separator,frame,tooltip,testkit}.rs
     shell/boxtree.rs       unchanged role: the ONLY egui_tiles importer (ruling 9)
     shell/registry.rs      PanelId, titles, min sizes, ScrollPolicy (dummy bodies deleted)
     storage/paths.rs       + layout()   (S3-A's resolver; one line)
bin  ui.rs                  winit adapter: Ui::new/on_event/run + S1 methods (target ≤ 400 lines)
     ui/frame.rs            UiFrame: headless frame(ctx, RawInput, &Snap, &mut UiState) -> FrameOut
     ui/snap.rs             Snap + caches — the only UI file that names Editor
     ui/request.rs          Request, TransientCmd, ViewCmd, EditSession
     ui/commands.rs         CommandId table, Chord/Mods, hints, resolve(); egui/muda key adapters derived here
     ui/dispatch.rs         dispatch(Request, &mut HostCtx) — extends S1-D's host dispatch
     ui/homes.rs            HomeId registry
     ui/panel.rs            Panel trait, Out, PANELS registry (the render hook BOX_SYSTEM_PLAN §4.2 promised)
     ui/panels/{properties/{transform,appearance,artboard,document,snapping},align,pathfinder,layers/{rows,dnd}}.rs
     ui/hands/{rail,ctlbar}.rs          mirrors: home field functions at Density::Compact
     ui/overlays/{rulers,ab_chrome,snap_hud,origin}.rs   take CanvasXform{view, ppp, board}
     ui/picker/{modal,plane,wheel}.rs   colour maths → varos-core pure module with tests
     ui/chrome/{topbar,statusbar}.rs    S1-C / S2-E2 own these; moved move-only in U6
     ui/layout.rs           LayoutFile load/save/reset
     input/route.rs         pure routing (moves out of the 490-line main.rs closure)
     platform/{mod.rs (Platform), cursors.rs, win_frame.rs, screen_sample.rs, pointer_poll.rs, mac_menu.rs, mac_caption.rs}
```

### 3.2 Core types (minimal; S1 names unchanged)
```rust
// ui/request.rs
pub enum Request {
    Edit(varos_core::EditCommand),
    Transient(TransientCmd),          // F4_DESIGN's declared Editor interfaces only (tool, paint focus, layer selection, previews)
    View(ViewCmd),                    // the active DocumentSession's View
    App(AppCommand),                  // S1's frozen enum, extended additively (S2/S3/S6 variants; WindowCmd below)
}
pub enum ViewCmd { ZoomIn, ZoomOut, ActualSize, Fit(FitTarget), PanBy([f32; 2]) }
pub struct EditSession { pub kind: SessionKind, pub modal: bool }
pub enum SessionKind { Picker(PickerTarget), Scrub(FieldKey), Rename(RenameTarget), Drag }
// S1 WindowCmd gains (additive, after S1-D): FocusPanel(PanelId), ReplacePanel { at: PanelId, with: PanelId }, ResetWorkspace

// ui/commands.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommandId { FileNew, FileOpen, FileSave, FileSaveAs, FileClose, AppQuit, EditUndo, EditRedo, EditCut, EditCopy,
    EditPaste, EditSelectAll, EditDeselect, EditDelete, ObjGroup, ObjUngroup, Arrange(ArrangeOp), Tool(ToolKind),
    ViewZoomIn, ViewZoomOut, ViewFit, ViewActualSize, ViewRulers, ViewGuides, ViewSmartGuides, SnapToggle,
    Panel(PanelId), ResetWorkspace, TabNext, TabPrev /* … one row per action */ }
pub struct Chord { pub key: Key, pub mods: Mods }          // Key = our enum; egui/winit/muda codes are derived from it
pub struct Mods { pub primary: bool, pub shift: bool, pub alt: bool } // primary = ⌘ on macOS, Ctrl elsewhere
pub struct CommandSpec { pub id: CommandId, pub label: &'static str, pub chords: &'static [Chord],
    pub menu: Option<MenuPath>, pub home: HomeId }
pub static TABLE: &[CommandSpec];
pub fn lookup(c: Chord, ctx: KeyCtx) -> Option<CommandId>;     // KeyCtx = Start shown / text focused / modal open
pub fn hint(id: CommandId, p: &Platform) -> Option<String>;    // "⌘S" / "Ctrl+S", "⌥" / "Alt"
pub fn resolve(id: CommandId, s: &Snap) -> Result<Request, Disabled>; // Disabled(&'static str) = the user-facing reason

// ui/snap.rs — one per frame; plain data, `Default` for tests
pub struct Snap<'a> {
    pub frame: FrameCtx,                 // live ppp, board rect, &Platform, FocusSnap
    pub sel: SelSnap, pub board: BoardSnap, pub doc: DocSnap, // today's Snap / AbSnap / ab_infos / document settings
    pub layers: &'a [LRow],              // cached on (session, rev, live_rev, DocUi key)
    pub tabs: &'a [TabView], pub active: Option<SessionId>,   // from S1's set_tabs
    pub session: Option<&'a EditSession>,
}

// ui/panel.rs
pub trait Panel {
    const ID: PanelId; const SCROLL: ScrollPolicy;             // Box | SelfManaged
    type Local: Default;                                         // lives in DocUi (per tab) or AppUi
    fn min_size() -> egui::Vec2;
    fn ui(local: &mut Self::Local, ui: &mut egui::Ui, snap: &Snap, out: &mut Out);
}
pub struct Out { pub requests: Vec<Request>, pub session: Option<SessionOp> }  // SessionOp = Open | Commit | Cancel

// ui/homes.rs
pub enum HomeId { Tools, Transform, Appearance, Colour, Align, Pathfinder, Layers, Artboard, Document, Snapping,
    Tabs, Start, Export, Workspace }
pub struct Home { pub id: HomeId, pub place: Place /* Panel(PanelId) | Hand | AppBar | BoardLeaf | Modal */, pub family: Family }
```
Host API kept and extended, not replaced: `Ui::set_tabs`, `take_app_commands` (becomes a filter over `take_requests()` until U6), `settle` (cancels the modal session, discards rename buffers — as S1 defines), `document_switched` (swaps `DocUi`). `Ui::run` loses `ppp` (U1-C, L15) and takes `&Editor` instead of `&mut Editor` once dispatch moves to the host (U3 end). Core additions (pure, no new deps): `SetObjectBounds { rect, anchor: RefPoint, keep_ratio }` and the artboard equivalent (L23); `Editor::begin` assert (L5); no-op guards (L9); `OverlayPalette` struct taken by `build_scene` (L25); tolerances in logical px (L15).

### 3.3 The kit (API sketch — every piece in `shell/kit/`, all states golden-tested)
```rust
pub enum Act { Cmd(CommandId), Req(Request), Off(&'static str) }       // a dead control cannot be written
pub struct State { pub hovered: bool, pub pressed: bool, pub on: bool, pub disabled: bool, pub focused: bool }
pub fn look(kind: Kind, s: State) -> Look;                             // fill, stroke, fg, radius — the only state→style map
pub fn icon_button(ui, key: WidgetKey, icon: Icon, size: IconSize /*Tool32|Bar26|Inline22*/, on: bool, act: Act) -> Response;
pub fn button(ui, key, label: &str, kind: ButtonKind /*Secondary|Ghost*/, act: Act) -> Response;
pub fn segmented<T: Copy + Eq>(ui, key, items: &[(T, &str)], current: T) -> Option<T>;
pub fn num_field(ui, key: FieldKey, spec: &NumSpec, value: Option<f32>, d: Density) -> NumEdit; // None | Live(v) | Commit(v) | Cancel
pub fn inline_edit(ui, key, current: &str, st: &mut InlineEdit) -> InlineOutcome;           // Commit(String) only when changed
pub fn swatch(ui, key, paint: SwatchPaint /*Solid|None|Checker*/, size: SwatchSize, target: bool) -> SwatchHit; // Focus | Open
pub fn section(ui, key, title: &str, collapsed: &mut bool, more: Option<HomeId>, body: impl FnOnce(&mut egui::Ui));
pub fn menu(ui, anchor: &Response, key, build: impl FnOnce(&mut MenuBuilder)); // m.cmd(id) · m.check(id, on) · m.sep() · m.sub(..)
pub fn list_row(ui, key, st: RowState /*selected|active|dimmed|dragged|drop*/, body: impl FnOnce(&mut egui::Ui)) -> Response;
pub fn separator(ui, s: Sep); pub fn frame(kind: FrameKind /*Box|Hand|Dialog|Menu|Hud*/) -> egui::Frame;
pub fn tip(r: Response, t: Tip /*Plain(&str)|Cmd(CommandId)|Disabled(&str)*/) -> Response;
```
Menu rows take a `CommandId`, so label, hint and enablement come from the table. `testkit.rs` renders a component in a bare `egui::Context` and writes the golden: one line per shape (widget key, rect rounded to 0.5 pt, text, font token, colour **token name** — an unknown colour fails the test).

### 3.4 Token schema (`shell/tokens.rs`; values marked ★ depend on Q4)
| Group | Tokens |
|---|---|
| Colour ramp | `BG PANEL SURFACE HOVER ROW_HOVER INPUT_WELL LINE LINE2 TEXT MUTED FAINT SEAM` (current values) |
| Accent & state | `ACCENT ACCENT_SEL ACCENT_TINT ACCENT_SOFT` (tokenises the α 0.5 row edge) · `ON_ACCENT` #fff (only on ACCENT) · `FOCUS` = ACCENT · `DISABLED_A` 0.4 |
| Canvas palette → core | `SEL` = ACCENT · `SEL_FILL` · `HANDLE` · `SMART_GUIDE` ★ #ff54a8 · `RULER_GUIDE` ★ current cyan · `PAGE_EDGE PAGE_EDGE_T AB_GHOST PAPER DOT_GRID RULER_BG` (warm, per 02-V-I5) |
| Radii | `R_MARK` ★ 2 (swatches ≤ 18, glyph marks) · `R` 3 controls · `RBOX` ★ 8 boxes/hands/menus/dialogs · `RCAP` ★ 11 box-tab pills only |
| Space | `SP_1…SP_6` = 2 · 4 · 8 · 12 · 16 · 24 · `SEAM_GAP` ★ 12 · `BOX_PAD` 12×10 |
| Sizes | `H_ROW` 26 · `H_FIELD` 25 · `H_FIELD_BAR` 24 · `H_SEG` 22 · `H_TAB` 28 · `H_BOXTAB` 22 · `H_BOXHEAD` one value · `TOOL` 32 · `BAR_BTN` 26 · `INLINE_BTN` 22 · `IC` 16 · `IC_S` 13 · `SW` 18 · `SW_S` 16 · `RIGHT_COL` ★ 274 · `RAIL_W` 44 · `BAR_H` 36 · `HAND_INSET` 12 · `RAIL_BAND` 60 · `BAR_BAND` 58 · `WIN_MIN` 800×560 · `STATUS_H` |
| Type (≤ 6 steps) | `T_MICRO` 9.5 / 600 / +1.2 / upper · `T_SMALL` 11 · `T_BODY` 12 · `T_TITLE` 13 · `T_NUM` mono 11 |
| Stroke | `HAIRLINE` 1 pt · `FOCUS_W` 1 · `SEL_EDGE_W` 2 · `NONE_SLASH_W` 1.4 |
| Timing | `ANIM` 0 · `TOOLTIP_DELAY` · `DOUBLE_CLICK_FALLBACK` 500 ms · `DRAG_THRESH` 4 logical px |

Platform-dependent sizes (Mac app bar 28 / Windows 46) come from `Platform`, not from tokens. A token test compares the law values with `UI_DIRECTION.md` / mockup `:root`, so a value change updates the law in the same commit (02 rule 14).

### 3.5 Layout persistence
`LayoutFile { version: 1, tree: <egui_tiles Tree<PanelId> serde>, rail: bool, bar: bool, collapsed: BTreeMap<SectionId, bool> }` at `storage::paths` → `<data root>/layout.json`, written through S3-A's `durable::write_replace` after each `WindowCmd` that changes layout and at quit; read once at launch. Unknown version, parse error, a duplicate `PanelId`, a missing Board or an unknown panel → `ShellState::standard()` plus status text "Layout reset — the saved layout couldn't be read." `CommandId::ResetWorkspace` (Window menu + box ☰) restores `standard()`. The layout is app-wide, never per document (L8). The tree's serde format is owned by the fork, so `version` is bumped whenever the fork changes it.

### 3.6 Platform seam
`platform::Platform { os, primary: ModKey, glyph_labels: bool, native_menu_bar: bool, caption: CaptionKind, appbar_h: f32, screen_eyedropper: bool, pinch: bool, double_click_ms: u32 }`, built once at launch (`Platform::current()`, `Platform::fixture(os)` for tests) and handed to `Snap.frame`. Panels read "not available on this platform" from it (05-R7). `cursors.rs` (1,244 lines) splits into `platform/{cursors, win_frame, screen_sample, pointer_poll}.rs` with a `SAFETY:` comment on every `unsafe` (05-G16, L34).

### 3.7 Deleted
Sandbox dummy bodies, `draw_board`/`draw_hands`/unhosted `ShellState::ui`, stale `shell-sandbox` and `BoxState` docs (01-G3, 03-F-G2, 05-G04); dead `to_json`-only path (replaced by §3.5); the `Op` enum + `apply_ops`, `fit_request`, `win_action` (L3); per-frame `SetSnapConfig` / `set_constrain_wh` (L1); `apply_key` string matching, `shortcut`, `egui_key`/`muda_code` hand tables, `ui.rs:925` key list and every hint literal (L10); ~15 icon-button painters, 7 swatch painters with 8 none-slash copies, 6 tab/segment painters, 6 separators, 2 menu systems, 4 text inputs, 3 artboard-rename editors → kit (`inline_edit` replaces all three, 01-I2, 05-G06); token aliases `ui.rs:22-26` (02-V-G2); `C:/Windows/Fonts` lookup (L30); drag-ghost ease, fork glide (~90 lines of `tree.rs`), azure gradient glow (Q3); inner ScrollAreas in Properties/Artboard (03-F-B2); the Properties›Shape Pathfinder copy (§7); `too_many_arguments` allows (05-G24); frame-loop debug file writes and the cwd panic breadcrumb (04-G2, 05-D24); release-build dev flags (05-G18). The splash hold goes when Start lands (Q3).

## 4. UX contract (one behaviour per control kind)

| Control | Contract |
|---|---|
| Inline edit (layer, artboard, document names) | Double-click the name or press Enter on a focused row to edit. **Enter or click-away commits, Escape cancels.** Unchanged, empty or whitespace-only → no-op: no undo step, no dirty dot (empty keeps the old name, §7). Core trims with `clean_name` (bidi-safe, 05-D25). One component, one rule, every surface. |
| Numeric field | Value in `T_NUM` mono, micro-label **inside** the field (mockup `.fld`). Click selects all; type; **Enter / Tab / click-away commits, Escape reverts**. ↑/↓ = ±1 unit, ⇧ ±10, ⌥ ±0.1 (not Ctrl: Mission Control, 04-I4). Drag the label to scrub (same `DRAG_THRESH`; ⇧ ×10, ⌥ ×0.1); a whole scrub is one `EditSession` = one undo step. Invalid text reverts without a step. Mixed values show "—". Ratio lock and reference point travel in the command and are resolved in core (L23). |
| Button / icon button | Acts on release inside. States from `look()`. Resting icons MUTED, hover HOVER fill + TEXT icon, current tool ACCENT fill + ON_ACCENT icon. Toggles and segments "on" = SURFACE fill + 2 px ACCENT underline (mockup `.cbtn.on`), not an azure block. No button is "primary azure". |
| Disabled | `DISABLED_A`, no hover, tooltip states the reason ("Export isn't available yet — …", "Select an object to edit its size"). The kit cannot draw a disabled control without a reason. |
| Menus (☰, box menu, magnet, context, native) | One kit system. Rows come from the table: label left, hint right (platform glyphs), check mark, disabled-with-reason. Opens on click; ↑/↓/→/←/Enter; Esc closes the menu only (L14); a click outside closes it and does not reach the canvas. Native menu rows use the same `CommandId`s and enablement. |
| Tooltips | After `TOOLTIP_DELAY`: name + hint from the table ("Pen  P"); disabled → the reason. No tooltip repeats a key in its label text. |
| Swatches | Click = set paint focus; double-click = open the Colour home. The same on the rail, the bar and Properties (03 ONE-HOME map). |
| Lists (Layers) | Click selects, ⇧ range, ⌘ toggle, double-click name = inline edit, right-click = context menu from the table, drag with the one threshold; virtualised. |
| Box header | ☰ lists `DOCKABLE` panels + Reset Workspace; choosing an open panel focuses it (L19). One header height whatever the tab count (03-F-I3). |
| Modal session (picker) | While open: canvas and document commands refused (L4); OK/Cancel are neutral kit buttons; Enter = OK only when no field is focused; Esc per L14; a lifecycle command cancels it first (S1 `settle`). |

**Floating control bar placement.** Anchored at the board's top-left: top = `HAND_INSET`, left = board left + `RAIL_BAND` (a fixed left edge, never centred, so it never jumps; 05 Needs-Ahmed #2 default). Width is a **fixed class per mode** — Idle/tool options S, Direct/Artboard M, Object L (token values measured in U4); slots are stable within a mode (disabled, never removed); the selection name sits in a fixed slot with middle ellipsis. Width is clamped to board width − `RAIL_BAND` − `HAND_INSET`; trailing slots fold into "…" (jumps to the home); below a minimum board width the bar hides and the homes still work. **Fit never puts art under a hand:** the fit rectangle is the board minus `BAR_BAND` at the top and `RAIL_BAND` at the left (when those hands show), including the artboard name label above the top edge (03-F-I1/I2, 03 rules 11–12, 05-D20). The hands sit inside the rulers, never over them.

**Start page.** Exactly S2 §3.7: inside the Board leaf, no hands, no rulers, Home chip in the tab strip. The UI system adds only that Start is built from kit pieces (`button`, `list_row`, `section`), its actions are table rows (`FileNew`, `FileOpen`, Open Recent as `Request::App`), and its shortcut hints come from `hint()`.

**Keyboard focus look.** Any focused kit control shows a 1 pt `FOCUS` outline inset by 1 pt at its own radius; text fields keep ACCENT border + INPUT_WELL. ⌃F6 cycles boxes (focused box shows the outline on its header), Tab cycles fields inside it, Esc returns to the canvas (L13, 03-F-M4).

**Retina sizing.** Every chrome size is in points; icons are rasterised at `ppp × size` from the live scale factor; hairlines are 1 pt snapped to physical pixels. Canvas marks and hit radii use logical px (L15), so on a 2× Mac a transform handle is 7 pt, not 3.5 (04-M1). Smallest text: `T_MICRO` 9.5 pt for labels only; information text ≥ `T_SMALL` 11 pt.

## 5. Gated build plan

Cap: each **piece** ≤ one agent-day; a stage is its pieces plus sequential merges. Every piece: own worktree, the per-merge gates of DFS S1 §5 (test, clippy `-D warnings`, fmt, Windows and Mac target clippy) + L31/L33 once they exist, moderator diff review, independent Codex review before `main`, and a plain-Arabic hand-off. UI look and feel is Ahmed's hand acceptance, never inferred from headless passes. If a piece misses its day, stop and report it **not passed**.

**Hot-file queue (one owner at a time, rebase + re-run headless tests before each merge):**
| File | Order |
|---|---|
| `main.rs` | S1-D → **U1-B/C** → S2-E2 → S3-F1 → S3-F2 → **U4-C** → **U5-B** |
| `chrome.rs`, `mac_menu.rs` | S1-C → **U1-A** → S2-E2 (Open Recent rows join the table) → S6-C |
| `ui.rs` | S1-C/D → **U2** → U3-A, C, D, E (disjoint regions) ↔ E2 (topbar, Board leaf) → F1 (Document section) → **U3-B** → F2 → **U4** → **U6** |
| `boxtree.rs`, `registry.rs` | **U2** (render hook) → **U4-A** → **U4-B** → **U6** |
| core `editor.rs` / `command.rs` | **U3-A** → **U3-B** → **U5-A** (S5 owns `file.rs`/`model.rs` format code, not these) |
| `shell/tokens.rs` | **U0-A** (add, no value change) → **U3-F** (Q4 values + law docs) |

**U0 — gates, tokens, fonts, kit skeleton** · 4 pieces × S · **starts now** (new files + one `install_fonts` hunk + CI).
- *Pieces:* U0-A tokens (new groups in `tokens.rs`, `OverlayPalette` in core `scene.rs` with today's values, renderer/cursors take BG from it). U0-B CI: `tools/check_ui_literals.sh` + baseline allowlist, `tools/check_vendor.sh` (port of the `.ps1`) + import confinement, `undocumented_unsafe_blocks` deny + SAFETY comments (`cursors.rs`, `single_instance.rs`). U0-C `shell/fonts.rs` + `assets/fonts/` + `OFL.txt`; `install_fonts` calls it (needs Q2). U0-D `shell/kit/` skeleton: `look()`, `Act`, `testkit` golden harness, `icon_button` + `button` + `separator` with 5-state goldens; no call site migrated.
- *Evidence:* all gates green; literal baseline recorded in GATE_LOG; kit goldens; font test (no OS path); palette test (core colours bit-equal to before).
- *Ahmed:* only the font changes — labels and numbers on the Mac now use the chosen face; nothing moved.
- *Closes:* 02-V-B1, 02-V-M1, 02-V-M5, 03-F-A4, 05-R9, 01-G4 (fonts). Astra F11 in part (with QW6).

**U1 — command table + one dispatch** · 3 pieces · **after S1-D merges, before S2-E2**.
- *Pieces:* U1-A (M) `ui/commands.rs` + `Key/Chord/Mods` + derived egui/muda adapters, replacing `chrome.rs` / `mac_menu.rs` tables and `MenuCmd::Key`; hints from `hint()`. U1-B (M) `ui/dispatch.rs` extending S1-D's dispatch; delete `apply_key` strings and `shortcut`; native rows' enablement from `resolve`. U1-C (S) the three P1 input fixes that need the same `main.rs` slot: pointer capture (L6), one scale factor (L15 part), `text_edit_focused` (L13 part).
- *Evidence:* table walk (every id: chord-or-menu + home); grep gates for key strings/hint literals/`MenuCmd::Key`; Mac/Windows hint fixtures; ⌘S and View ▸ Rulers work with a field focused; release-after-panel-press test; Tab-then-V test.
- *Ahmed:* every menu row works while typing in a field; hints read ⌘ ⌥ ⇧ on the Mac; Tab no longer kills V/⌘Z; click inside the picker then Cancel reverts.
- *Closes:* 04-I1, 04-I3, 04-I5, 04-G1, 04-B1 (main path), 04-B4, 04-B6, 01-B2, 01-A3 (menus), 02-V-I9, 05-G19.

**U2 — Snap, panel contract, registry; one panel as the pattern** · 2 pieces × M.
- *Pieces:* U2-A `ui/{snap,request,panel,homes,frame}.rs`; registry render hook replaces the `ui.rs:1332` closure; `DocUi` keyed by `SessionId` (wires `document_switched`); `Op` kept as a translation shim until U3 ends. U2-B migrate **Align** (the exemplar mirror) and the bar's align slots to `ui/panels/align.rs` on kit pieces.
- *Evidence:* Align golden from `Snap::default()`; click Align Left → exactly one `Request::Edit(Align…)`; grep `Editor` in `ui/panels` = 0; `UiFrame` runs from `RawInput` alone.
- *Ahmed:* Align panel and the bar's align buttons do the same as before; icons MUTED at rest.
- *Closes:* 03-F-A1 and 03-F-G1 (pattern), 01-A4, 01-A7, 01-A5.

**U3 — the rest of the panels** · 6 pieces, ≤ 1 agent-day each, parallel worktrees, sequential merges.
- *Pieces & ownership:* U3-A Properties Transform + Appearance sections, `num_field` + `swatch` kit, core `SetObjectBounds{anchor, keep_ratio}`; U3-B Artboard + Document + Snapping sections, `inline_edit` + core rename guards (**after F1**); U3-C Layers (`ui/panels/layers/*`, virtualised rows, stable ids, `list_row`); U3-D Pathfinder + rail + control bar as `Density::Compact` mirrors, `menu` kit replaces `menu_below` and the egui box menu; U3-E picker as a modal `EditSession`, `live_rev`, `Editor::begin` assert, swatch gesture unification; U3-F Q4 token values + `UI_DIRECTION.md`/mockup `:root` in the same commit + overlay palette flip. U3 ends by deleting `Op`/`apply_ops`; `Ui::run` takes `&Editor`.
- *Evidence:* golden + interaction test per panel; mirror test (centre ref point + lock: same X in bar and Properties); rename tests (Esc cancels; unchanged = no `rev` bump) on all three former editors; picker tests (canvas click refused, ⌘Z refused, second open settles, Cancel reverts); L9 property test; 10 000-row Layers build test; grep `Editor|ed\.` in `ui/panels|hands` = 0; literal allowlist drops for every migrated region.
- *Ahmed:* Properties shows the same five sections in every tool; bar X = Properties X; renaming an artboard behaves the same in all three places and click-in/click-out leaves no `*`; picker Cancel always reverts; Layers stays smooth on a big file; the new radii/guide colours look right.
- *Closes:* 01-B1, B3, B4, B5, B6, B7, I1, I2, I5, I6, A1, A2, A6, G1, G2; 03-F-B1, F-B2, F-B4, F-A2, F-G4, F-G5, F-G6, F-I5, F-I7; 02-V-G1, V-G2, V-I1–V-I6, V-I8, V-A1–V-A3, V-M2–V-M4; 05-D12–D14, D23, D25, G06–G09, G11–G13, G15; Astra F10 regressions.

**U4 — layout persistence, minimums, floating bar** · 3 pieces × M.
- *Pieces:* U4-A `ui/layout.rs` (§3.5), `ResetWorkspace`, uniqueness in ☰/toggle, one Window-menu path (egui and native both send `WindowCmd`). U4-B size policy: `RIGHT_COL` fixed, per-panel minimum at layout time, default shares valid at `WIN_MIN`, one header height. U4-C hands policy + Fit bands (§4), absorbing QW8 if not yet merged.
- *Evidence:* serde round-trip; corrupt / duplicate / old-version file → standard layout; minimum test at 800×560; bar width-class and clamp tests at 1460/1024/800; Fit test: fitted page and its label never intersect the bands.
- *Ahmed:* move panels, quit, relaunch → same layout; Reset Workspace; shrink to the minimum window → nothing clipped; bar does not jump between tools and never covers the page after Fit.
- *Closes:* 03-F-B3, F-B5, F-B6, F-I1, F-I2, F-I3, F-I6, F-A3, F-M2, F-M3; 05-D20, G05; Astra control-bar observation.

**U5 — input contract** · 3 pieces.
- *Pieces:* U5-A (M) core tolerances in logical px: `DRAG_THRESH`, size filters, object/page drag threshold, named handle/ring radii. U5-B (L→ split if needed) `input/route.rs` pure router out of the `main.rs` closure; Escape/Enter ladder; `reset_transient_input`; Space-pan vs chrome; hover/cursor leftovers; one double-click helper; `event.repeat` guard. U5-C (S) trackpad: `PinchGesture` zoom at cursor, `PixelDelta` 1:1 pan.
- *Evidence:* `route()` tests for every audit-04 gesture; same-gesture tests at scale 1/2 × zoom 0.05/1/40; Esc ladder test per state; dialog/focus-loss reset tests; grep `cfg(target_os` in routing = 0.
- *Ahmed:* on the Retina Mac handles and hover reach feel the designed size; a small jitter never moves an object; Esc closes a menu without deselecting; pinch zooms; holding keys never queues dialogs.
- *Closes:* 04-B2, B3, B5, B7–B10, I2, I4, I6, A1–A4, M1–M3, G3; 05-D15, G21.

**U6 — deletions, gates to zero, the standard** · 3 pieces × M.
- *Pieces:* U6-A deletions of §3.7 still standing, `cursors.rs` split + `Platform`, move-only `ui/chrome/*` after S2/S6 settle, `#![allow(deprecated)]` scoped, `ToggleDock` → `ToggleBar` rename. U6-B allowlist to zero (or a named residue table reviewed by Codex), golden coverage test for every `PanelId` and kit state, DEBT-marker listing (05-R8). U6-C documents: `UI_DIRECTION.md` rewritten as the current law (resolved numbers, kit, the 34 laws); BOX_SYSTEM_PLAN stamped historical; a vendor-neutral `docs/reference/DESKTOP_UI_STANDARD.md` (laws, token schema, kit contract, gates) that another desktop tool could adopt, with a test that its token table matches `tokens.rs`.
- *Evidence:* all L31–L34 gates at zero; doc-vs-token test; `ui.rs` ≤ 400 lines, no module > 1,500 (05-R11).
- *Ahmed:* reads the Arabic summary of the standard; one full walk through the app on the final batch list.
- *Closes:* 01-G3, 02-V-G4, V-G4b, 03-F-A5, F-G2, F-G3, 05-G03, G04, G16–G18, G23–G27, D24.

Total ≈ 24 agent-days of pieces; U0 runs now, U1 right after S1-D, the rest interleaves with DFS wave 2 by the hot-file queue.

## 6. Risks & open questions (recommended default each)

1. **Collision with DFS wave 2** (E2/F1/F2/S6 also edit `ui.rs`/`main.rs`). *Default:* the hot-file queue in §5; U1 before E2 so new DFS commands are table rows from birth; if E2 goes first, U1 absorbs its rows.
2. **egui text limits.** egui 0.35 as used here applies no OpenType features, and Arabic shaping in the UI is **not verified**. *Default:* tabular numbers come from a mono face (`T_NUM`), chrome stays Latin for V1, Plex Sans Arabic is bundled as a fallback only; Arabic UI shaping is a later system with its own spike.
3. **Golden brittleness** on an egui bump. *Default:* text goldens (rects rounded to 0.5 pt + token names, not pixels), regenerated only by an explicit `UPDATE_GOLDEN=1` run reviewed in the diff; no new dependency (`egui_kittest` not needed).
4. **Removing the glide edits the fork.** The file set stays the five ADR-0006 files. *Default:* update the `tree.rs` row of `docs/VENDOR_PATCHES.md` in the same commit; no ADR supersession — Codex review confirms the contract reading.
5. **Modal picker vs "live canvas" wish** (`main.rs` comment). *Default:* modal for V1 (L4); a docked Colour section, if Ahmed wants it later, gets its own session design.
6. **Core command changes** (`SetObjectBounds` fields, begin assert, no-op guards, tolerances) change `EditCommand` shapes and F3 characterisation tests. Commands are not persisted, so no format impact. *Default:* core pieces stop and report if an existing test must change.
7. **Per-frame cost** of `Snap` (document colour scan, row hash). *Default:* cache on `(session, rev, live_rev)`; measure on the P11.2 harness before and after U3-C.
8. **Accessibility** (Astra: canvas controls invisible to assistive tech). *Default:* out of this system; stable ids (L24) and keyboard focus (L13) prepare it; AccessKit is a later spike.
9. **Day cap on U3/U5-B.** *Default:* split the piece, never drop a test; report the stage not passed.

## 7. Needs Ahmed (the 27 audit items, deduplicated; default = proposed)

**The five decisions**
| Q | Decision | Default |
|---|---|---|
| Q1 | Adopt this UI system as one priority right after DFS S1 (U0 now, new files only). | Yes |
| Q2 | Bundle fonts: IBM Plex Sans (400/600) for UI, IBM Plex Mono for numbers, IBM Plex Sans Arabic reserved; SIL OFL 1.1, licence shipped; no OS font lookup. (02-V-B1) | Yes |
| Q3 | Remove the drag-ghost ease, the fork's 250 ms glide and the azure gradient glow (flat azure outline for the drop slot); remove the 1.55 s splash hold when Start lands. Reverses the 07-05 approval. (01-I3, 02-V-B2, 02-V-I3c, 03-F-I4, 05 #21) | Yes |
| Q4 | Law numbers: radii 2 / 3 / 8 / 11 (11 = box tabs only); seam 12; smart guides magenta `#ff54a8`, ruler guides cyan, selected segment = ACCENT, green retired; "on" = SURFACE + 2 px azure underline, current tool = azure fill; no azure primary button. Written into UI_DIRECTION + mockup `:root` in the same commit. (02-V-M2, V-I3, V-I4, V-I8, 05 #19, #20, #22) | Yes |
| Q5 | Save/restore the layout + Reset Workspace; right column fixed 274 pt; minimum window 800×560 with nothing clipped. (03-F-B5, F-B6) | Yes |

**Smaller UI items (working default; hand-check at the named stage)**
| Item | Default |
|---|---|
| Control-bar anchoring (P8, 05 #2) | Fixed left edge + width class per mode (§4) — U4 |
| Resting icon grey / QW6 look (05 #4) | MUTED at rest; FAINT only disabled/placeholder — U0/U3 |
| Artboard ⋮ menu gated to the Artboard tool (03-F-B4) | Un-gate, as LAYERS_VISION says — U3-B |
| Pathfinder also inside Properties›Shape (03 map) | Remove the copy; Pathfinder panel is the home, the bar keeps a mirror — U3-D |
| Colour home: modal picker vs docked section (03 map) | Modal stays the one home for V1; click = focus, double-click = open everywhere — U3-E |
| Artboard/Document hidden by tool or selection (03-F-A2) | Always-present collapsible sections in Properties — U3 |
| Enter clears the selection (04-I6) | Enter never deselects — U5 |
| Fine-step modifier (04-I4) | ⌥ = ×0.1 (Ctrl clashes with Mission Control) — U3-A |
| Tab on the canvas | Does nothing (no focus grab); "Tab hides panels" can come later — U1 |
| Emptied rename field (05 #17) | Keeps the old name — U3-B |
| Board as a special box vs "no box is special" (05-G23) | Amend the law: Board = one non-tabbable box type — U6 |
| Mac screen eyedropper (05 #8) | Defer; `Platform` shows "not available on Mac yet" |
| Mac window memory (05 #6) | QW2-lite; `layout.json` sits beside it — U4 |
| Tab overflow, Share disabled, red light = Quit (05 #9, #11, #12) | S1 defaults stand |

**Outside this system (audit 05 defaults stand):** #1 Pen on a middle anchor, #3 Duplicate row, #5 marquee/rotate ring, #7 board activation dirty, #10 per-tab paint, #13–#16 DFS S2–S6 items, #18 Delete vs Clear, #23–#27 eye tests, `codex/p6-header`, program items (cargo-audit, MCP, online, Compositor).
