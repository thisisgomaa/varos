> **Status:** current — macOS window chrome + native menu bar design for `varos-app`, governed by `docs/foundation/FOUNDATION_CHARTER.md` §3.
# macOS Window Chrome + Native Menu Bar

**Date:** 2026-09-23
**Reason:** Ahmed on his Mac: "the whole top bar is broken, and the default Mac menu is not used at all."
Seen in his screenshots: (1) two title bars — the Mac one (traffic lights + title) above our own bar,
which still showed Windows ─ ☐ ✕ buttons; (2) the Mac title strip was see-through; (3) the menu bar
had only "Varos" — no File / Edit / View / Window, so ⌘Q and friends were missing.
**Rule:** Windows stays exactly as it is (every change is behind `cfg(target_os = "macos")` or a
platform constant whose Windows values are the old numbers). Menus are **mirrors only** — every item
runs a path that already exists; no new editor behaviour.

## A. One bar
- The window is created **decorated from the start** with `with_fullsize_content_view(true)`,
  `with_titlebar_transparent(true)`, `with_title_hidden(true)` (winit 0.30.13
  `WindowAttributesExtMacOS`). Our egui top bar now fills the title-bar area; the native traffic
  lights sit on top of it at the left.
- macOS never calls `set_decorations` (winit's `set_decorations(true)` rewrites the style mask and
  **drops** `FullSizeContentView` — `window_delegate.rs:1508-1522`). The splash-→-editor switch keeps
  only its Windows half.
- Platform table `chrome::TOPBAR` (pure, tested): macOS = 78 pt left inset for the traffic lights,
  no ─ ☐ ✕ caps, 6 pt right inset, 28 pt height (controls centred at the native 14 pt traffic-light
  centre); Windows/other = the old numbers (4 pt, caps on, 46 pt height).
- The window title string ("Untitled-1 · Varos α — Select (V)") is still set every change — hidden
  in the bar, but used by Mission Control, the Window menu and the Dock.
- **Drag:** Windows drags through the OS hit-test (`HTCAPTION` on the empty bar). macOS mirrors it:
  the same caption height + exclusion rects the bar already publishes (`cursors::set_caption`) are
  stored, and a left press on an empty bar spot calls `window.drag_window()`; a double-click there
  toggles zoom (what the native title bar did). Floating egui layers and active widget drags take
  priority over the caption geometry. Zoom requires two presses within 350 ms and 4 logical px,
  with no intervening window drag; dragging or zooming clears the pending click.

## B. Opaque
- `with_transparent(true)` (for the floating splash) is **off on macOS**; the NSWindow background is
  set to `#141313` (`tokens::BG`) through one small AppKit call, so nothing can show through — not
  the title strip, not the first frame before the GPU is ready, not a live-resize edge.
- Consequence: on macOS the splash card sits centred on the dark window (painted by egui) instead of
  floating over the desktop. Windows keeps the floating card.

## C. Native menu bar — `muda` 0.20 (MIT/Apache-2.0, Tauri's menu crate)
- **Why muda, not raw `objc2-app-kit`:** an NSMenu item needs an Objective-C target object for its
  action; muda already has that, key equivalents, check items and the standard items (About, Hide,
  Services, Minimize, Zoom, Full Screen). Raw objc2 would mean declaring our own Objective-C class —
  more unsafe code for no gain. muda is a **macOS-only** dependency (`[target.'cfg(target_os =
  "macos")'.dependencies]`), so the Windows build graph does not change. It brings `objc2` 0.6 /
  `objc2-app-kit` 0.3 next to winit's 0.5 / 0.2 (two versions, both compile; small cost). The
  window-background call uses those same 0.6/0.3 crates (declared directly, macOS-only).
- winit's own default menu is turned off (`with_default_menu(false)`); ours is installed on the
  first `NewEvents(Init)`. muda's click handler wakes the loop through an `EventLoopProxy`.
- **How items run:** a shortcut item is a synthetic keystroke — ⌘ + key goes into the **same**
  dispatch the keyboard uses (`OpenDocContext::shortcut`, extracted unchanged from the
  `KeyboardInput` branch). If a text field is focused, the keystroke is handed to egui instead —
  exactly what the keyboard path does (so ⌘Z still undoes typing in a field).
- A menu key equivalent consumes the key (AppKit asks the menu before the view), so ⌘S etc. never
  fire twice.

