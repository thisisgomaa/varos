# Varos AI panel — plan (owner request 2026-10-26)

> Status: proposed, owner-requested; design mockup before code (design-first rule). Extends
> `docs/VISION_AI_NATIVE.md` (old repo, 2026-10-06) step 5 "chat box inside the desktop app",
> and adds the owner's new point: **bring-your-own-key image generation**.

## What the owner asked (his words, paraphrased)
- «عاوز أضيف بانل AI… أسجّل دخول من خلال كوديكس مثلاً وأكتب التعديل اللي عاوزه وكوديكس ينفّذ جوّه فاروس».
- «فوتو جينيريشن وربط API جوّه فاروس نفسه: أحط API بتاع OpenRouter أو غيره وأستخدم المفتاح بتاعي في توليد صور».
- «عاوز أدّي حرية للمستخدمين — دي النقط اللي هتبيع، وأقوى من الكرافت الجديد».
- «في خطط قديمة موجودة، شوفها الأول وضيف دي معاهم».

## What already exists (do not rebuild)
| Piece | Where | Status |
|---|---|---|
| Agent layer: loopback endpoint, HMAC proof, actor-tagged history, orange presence, write guards | `crates/varos-agents`, `crates/ui-egui/src/varos/agents.rs`, `apps/vectorcraft/src/varos_control.rs` | merged (v8+) |
| MCP proxy for any CLI agent: `vectorcraft-cli mcp` (auto-attach, host_not_running / ambiguous_target) | `crates/mcp/src/varos_connect.rs` | merged |
| Board-scoped verbs: stable board ids, revisions, typed refusals (target_changed, stale_revision, wrong_board, person_busy) | lane-agents2 (`crates/mcp/src/varos_boards.rs`, `ui-egui/src/varos/boards.rs`) | in review 2026-10-26 |
| Images: Place / embed / link, image object, exports | upstream (links.rs, place) | merged |
| Old vision: principle 4 «no middleman, no keys required — drive the user's own Claude Code / Codex CLI» | `docs/VISION_AI_NATIVE.md` | standing |

So the panel is a **front door** to machinery that exists; the new work is the panel, the CLI driver,
and the image-provider adapters.

## Principles (carried over + new)
1. **The user's own account, not ours.** Agents = the CLI the user already pays for (Codex, Claude Code).
   Varos never proxies their traffic and never stores their password. "Sign in" = run the CLI's own
   login (it opens the browser); Varos only reads the login *status*.
2. **Keys are optional and the user's.** For image generation (and, later, API-based chat) the user
   pastes their own key (OpenRouter, OpenAI, fal, Replicate, Stability, a local server). Stored in
   `~/Library/Application Support/Varos/keys.toml`, mode 0600, same policy as the agent endpoint
   file (ADR-0011 Amendment 2: no Keychain, no OS prompts). Never written into project files or
   documents. Zero cost to the project.
