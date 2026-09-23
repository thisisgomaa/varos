> **Status:** reference — study/proposal for Ahmed's decision (charter §3, level 5); not an accepted decision.

# Varos online (like Figma) + a Mac version — feasibility study

- **Date:** 2026-09-23
- **Question from the product owner:** "Make Varos 100% online like Figma, with a Mac version too."
- **Nature:** study only. No code was changed, nothing was committed. Nothing here overrides an ADR; where an idea conflicts with an accepted ADR, this study says so and names the ADR.
- **Evidence rules:** code claims carry `file:line` (paths relative to the repo root). Claims about browsers, Apple, prices, or third-party libraries that could not be checked offline on this machine are marked **(unverified)**. Effort numbers are estimates for *one designer + AI sessions working in gated stages*, with ±50% uncertainty.

---

## 1. الخلاصة بالمصري (للقرار)

1. فكرة "Varos أونلاين زي Figma + نسخة ماك" ليها **٣ أشكال واقعية**، وكل واحد تمنه مختلف خالص:
2. **(أ) برنامج على الجهاز لماك وويندوز:** ده تقريبًا ماشي — المحرك كله اشتغل على الماك، وفاضل غلاف البرنامج (شغال عليه دلوقتي) + التغليف والتوقيع من Apple. التمن: **١–٢ شهر** + حوالي **٩٩ دولار في السنة** لـ Apple.
3. **(ب) نفس الكود جوه المتصفح (WebAssembly + WebGPU):** ممكن، لأن أدواتنا بتدعم المتصفح، بس فيه عوائق حقيقية: فتح وحفظ الملفات، الخطوط، التوقيت، وطريقة تشغيل البرنامج. والمصمم **مش هيقدر يستخدم الخطوط المتسطبة على جهازه** بسهولة جوه المتصفح. التمن: تجربة صغيرة **٢–٤ أسابيع**، ونسخة متصفح حقيقية **٣–٦ شهور** زيادة، وبعدها كل ميزة جديدة لازم تتجرب في مكانين.
4. **(ج) سحابة حقيقية زي Figma** (كذا حد على نفس الملف في نفس اللحظة + حسابات + سيرفر): ده **منتج تاني**. محتاج سيرفر شغال ٢٤ ساعة، أمان وحسابات، وتغيير في قلب طريقة حفظ الرسمة. التمن: **١٢–٢٤ شهر على الأقل** + مبرمج سيرفرات بشري + فلوس استضافة كل شهر. الـ VPS بتاعنا (٢ معالج / ٨ جيجا) ميستحملش ناس كتير.
5. **التوصية:** نعمل (أ) دلوقتي. نفتح باب (ب) بخطوات صغيرة بتفيد النسخة العادية كمان، ونعمل **تجربة متصفح صغيرة للرسم بس، من غير وعود**. لو إحساس القلم عجبك فيها، نكتب قرار معماري جديد (ADR). و(ج) **بعد إصدار v1 بس**.
6. **السبب:** اللي هيميّز Varos هو **الكتابة العربي**، مش الشغل الجماعي. والسحابة هتاكل سنة ونص من وقتك قبل ما نوصل للعربي.
7. **تنبيه:** أي نسخة متصفح بتخالف ADR-0001 (اللي بيقول البرنامج native بس) — لازم قرار جديد منك قبل ما تدخل الكود الأساسي.

---

## 2. The three shapes at a glance

