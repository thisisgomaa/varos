> **Status:** reference — independent plan review, 2026-09-24.
# Review — `DFS_S4_S6_ASSOCIATION_EXPORT.md` (S4 association + S6 export)

Reviewer: independent plan reviewer (read-only; no cargo run). Baseline: branch `claude/sweet-cerf-1sg30t`.

## Verdict: APPROVE WITH CHANGES
1. The core designs are right: one `os_open` queue fed by CLI/Windows/macOS; a pure `write_pages(.., model: Option<_>)` split; one Export home reached by `ShowExport`.
2. The macOS bridge is sound. winit 0.30.13 has no open-documents API, and adding a method to its delegate class is the only option that does not panic. But the fallback in the plan is a no-op and one refresh step is missing (F3).
3. S6-A (in flight) should **emit the PDF clip that `docs/MASKS_PLAN.md` Stage 5 already designed**, not build a refusal detector that will be thrown away (F1, P1).
4. Seven pieces can be five. S4-C duplicates S3's fingerprint guard. S6-D is one test, not a piece. `file_routes.rs` is a third route table next to S1's `lifecycle_key` and `chrome::menus()` (F4–F6).
5. The linear merge order chains S6 behind S5, which is gated on ADR-0008. Use a dependency graph instead (F9).

## Findings

**Affects S6-A, which is being built now. Send these to its agent first.**