| Menu | Items (all existing paths) |
|---|---|
| Varos | About Varos (native panel, version from Cargo.toml) · Services · Hide ⌘H · Hide Others ⌥⌘H · Show All · Quit Varos ⌘Q (= the ✕ button path) |
| File | Open… ⌘O · Save ⌘S · Save As… ⇧⌘S · Close Window ⌘W (= ✕ path) |
| Edit | Undo ⌘Z · Redo ⇧⌘Z |
| Object | Transform Again ⌘D · Arrange ▸ (Bring to Front ⇧⌘] · Forward ⌘] · Backward ⌘[ · Send to Back ⇧⌘[) · Group ⌘G · Ungroup ⇧⌘G |
| View | Fit in Window ⌘0 · Actual Size ⌘1 · ✓Rulers ⌘R · ✓Guides ⌘; · ✓Lock Guides ⌥⌘; · ✓Smart Guides ⌘U · ✓Snap to Grid · ✓Snap to Point (the magnet menu) · Toggle Full Screen |
| Window | Minimize ⌘M · Zoom · ✓Tool rail · ✓Control bar · ✓every dockable panel (the bar's Window menu) · Bring All to Front |

Check marks are re-read from the real state every frame (only changed ones are written).

**Omitted (no existing shortcut/path):** New ⌘N (the burger row is a label; the "+" tab is a fake
tab, not a document) · Export… (burger row unwired) · Cut / Copy / Paste / Duplicate / Select All /
Deselect · Zoom In / Zoom Out (only Alt+wheel and Space-click zoom exist) · Delete (Backspace as a
menu key would steal it from text fields). Each lands when its home exists.

## Shortcut labels (done 2026-09-24)

`tokens::primary_mod_label()` / `shortcut_label()` are the shared source for primary-modifier
labels: ⌘ on macOS, the original Ctrl strings on Windows. Search, burger menu mirrors, Smart Guides
and the Group tooltip use them. Native menu accelerators already display AppKit's glyphs.
Dispatch and physical Ctrl support are unchanged. GPU-free test:
`shortcut_labels_use_the_platform_primary_modifier`.

The top bar now uses `chrome::TOPBAR.height` everywhere, including its drag band and menu edge.
The Mac bar matches the native 28 pt title area instead of leaving its controls 9 pt too low;
Windows retains 46 pt. `topbar_layout` computes the production control rectangles from the bar
rect and measured text widths. GPU-free test `mac_topbar_controls_share_the_native_traffic_light_centre`
checks every control, including tab close buttons, for containment and centring within ±1 pt of
the traffic lights at three window widths and two bar origins.

Review follow-up verification (2026-09-24): workspace **306 passed, 0 failed** (one obsolete easing
test removed); macOS and Windows-target Clippy with warnings denied and formatting all passed.
The earlier release build passed, but its launch found **no display** in the sandbox:
winit stopped with `invalid display ID`, and
`screencapture -x /tmp/varos-chrome-centred.png` failed (`could not create image from display`).
Centring is checked through shared production geometry; a real-window visual check remains pending.

## Known gaps (honest)
1. A nearby, timely double-click on empty bar space zooms; it still ignores the System Settings
   "double-click title bar" choice (minimize / do nothing). Floating UI and intervening window drags
   now block caption zoom (GPU-free regression tests); these fixes still need a real-window hand-test.
2. Close Window / Quit have no unsaved-changes prompt — same as the ✕ button on Windows today.
3. The splash no longer floats over the desktop on macOS (see B).
4. An opaque window that is covered gets no redraws, so a splash started behind another window
   stalled until the next input; fixed by repainting on `WindowEvent::Occluded(false)` (macOS only).
5. Measured 2026-09-23 (synthetic mouse/keys on this Mac): one bar + traffic lights, opaque strip,
   menu bar File/Edit/Object/View/Window, View ✓ marks match state, View ▸ Rulers by mouse and ⌘R
   by key each toggle exactly once, bar drag moves the window (7/9 tries; two presses lost right after
   a previous drag — cause unknown), double-click zooms/unzooms. **Not verified:** ⌘U produced no
   event in the test (possibly a global hotkey) · ⌘Z inside a text field · ⌘S/⌘O dialogs · Quit.
   Hand-test pending (Ahmed, batch).