| Shape | What the user gets | Rough effort (designer + AI team) | Running cost | Breaks which accepted law? |
|---|---|---|---|---|
| **A. Native Mac + Windows** | Download an app for Mac or Windows; files stay on the computer (`.vrs`) | **1–2 months** in total, including the app-shell port already under way | Apple Developer Program, about 99 USD/year **(unverified current price)** | None. ADR-0001 is about the UI stack, not the OS |
| **B. Same code in the browser (WASM + WebGPU)** | Open a URL, draw on the canvas, download/upload `.vrs`; no accounts, no sharing | Spike **0.5–1 month**; a usable browser editor **+3–6 months**; then about **20–30% extra** on every later feature, because each one has to be tested in the browser too | Static hosting, close to free **(unverified: free tiers of static hosts)** | **ADR-0001** (a parallel web shell); how updates work in practice under **ADR-0007**; possibly the CLAUDE.md "never eframe" rule |
| **C. Figma-style cloud** | Accounts, files in the cloud, share links, live multiplayer, comments | **12–24+ months** to reach a first trustworthy multiplayer. Needs at least one experienced backend/security human | From tens of USD/month (small beta) to hundreds or thousands at real scale **(unverified, order of magnitude only)**, plus someone who fixes it when it breaks | ADR-0001, **0002, 0003, 0004, 0005, 0007**, and charter §6 "no new crates" |

A cheaper kind of "online" already exists: a `.vrs` file **is a valid PDF** (ADR-0003:15-17), so anyone can open a shared `.vrs` in any PDF viewer, in a browser, or in Google Drive or Dropbox today. People can *see* work online without any new code.

---

## 3. Mac version — what remains after the app-shell port

Facts known today (reported by the lead session, not re-run by this study): `varos-core`, `varos-pdf`, and `varos-render-wgpu` build and pass their 220 tests on macOS (Metal). Only `varos-app` is Windows-bound, and another agent is porting it now. The list below is what is **left after** that port lands. Effort is working days of gated sessions.