3. **Agent edits are ordinary edits** (undo, Layers, History with the agent's name, orange presence).
   The panel never takes over the Mac: it drives the board through the board-scoped verbs only.
4. **No modal, no approval prompts** (owner rules 2026-10-08/10-20). Refusals show inline in the panel.
5. **Tokens are a constraint**: the panel sends summaries/diffs, never the whole file.

## The panel (one Varos card, rail icon "AI")
Tabs: **Ask** · **Images** · **History**.

### Ask (agent chat)
- Provider picker: Codex · Claude Code · (later) OpenRouter model with own key.
- Status row: "Codex — signed in as …" / "Not signed in — [Sign in]" (runs the CLI login, which opens
  the browser; Varos polls status). No credential UI of ours.
- Input: plain text (Arabic/English). Scope chips: *This board* (default) · *Selection* · *Artboard N*.
- Run: Varos spawns the CLI as a subprocess with **our MCP server registered** (`vectorcraft-cli mcp`),
  passes the prompt + a short board summary + the board id/revision, and streams the transcript
  (assistant text, tool calls as one-line receipts) into the panel. The agent edits the board through
  the verbs; presence/history/undo work as today. Stop button kills the subprocess; the board stays
  consistent (every verb is one undo step or a batch).
- After a run: "Undo this run" (one click = undo the run's steps, from the actor-tagged history).
- Transport per provider (to confirm in the A1 spike):
  - Codex: `codex exec --json` (streamed events) with MCP server config passed via `-c`, or
    `codex app-server` (JSON-RPC, experimental). Login status via `codex login status`.
  - Claude Code: `claude -p --output-format stream-json --mcp-config <file>`.
  - Both run with cwd = a scratch folder; file tools are irrelevant since the work is through MCP.

### Images (bring your own key)
- Providers (adapter trait `ImageProvider { generate, edit, variations, cost_hint }`): OpenRouter
  (images-capable models), OpenAI Images, fal.ai, Replicate, Stability, and **Local HTTP** (ComfyUI /
  Draw Things / any OpenAI-compatible endpoint) — all behind one JSON contract.
- Key management row per provider: paste key → stored 0600 → "Test" call → shows model list.
- Generate: prompt, size/aspect (defaults to the selected artboard or a chip), count, seed; result
  thumbnails → "Place" (embedded image object on the active artboard, centred or into the selection's
  bounds), or drag onto the canvas.
- Edit / variations: the selection (vector or image) is rasterised at the chosen ppi and sent as the
  input image + prompt; the result is placed as a new object beside/over the source (never replaces
  without the user's click). Masks: the selected shape becomes the mask.
- Every placed image keeps metadata (provider, model, prompt, seed, cost) in the object's `varos.ai`
  field so "Regenerate" and "Variations" work from the Layers/Properties context menu.
- Cost: provider-reported cost per call shown next to the result; nothing hidden.

### History
- The per-agent review list (old S6 item): runs with who/when/how many steps, "back to before this
  run", and the receipts (verbs called). Reuses the activity log already in `varos-agents`.

## Phases (order, no dates)
| # | Slice | Depends on | Size | Owner exit test |
|---|---|---|---|---|
| A0 | **Design mockup** of the panel (3 tabs, sign-in states, run transcript, image grid) — HTML, kit rules; owner picks | panel-kit mockup | S | he approves the mockup |
| A1 | **Ask via Codex**: subprocess driver, streamed transcript, scope chips, stop, undo-this-run; sign-in status | lane-agents2 merged | M–L | type «اعمل 3 مربعات ملونة» → Codex does it on the open board, undo in one click |
| A2 | **Ask via Claude Code** (same driver, second transport) | A1 | S–M | same test with Claude |
| A3 | **Images BYO key**: keys.toml 0600, OpenRouter + OpenAI + local HTTP adapters, generate → place with metadata | upstream Place | M | paste his OpenRouter key → generate → image on the artboard |
| A4 | Image edit/variations from the selection + masks; Regenerate from the object | A3 | M | select a logo → "make it watercolor" → new object beside it |
| A5 | **API-based agent loop** (OpenRouter chat model with tool-use over our verbs) for users without a CLI | A1, A3 | L | a user with only an OpenRouter key runs «اعمل…» |
| A6 | History tab (S6 review UI) | A1 | M | "back to before this run" |

## Risks and decisions to take
- **CLI protocol churn**: `codex exec --json` / app-server are evolving; the driver isolates the
  transport behind one trait so a change is one file.
- **Login flow**: the CLI opens the browser; Varos must not block or pop a modal; show inline status
  and a "Retry" link. (No Keychain, no password fields, ever.)
- **Images = raster objects**: embedded bytes grow files; keep the proxy + ppi rules from upstream's
  image object; offer "link to file" for big results.
- **Content policy / safety** belongs to the provider; Varos shows provider errors verbatim.
- **Selling point wording** (for README/launch): «موديل بتاعك، مفتاح بتاعك، ملفاتك على جهازك — مفيش اشتراك».

## Mapping to older plans
- `docs/VISION_AI_NATIVE.md` step 5 (chat box) → A1/A2/A6 here. Principle 4 stays the default; BYO
  key (A3–A5) is the *optional* second path, still "no middleman".
- BACKLOG item 11 (agents S4 verbs + S6 review UI) → S4 verbs are the board-scoped verbs (lane-agents2);
  S6 = A6.
- Programme plan Phase 3 (images v6: Place/embed/link) is the prerequisite for A3 — upstream already
  supplies it in the fork.