1. **P1 — Masks: clip instead of refusing (changes S6-A's scope).**
   - *Evidence:*
     - `docs/MASKS_PLAN.md` (status *current*) §5 row "Export (PDF)" (`:244`) and "Stage 5 — PDF export clip" (`:298–304`) already specify `q … W* n … Q` around each clipped member, with the mask outline via `emit_ring`, plus a test.
     - The canvas supports only single-level clips (`varos-core/src/scene.rs:622`), and it uses even-odd over all mask rings (`scene.rs:48–58`). That is exactly `W*`.
     - The accessors already exist: `Document::clip_group_of`, `node_mask_child`, `node_paths` and `unit_xform` (`model.rs:657–672, 1081, 1687`).
     - The clip is about 20 lines inside the loop at `varos-pdf/src/lib.rs:122–207`. The refusal needs the same ancestor walk *plus* an error variant, UI copy, and a per-frame plan walk (F10). It also blocks Export for **every** document that contains one mask, and Astra's files use masks (STATUS batch-2 item 6).
     - A shared-loop clip also fixes the `.vrs` preview pages (the WO's own R2 note) at no extra cost.
   - *Change:*
     - Replace §3.3 "**Masks (the spec §6 fidelity rule):** if any path emitted … `ClipMasksNotSupported` … see §6 R2." with: "**Masks:** implement MASKS_PLAN Stage 5 in `write_pages`. For a painted path with `clip_group_of = Some(c)`: `q`, the rings of every path in `node_paths(node_mask_child(c))` (each with its own `unit_xform`), `W* n`, the member's usual paint (knockout XObject included), then `Q`. Cull a member whose bbox misses the mask bbox. Single-level only, as on the canvas."
     - Delete `ClipMasksNotSupported` from the enum. Replace step 4 of S6-A and the tests `clip_mask_member_makes_scope_unavailable` / `clip_mask_off_page_does_not_block` with `clip_member_is_wrapped_in_w_star_n` and `member_outside_mask_is_not_emitted`.
     - Step 1 captures the byte-identity fixture on the demo document **without** a mask. The masked native fixture is regenerated in the clip commit, and the PR says so.
     - R2 becomes: "Fallback only if the clip cannot pass inside the day: refuse, as originally planned."
   - Owner confirmation is listed below under "Needs Ahmed".
2. **P2 (S6-A, send now) — `has_embedded_model` must not run a full lopdf parse on an arbitrary file.**
   - *Evidence:* S6-B calls it on whatever file the user picks as the destination, up to 256 MiB (§3.3 guards). A lopdf load is unbounded, which is the exact risk S5-D exists to remove (spec §2 "A parser that allocates unbounded…"; S5 WO §2 `:59–67`).
   - *Change:* keep the signature. Add to its doc line: "Bounded byte scan for `/VAROS_Model` or `model.varos.json`; no PDF parse. It feeds a warning only, so a false positive just asks one extra question."
3. **P2 (S6-A, send now) — use the canvas bounds for Artwork bounds.**
   - *Evidence:* `world_bbox` (`lib.rs:320–332`) is the hull of the control points, handles included, so the pages of curved art come out oversized. `Document::outline_bbox(pi)` (`model.rs:868`) is the flattened, xform-aware extent that the canvas already uses.
   - *Change:* "the union of the visible paint-list world bboxes, padded by half the stroke width" → "the union of `doc.outline_bbox(pi)` over visible painted paths, padded by half the stroke width".

**macOS bridge (S4-B)**

4. **P2 — the isa-swizzle fallback cannot fix anything, and the plan is missing a delegate refresh.**
   - *Evidence:*
     - Delegate replacement is fatal, not just stale docs: `ApplicationDelegate::get` panics unless the delegate `is_kind_of` winit's class (`winit-0.30.13/src/platform_impl/macos/app_state.rs:174–184`). It is called on **every run-loop wake** (`observer.rs:62,84`) and in `app.rs:45`.
     - A subclass swizzle passes that check, but it resolves the selector exactly as `class_addMethod` does. So if an added method is never called, the swizzle is not called either.
     - Adding a method to a live class is supported by the ObjC runtime (categories work this way), and `objc2 0.6.4` exposes `ffi::class_addMethod` (`src/ffi/class.rs:78`). winit's class comes from objc2 0.5. That is harmless, because there is one libobjc and the IMP receives `*mut AnyObject`.
     - The real remaining risk is AppKit caching "delegate responds to X" at `setDelegate:` time. winit calls `setDelegate:` inside `EventLoop::new`, *before* `install()` runs (`event_loop.rs:232–240`).
   - *Change:*
     - Replace "Fallback if the spike fails … isa-swizzle … same queue." (§3.2, S4-B step 4, R1) with: "After adding the method, re-assign the same object with `app.setDelegate(app.delegate())`. This is sound: winit keeps the strong reference in `EventLoop.delegate`. It makes AppKit re-read any responds-to cache. The IMP is `extern "C-unwind"`, never panics, and keeps the proxy in a static `Mutex`, as `mac_menu.rs:66` does. If the IMP still never fires on the bundled app, S4's gate stays open (R1). There is no swizzle."
     - Winit's own docs (`platform/macos.rs:29–30`) are stale. Add a comment that the pattern shown there would panic on the first wake.
5. **P3 — the encoding test proves nothing.**
   - `bridge_selector_signature_is_v_at_colon_at_at` compares a constant with itself.
   - *Change:* factor the logic as `add_open_urls(cls) -> Added|AlreadyPresent` and test it on a throwaway `NSObject` subclass. This needs no NSApp, no window and no EventLoop. The test checks: the method is added; a second call returns `AlreadyPresent`; `msg_send` with `[file://…, https://…]` enqueues exactly one path.

**Piece count and duplication**

6. **P2 — delete S4-C, the process lock.**
   - *Evidence:*
     - The spec asks to "protect duplicate-process writes" (§2 Association), which it satisfies with "fingerprint/ownership protection" (§6).
     - S3 already builds the fingerprint check (S2_S3 WO §3.3 `fingerprint`, F1 test `external_change_prompts_before_overwrite`) and per-process owner locks (`Recovery/owners/<owner>.lock`, §3.5).
     - On macOS, a second process only happens when someone execs the binary directly. `open`, Finder and the Dock all reuse the running app.
   - *Change:* delete §3.2 "Cross-process write protection…" and the S4-C piece. Add to §1 S4 *Closes*: "Duplicate-process write safety = S3's fingerprint prompt (`external_change_prompts_before_overwrite`)." Update R4 to match.
7. **P2 — `file_routes.rs` is a third table.**
   - *Evidence:*
     - S1-D already adds `LifecycleKey` + `lifecycle_key()` as the single key-to-`AppCommand` mapper, and the test `every_file_menu_row_reaches_its_lifecycle_command` walks `chrome::menus()` (S1 WO S1-D steps 3–4).
     - S6-C's `FileCmd` + `to_app_command` + `FileRoute{native_menu_id, hamburger, chord}` restate `chrome::menus()` (ids and accelerators) and the burger rows (`ui.rs:3415–3421`), and only a test keeps them in sync.
     - This is not strictly a ONE-HOME breach, since routes are mirrors. It is duplicated mapping code.
   - *Change:* replace the S6-C "F4.2 file-effect parity" block with:
     - "Extend S1's `LifecycleKey` with `Export`, which stays the only enum and the only mapper.
     - Native file rows become `MenuCmd::Lifecycle(LifecycleKey)` instead of `Key(..)`. That is the spec §4 rule 'command IDs, not synthetic key events'.
     - The burger draws from a `const BURGER_FILE_ROWS: &[(&str, LifecycleKey)]`.
     - The parity test reads those real sources (`chrome::menus()`, `BURGER_FILE_ROWS`, `lifecycle_key`). There is no `file_routes.rs`, `FileRoute` or `TopbarCtl`."
8. **P2 — fold S6-D into S6-C.**
   - *Evidence:* the spec's release gate is Ahmed's manual journey on the Mac (§5 "System release gate"; "UI evidence is manual"). The headless journey adds useful cross-piece coverage, but it is one test. The hand-test list is moderator work under the CLAUDE.md process law.
   - *Change:* delete the S6-D piece. Add `system_journey_headless` to S6-C's tests, with "S6-C merges last". The simulated crash must also drop S3's `RecoveryStore` so that the owner lock is released before the rescan.
9. **P2 — merge order: use a dependency graph, not a line.**
   - *Evidence:* S5's own order says that "nothing from B–E reaches `main` while ADR-0008 is `Proposed`" (S5 WO §5 step 5). S6-B/C need nothing from S5 (after F2). Hot spots are also missing: S2-E owns `chrome.rs`/`mac_menu.rs` (native Open Recent; S2_S3 WO §5), and S3-F1 owns the `AboutToWait` completion drain in `main.rs`.
   - *Change:* replace the §5 "Order" list with:
     - S6-A: any time.
     - S4-A: after S1-D. S4-B: after S4-A.
     - S6-B: after S1 + S3-F1 + S6-A. S6-C: after S1-C + S2-E + S6-B.
     - S5 is independent. The only ordering is the `lib.rs` rebase between S5-D and S6-A.
     - Add the S2-E and S3-F1 hot spots.
10. **P2 — S6-B: reuse S3's writer and worker, and drop the fallback.**
    - *Evidence:* S3 builds `write_replace` (unique temp, `sync_all`, rename, directory sync, typed `reason()`) and a FIFO `IoWorker`, whose single thread already serializes writes per destination (S2_S3 WO §3.3, §3.6). The "Fallback if S3 has not landed" in §3.1 is a second durable writer, and S6-B already waits for S3.
    - *Change:*
      - Delete that fallback sentence.
      - Change `export_job.rs` → "add `Job::Export { sid, dest, doc, plan, cancel: Arc<AtomicBool> }` to S3's `IoWorker`; the write uses `write_replace`".
      - Compute the plan only when `(session, rev, scope)` changes, not on every frame of the panel.
      - Keep the per-session scope in a `HashMap<SessionId, ExportScope>` inside `export_home`, because S6-B does not own S1's `workspace.rs`. This makes S6-B roughly M instead of L.
11. **P2 — S4-A redoes S1-D, and "at startup" is wrong for macOS.**
    - *Evidence:*
      - S1-D already routes the startup `file_arg` and the handoff drain into `OpenPaths` (S1 WO §3.5 and S1-D step 5). S1-D also rewrites `main.rs`, so the §2 line numbers will be stale.
      - A cold Finder open arrives *inside* `run()` (odoc is delivered before `applicationDidFinishLaunching:`). So "At startup, 'a batch is pending' suppresses Start" sees an empty queue on the Mac.
    - *Change:*
      - "This fixes the cold-launch bug." → "S1-D fixed the single-path case; S4-A swaps S1-D's `file_arg`/`take_pending_file_paths` for `file_args` + `os_open`."
      - "At startup, 'a batch is pending' suppresses Start" → "Start is decided at the first `AboutToWait` drain, not before `run`."
      - `cold_os_open_skips_start_and_opens_b` must enqueue **after** the workspace is built and before the first drain.
      - Use S1's frozen names: `OsHandoff`, no `Recent`, `OpenPaths(Vec, OpenOrigin)`.
12. **P2 — Windows handoff loses files during startup.**
    - *Evidence:* the sender gives up after `SendMessageTimeoutW(.., 2_000)` (`single_instance.rs:235–243`). The first instance does not pump messages until `run`, which comes after `Renderer::new` (`main.rs:847`). Moving the subclass install up (§3.2) does not close this gap.
    - *Change:* add to the S4-A Windows steps: "On a zero return (timeout), the sender retries until a 10 s deadline before exiting." This stays compile-gate only (R3).
13. **P3 — premature pieces.** `MAX_PENDING` + "{n} more files weren't opened…" is a cap and user copy that the spec does not ask for. The spec asks only for payload-length validation, which the `MAX_PATH_UTF16` bound in `decode_handoff` covers. *Change:* delete `MAX_PENDING`, `dropped` and that copy line.

## Needs Ahmed
- **Masked art in PDF export (F1).** Option A: export clipping masks correctly, using MASKS_PLAN Stage 5, which is already planned. Option B: refuse to export any page that contains a mask, with "Clipping masks can't be exported to PDF yet." **Recommended: A.** Refusal would block Export for any file with a mask, and warning-then-exporting would show the art hidden outside the mask. B stays as the fallback only if A misses the day.
- **Default export name for a document opened from a `.pdf`.** The spec says `<name>.pdf`, which is the open file itself, so the guard would always refuse it. **Recommended:** suggest `<name> export.pdf` in that one case only.