| # | Item | Evidence in code | Effort |
|---|---|---|---|
| 1 | **Retina / mixed displays.** The UI scale is read once at startup and never updated. There is no `ScaleFactorChanged` handling anywhere in `varos-app`. On a MacBook with an external non-Retina monitor, dragging the window between screens will show the wrong UI size. (This is the same risk as the parked Windows "DPI-change" item, FOUNDATION_CHARTER.md:115.) | `varos/crates/varos-app/src/main.rs:566` (`let scale = window.scale_factor()`), used at `:644`, `:960`; grep for `ScaleFactorChanged` finds nothing | 1–3 days |
| 2 | **UI fonts.** Fonts load from `C:/Windows/Fonts` (Segoe UI / Cascadia). On a Mac this finds nothing, so egui's built-in font is used silently. The fix is to bundle an open-licence UI font for both OSes. That changes UI_DIRECTION's font choice, so it needs Ahmed's design sign-off. Apple's SF fonts are generally not redistributable **(unverified licence detail)** | `varos/crates/varos-app/src/ui.rs:1389-1401` | 1–2 days + design decision |
| 3 | **Settings and crash-log folder.** Paths use `%APPDATA%`. On a Mac they return `None`, so window position and crash logs are silently not saved. Mac should use `~/Library/Application Support/Varos` | `main.rs:360-366`, `:451-458` | ½ day |
| 4 | **Debug files written to a relative `target/` folder.** Inside a Mac `.app` the working folder is not the project, so these writes fail silently. They should be dev-only | `main.rs:466`, `:586-592`, `:1026-1029` | ½ day |
| 5 | **Window chrome.** The app draws its own title bar and min/max/close buttons, using DWM tricks (cloak, custom frame, transparent splash). Mac users expect the red/yellow/green "traffic-light" buttons. winit exposes a transparent title bar / full-size content view on macOS **(unverified exact API names)** | `main.rs:549-558`, `:993-1008`, `:1058-1076` | 2–5 days + Ahmed visual decision |
| 6 | **Global menu bar.** A Mac app without an app menu (About, Quit ⌘Q, and an Edit menu so copy/paste works in text fields) feels broken. A menu crate such as `muda` is an option **(unverified)**. Under the ONE-HOME rule, the menu is a mirror only | no native menu exists today | 3–5 days |
| 7 | **Shortcuts and trackpad.** Cmd already counts as Ctrl (`main.rs:739`, good). UI labels still say "Ctrl". Trackpad two-finger scroll already pans (`main.rs:822`). **Pinch-to-zoom is not handled.** Zoom is Alt+wheel only (`main.rs:824-833`), and Mac designers expect pinch **(unverified: winit's pinch event name on 0.30)** | `main.rs:739`, `:813-841` | 1–3 days |
| 8 | **Double-click a `.vrs` in Finder.** This needs an `Info.plist` document-type entry, plus handling the macOS "open file" Apple Event. winit 0.30 does not surface that event, so a small hook into the app delegate is needed **(unverified)**. The Windows single-instance code (`single_instance.rs`) is mostly unnecessary on Mac, because macOS already keeps one copy of an app running **(unverified behaviour detail)** | `varos/crates/varos-app/src/single_instance.rs:39-302` | 2–4 days |
| 9 | **Cursors.** Illustrator-style cursors are read at **runtime** from a build-machine path. That folder is git-ignored proprietary reference material (`.gitignore:21-22`), so it must **never** ship. Shipped builds use the built-in cursors. On Mac they must go through winit's custom cursor instead of Win32 `HCURSOR` (part of the port) | `varos/crates/varos-app/src/cursors.rs:273-276` | in the port |
| 10 | **Eyedropper outside the window.** Windows uses `GetPixel` on the screen. Mac needs the system colour sampler (`NSColorSampler`, **unverified**) or a screen-recording permission prompt | `cursors.rs:372-395`; non-Windows stub returns `None` at `:397-400` | 1–2 days |
| 11 | **App icon.** `.ico` has to be converted to `.icns` | `varos/varos.ico` | ½ day |
| 12 | **Bundle + universal binary + DMG.** Build for Apple Silicon and Intel, merge the two, and wrap in a `.app` and `.dmg`. Tools such as `cargo-bundle` / `cargo-packager` exist **(unverified)** | — | 1–2 days |
| 13 | **Signing + notarization.** This needs a paid Apple Developer account, a Developer ID certificate, hardened runtime, notarization, and stapling. Without it, macOS blocks the app on double-click and users must override it in System Settings **(unverified: exact current Gatekeeper flow)** | — | 2–4 days the first time, plus the yearly fee |
| 14 | **Updates.** ADR-0007 already covers this (visible, user-controlled). Nothing new is needed until distribution work starts | `docs/adr/ADR-0007-visible-update-policy.md:15` | — |
| 15 | **Process cost.** Every gate now needs a hand-test on Windows **and** Mac. Hosted CI is blocked anyway (STATUS "External action items") | — | about +20% per gate, permanently |

**Total after the port: about 3–6 weeks.** Nothing here touches an ADR. CLAUDE.md's "Windows-first" stays true; Mac becomes a second tier-1 target.

---

## 4. Browser build (WASM + WebGPU)

### 4.1 What was checked, and how

- The only installed Rust target is `aarch64-apple-darwin` (`rustup target list --installed`). **No crate was compiled for `wasm32-unknown-unknown`.** Installing that target needs a download, which this study did not do.
- `cargo tree --offline --target wasm32-unknown-unknown -p varos-core` **resolves** (4 deps: `flo_curves`, `i_overlay`, `serde`, `serde_json`).
- The same command for `varos-pdf`, `varos-render-wgpu`, and `varos-app` fails with `failed to download bumpalo v3.20.3`. That shows the web-only dependency set has **never been fetched on this machine**: no one has tried a web build yet.
- Everything else below comes from reading our code, `Cargo.lock`, and the cached sources of `wgpu 29.0.4`, `wgpu-types 29.0.4`, `egui-wgpu 0.35.0`, and `lopdf 0.43.0` in `~/.cargo/registry`.

### 4.2 Crate-by-crate verdict

| Crate / dependency | Web verdict | Evidence | What must change |
|---|---|---|---|
| **`varos-core`** | **Likely compiles as-is** (not compiled) | Deps are pure Rust (`varos/crates/varos-core/Cargo.toml:11-17`). No `Instant`, threads, or time APIs in `src/` (grep). `std::fs` appears only in `file.rs:52,53,60`. That compiles on wasm but fails at runtime, and the web path would use the bytes-based `doc_to_blob` / `doc_from_blob` (`file.rs:22,27`) | Nothing, for a demo |
| **`varos-pdf`** | **Probably compiles, with feature changes** | `lopdf = "0.43"` uses default features (`varos/crates/varos-pdf/Cargo.toml:13`), which pull in `rayon`, `chrono`, `jiff`, `time`, `getrandom 0.4`, and `rand` (`Cargo.lock`, lopdf entry). getrandom on `wasm32-unknown-unknown` needs its JavaScript backend, and lopdf ships a `wasm_js` feature for exactly this (`lopdf-0.43.0/Cargo.toml` `[features] wasm_js = ["getrandom/wasm_js"]`). rayon is expected to fall back to one thread on wasm **(unverified)** | Turn on `wasm_js` or trim lopdf's default features. Add a public "load from bytes" function: `load_vrs` reads a path (`varos-pdf/src/lib.rs:36-37`), but `write_pdf` already returns bytes (`:72`) and `extract_model(&[u8])` exists privately (`:343`). About a 1-hour change |
| **`varos-render-wgpu`** | **Compiles in principle; will crash at runtime until timing is fixed** | wgpu's default features include `webgpu` (`wgpu-29.0.4/Cargo.toml` `[features] default`). egui-wgpu's defaults also turn on `wgpu/webgl` (`egui-wgpu-0.35.0/Cargo.toml` `[features] default`), so both web back-ends are already in our build graph. `Backends::PRIMARY` (`varos-render-wgpu/src/lib.rs:210`) already includes `BROWSER_WEBGPU` (`wgpu-types-29.0.4/src/backend.rs:127-130`). `Renderer::new` is already `async` (`lib.rs:208`), which is good for the web. **Blocker:** `std::time::Instant::now()` at `lib.rs:1009`, `:1022` and `perf.rs:8,23` panics on `wasm32-unknown-unknown` **(well-known Rust behaviour, unverified here)** | Swap `std::time::Instant` for `web-time` (already in `Cargo.lock` via winit / egui-winit / egui-wgpu). The MSAA picker (`lib.rs:238-249`) already falls back to 4× or 1× (WebGPU allows only 1 and 4). The present-mode picker (`:257-263`) falls back to Fifo. The depth format `Depth24PlusStencil8` (`:15`) is core WebGPU. A WebGL2 fallback would also need `Backends::GL` added at `:210` **(untested)** |
| **`egui 0.35` / `egui-wgpu 0.35`** | **OK** | egui-wgpu already depends on `web-time` (`Cargo.lock`) | — |
| **`egui_tiles` vendored fork** | **Should be OK** | Deps: egui (no default features), itertools, log, serde, ahash `no-rng` (`varos/vendor/egui_tiles/Cargo.toml:73-100`). No `Instant`, threads, or `fs` in `vendor/egui_tiles/src` (grep). The five patched files (ADR-0006) are pure egui | — |
| **`winit 0.30.13`** | **Web back-end exists; our startup pattern doesn't fit it** | winit's lock entry includes `web-sys`, `wasm-bindgen`, `web-time`. But we use the deprecated closure-style `EventLoop::run` (`main.rs:2`, `:657`), and we **block** on GPU start with `pollster::block_on` (`main.rs:559`). A browser tab cannot block its main thread | Restructure `main.rs` into a winit `ApplicationHandler` app object that creates the renderer asynchronously. This helps native too, since it removes the deprecated API. About 1–2 weeks, because `main.rs` is one 630-line event closure (`:657-1087`) |
| **`egui-winit 0.35`** | **Biggest unknown** | Its lock entry pulls `arboard`, `smithay-clipboard`, `webbrowser`. Whether egui-winit compiles and behaves correctly inside a browser is **unverified**. egui's paved web road is eframe's own web runner, which does **not** use egui-winit **(unverified)**. CLAUDE.md:5 says "never eframe" | Three options: (a) prove egui-winit works on the web, (b) write our own small browser-input glue (about 1–3 weeks, **estimate**), or (c) amend "never eframe" for the web host only |
| **`rfd 0.15`** (dialogs) | **Must change** | Lock entry has a web path (`js-sys`, `web-sys`, `wasm-bindgen-futures`). But on the web rfd offers only its *async* API, and we use the *blocking* `FileDialog` (`main.rs:872-876`, `:902-904`) and `MessageDialog` (`:383-387`, `:393-398`, `:430-434`, `:473-477`, `:890-894`) **(unverified: rfd web API surface)**. On the web, "Save" usually means "download a file" | A web branch for open/save/alerts. Alerts could be drawn in egui (which also fits the visual constitution better) |
| **`windows 0.62`** | **Must be gated** | Declared for every platform (`varos-app/Cargo.toml:20`). Code uses are `#[cfg(windows)]` in `cursors.rs` / `single_instance.rs`, but `main.rs` calls `cursors::install`, `set_cloaked`, `custom_frame`, `is_maximized` unconditionally (`main.rs:556-557`, `:571-572`, `:687`, `:1060`). The Mac port is adding the non-Windows paths | After the port: move `windows` under `[target.'cfg(windows)'.dependencies]` |
| **`image` (png) / `resvg 0.45`** | **OK / probably OK** | `image` is pure Rust (window icon only, `main.rs:271-275`). resvg is used only for cursor SVGs (`cursors.rs:7`, `:209-216`), but its defaults pull in `fontdb` with `memmap2` and `fontconfig-parser` (`Cargo.lock`) | Possibly `default-features = false` for resvg on the web **(untested)** |
| **`pollster`** | **Must go on the web** | `main.rs:559` | Part of the `ApplicationHandler` change |
| **Threads** | **Good news: none in shared code** | Only `single_instance.rs:193-201` (Windows-only) sleeps on a thread | — |
| **Other `Instant` uses** | **Must change** | `main.rs:12`, `:772`; `ui.rs:10`, `:995`, `:1073`, `:1152` | `web-time` |
| **File-system uses** | **Must be no-ops on the web** | Fonts `ui.rs:1392`; window state / crash log `main.rs:360-458`; debug files `main.rs:466`, `:586`, `:1026`; cursor SVGs `cursors.rs:275-276` | Fonts must be **embedded** in the download (a bigger first load) |
| **`getrandom 0.3`** (via `ahash`, `Cargo.lock`) | **Config only** | Needs the JS backend switch on wasm **(unverified exact flag for 0.3)** | Build config |
| **Panic path** | **Must change** | The panic hook shows a native dialog (`main.rs:464-478`) | Console + in-canvas message |

**Bottom line for a canvas-only demo:** the core, PDF, render, and dock crates are close. The real work is in `varos-app`: a new startup shape, dialogs, fonts, timing, the input glue (the egui-winit unknown), and gating Win32 away. None of it is an impossible wall. The unknown that can kill the plan is **feel and speed**, and only a spike answers that.

### 4.3 Browser WebGPU support (all **unverified** — from memory, not checked online)

| Browser | WebGPU status as best known |
|---|---|
| Chrome / Edge desktop (Windows, macOS, ChromeOS) | On by default since version 113 (2023) |
| Chrome Android | On since about version 121 (2024) |
| Chrome on Linux | Partial / rolling out; may need a flag on some GPUs |
| Safari | On by default from **Safari 26** (macOS / iOS / iPadOS 26, autumn 2025); older Safari has none |
| Firefox | Shipped on Windows around version 141 (mid-2025); macOS Apple Silicon later; Linux and Android still partial |
| Any browser | Can be disabled by GPU block-lists or company IT policy |

A WebGL2 fallback is technically in our build graph (see §4.2), but it is a second GPU path to test, and our feel targets were measured on native GPU APIs.

### 4.4 What "online" means for files, fonts, and Arabic

- **Files:** the browser build can keep `.vrs` exactly as it is (ADR-0003 needs no change for shape B). The flow becomes "Open" = pick a file, "Save" = download a file. Browsers can also keep files in their own private storage (IndexedDB / OPFS), but that storage **can be cleared by the browser or the user** **(unverified: eviction rules differ per browser)**. The honest default is "download your `.vrs`". A real save-to-the-same-file button exists only in Chromium browsers (File System Access API, **unverified** for Safari/Firefox). Cloud storage is shape C, not B.
- **Fonts:** a web page **cannot freely read the fonts installed on the user's computer**. There is a Chromium-only Local Font Access API with a permission prompt **(unverified status)**. For a typography-first tool this matters a lot: in the browser, designers would get only the fonts we ship or that they upload. Every shipped font adds to the download.
- **Arabic shaping (later):** `rustybuzz` (a pure-Rust HarfBuzz port) is **already in our dependency tree** through `usvg` (`Cargo.lock`, usvg entry). Pure-Rust shaping should compile to wasm **(unverified)**, so the Arabic engine itself is not a web blocker. The blockers are font access (above) and typing Arabic through the browser's input-method layer into a GPU canvas **(unverified how well egui/winit handle web IME)**.
- **Keyboard:** browsers keep some shortcuts for themselves (Ctrl/Cmd+W, T, N; Ctrl+wheel = page zoom) **(unverified per-browser list)**. Our zoom is Alt+wheel (`main.rs:824`). Right-click and Space need their default page behaviour suppressed.
- **Eyedropper outside the window:** impossible in a browser. A Chromium-only EyeDropper API exists **(unverified)**.
- **Updates:** a web page loads the newest code on every visit. That is a *silent update by nature*, and it collides with ADR-0007:15 ("Varos will not silently install background updates"). A web build needs either version-pinned addresses (e.g. `/v0.4/`) plus a visible "new version — reload?" notice, or an ADR amendment.

---

## 5. Figma-style cloud — what it truly requires

### 5.1 The parts

1. **Accounts and sign-in:** email plus Google/Apple login, password reset, 2FA, account deletion. Either built or bought from an auth provider (monthly fee).
2. **A live server:** a WebSocket "room" per open document that orders everyone's changes, shows other people's cursors (presence), and survives disconnects and reconnects (offline edits that come back later).
3. **A collaborative document model:** the hard part, see §5.2. Either a CRDT library (Rust options include Automerge, Yrs, Loro, **unverified maturity for our needs**) or a server-authoritative "last writer wins per property" design. Figma's public engineering blog describes the second **(unverified detail)**.
4. **Storage:** document snapshots plus change logs, images and fonts in object storage, backups, version history, restore.
5. **Sharing and permissions:** owner/editor/viewer, links, teams, comments.
6. **Security and law:** TLS, rate limits, abuse (people upload illegal content), privacy law, data-loss responsibility. The charter's parked **hostile-file hardening** (FOUNDATION_CHARTER.md:113) becomes P0 the moment strangers' files reach our server.
7. **Operations:** monitoring, alerts, and a human who responds when it breaks at 3 a.m. AI sessions are not on-call.
8. **Money:** Varos is free and open source. Someone has to pay the monthly bill (sponsors, donations, a paid tier). That is a product and identity decision, not a technical one.

### 5.2 Does the `EditCommand` boundary help?

**It helps, but it is not a sync protocol, and ADR-0002 itself says so** (`docs/adr/ADR-0002-two-level-command-boundary.md:24`, `:32`).

What helps:
- Every discrete edit already flows through one funnel. STATUS records "zero direct document writes from the UI (measured)" after F4.1, which gives one place to detect a change and emit a change record.
- `varos-core` is headless (ADR-0005:17-33). The **same core could run on a Rust server** to validate and merge documents. That is a real advantage most apps don't have.
- Commands are described as deterministic (`varos/crates/varos-core/src/command.rs:11`).

What does not help, with evidence:
- **Commands are "intents", not document changes.** Many act on the *local selection*: `GroupSelection`, `DeleteSelected`, `Nudge`, `TransformAgain` (`command.rs:51-52`, `:77-82`, applied via `ed.objsel`, `editor.rs:328`). Replayed on a teammate's machine, they would act on *their* selection.
- **Artboards are addressed by list position** (`index: usize`, `command.rs:83-106`). Positions shift when someone else adds or deletes a board at the same moment.
- **Drawing gestures bypass `EditCommand`.** Pen and drag go through `pointer_down/move/up` directly (`main.rs:731`, `:785`, `:798`) and commit as whole-document snapshots (`editor.rs:2870-2887`).
- **Undo stores whole-document copies** (`editor.rs:368-370`, `:2889-2903`). Multiplayer undo must undo *only my* changes, which needs per-change inverses.
- **Object IDs come from a per-document counter** (`model.rs:516` `pub ids: u32`, `nid()` at `model.rs:588-591`). Two people drawing at the same time would both create "object 42". IDs must become globally unique (client ID + counter).
- **Drawing order is a plain list re-flattened by `sync_tree`** (`model.rs:500-512`). Collaborative ordering needs stable positions (a sequence CRDT or fractional indexing).
- `EditCommand` is not serializable (`command.rs:12`, no `derive`). Commands are not versioned (ADR-0002:24).

**Conclusion:** cloud collaboration means a **redesign of the document model in `varos-core`**: stable unique IDs, property-level changes, and ordering and undo that work across users. Every tool (pen, boolean, masks, artboards) must be re-verified on top of it. That is why this is a different product phase, not a feature.

### 5.3 Hosting reality

- The existing **2-vCPU / 8-GB VPS** could host a *small private beta* relay: tens of people editing at once is plausible for a light relay **(unverified estimate, depends on document size)**. It is still **one machine**: if it dies, every user's work is unreachable. There is no redundancy, and it already runs other tools.
- A real service is at minimum a managed database, object storage, two or more app servers, backups, and a CDN. Expect roughly **tens to hundreds of USD per month** at hundreds of active users, and **much more** at scale **(unverified, order of magnitude only)**.

### 5.4 Which accepted decisions shape C would reopen

| Law | Why |
|---|---|
| ADR-0001 | Needs the browser host (shape B) first |
| ADR-0002 (`:24`, `:32`) | Needs a versioned, serializable change protocol, which the ADR explicitly deferred |
| ADR-0003 (`:15-19`) | The document of record moves to the server; `.vrs` becomes import/export. The ADR says it is "not an eternal container promise" (`:19`), so a superseding ADR is the expected path |
| ADR-0004 (`:21`) | Stable property IDs + live migrations between clients on different versions, which the ADR names as future work needing its own ADR |
| ADR-0005 (`:17`, `:33`) + charter §6 (FOUNDATION_CHARTER.md:80) | A server crate (and probably a web crate) is a fifth or sixth crate, which requires a superseding ADR |
| ADR-0007 (`:15`) | Web clients update on reload |

---

## 6. Staged path that keeps every accepted law intact

**Stage 0 — Mac native (now).** This finishes the app-shell port, then the §3 list.
- Also do a few **"web-ready hygiene"** fixes that *also improve native*, each as its own small gated work order:
  - `web-time` instead of `std::time::Instant`
  - a bytes-based `.vrs` load in `varos-pdf`
  - `main.rs` moved to winit's `ApplicationHandler` (removes the deprecated API)
  - embedded UI fonts (also fixes Mac)
  - `windows` moved under `cfg(windows)`
- Optionally add a cheap guard, `cargo check -p varos-core --target wasm32-unknown-unknown`, so the core stays web-clean forever. It needs a one-time target install.
- **No ADR needed.** No user-visible change on Windows (charter §4 "behavior-preserving").

**Stage 1 — Experimental canvas-only browser spike, no promises.**
- **Scope:** canvas + pen + select + zoom/pan; one built-in sample document; "download `.vrs`". Panels optional.
- **Where:** a **separate branch**, behind `cfg(target_arch = "wasm32")` / a `web-spike` feature *inside `varos-app`*. There is **no new crate**, so ADR-0005 holds. It is **not merged to `main`**, so ADR-0001:23's "no web shell maintained in parallel" holds.
- **Access:** a private link for Ahmed only.
- **Time-box:** 2–4 weeks, then stop.
- **Exit evidence** (numbers, not opinions):
  1. Ahmed's hand-test of pen feel: yes/no, in Chrome and Safari 26 on his Mac.
  2. Frame times on the P11 benchmark scenes (`docs/foundation/P11_1_PERF.md`), web vs native.
  3. Download size in MB.
  4. Firefox result recorded.
  5. Answer to the egui-winit question from §4.2.
- **Kill it** if the feel fails.

**Stage 2 — Only if Stage 1 passes: a new ADR (proposed ADR-0008 "Browser host as a tier-2 target"),** accepted by Ahmed before any web code reaches `main`. What it would have to say about ADR-0001:
- **KEEP:** egui painted on the GPU through wgpu; **no DOM UI, no web views, no Tauri/Electron**; GPU required, with a readable failure (in a browser: a clear "your browser has no WebGPU" page) — ADR-0001:19.
- **AMEND:** "Varos V1 uses the native Winit + Egui + WGPU stack" (ADR-0001:17) becomes "Winit + Egui + WGPU, hosted natively (tier 1) or in a browser canvas (tier 2)". "Production UI work targets the native stack" (`:17`) becomes "targets the shared stack; native stays tier 1".
- **REJECT (supersede):** "no web shell is maintained in parallel" (ADR-0001:23). A browser host *is* a parallel shell.
- **Also decide in the same ADR:**
  - how web updates satisfy ADR-0007 (version-pinned URLs + visible reload notice, or an amendment)
  - whether CLAUDE.md's "never eframe" gets a web-only exception
  - the support tier per browser
- **Then** schedule the real browser work (3–6 months). Proposed timing: after the current foundation program (F8), with the product order Ahmed ratifies then (FOUNDATION_CHARTER.md:116).

**Stage 3 — Cloud, only after v1 ships.** Climb the cheapest steps first, each with its own ADR:
- (a) sharing `.vrs` as a PDF (already works);
- (b) a **read-only web viewer** link: render only, far cheaper than editing;
- (c) single-user cloud save/sync;
- (d) comments;
- (e) live multiplayer last.

Steps (c)–(e) need the model redesign in §5.2 and at least one experienced backend/security human.

---

## 7. Risks and honest unknowns

1. **egui-winit in the browser** is unproven for our manual (non-eframe) setup. This is the single most likely spike killer, or the thing that forces an eframe exception.
2. **Feel and speed.** Our tessellation runs on the CPU on one thread (`varos/crates/varos-render-wgpu/src/tess.rs`). Wasm is usually somewhat slower than native for this kind of work **(unverified, commonly 1.2–2×)**. Wasm threads need special server headers (COOP/COEP) **(unverified)**. P11 perf work helps both.
3. **Safari/Firefox WebGPU maturity**, GPU block-lists, and company policies: some users simply won't get a GPU in the browser.
4. **Fonts in the browser** undermine the Arabic-typography story (no access to installed fonts, download size), and web IME for Arabic typing is unknown.
5. **Browser storage can be wiped.** A browser-only user can lose work unless they download `.vrs` files.
6. **Maintenance tax.** Windows + Mac + browsers means every gate gets more expensive for a solo designer who hand-tests everything.
7. **Cloud is a 24/7 commitment:** security, abuse, law, bills, on-call. None of that is solved by AI sessions.
8. **Apple costs and policy** (yearly fee, notarization rules) can change **(unverified current terms)**.
9. **Estimates are ±50%.** Nothing was compiled for the web. The first 10 minutes of Stage 1 (install the target, run `cargo check` per crate) will replace half of §4.2's "probably" with facts.
10. **Everything marked (unverified)** comes from memory without network access. It must be re-checked online before any decision is made on it.

---

## 8. Decision asked of Ahmed

- [ ] Approve **Stage 0** (Mac native + web-ready hygiene, no ADR change).
- [ ] Approve or decline a time-boxed **Stage 1 spike** (branch-only, canvas-only, no promises), and when.
- [ ] Confirm **cloud (shape C) is parked until after v1**.

Sign-off: study author (AI session) — DRAFT — 2026-09-23
