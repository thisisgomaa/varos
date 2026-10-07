> **Status:** accepted — owner (Ahmed) 2026-10-07 («ماشي»); C1 commissioned after Bridge slice 3 (artboards)
# ADR-0011: Bridge connection and trust — register once, attach safely

**ملخص بالمصري**

المالك جرّب Claude Code مع Varos يوم 2026-10-07 واشتغل؛ المطلوب دلوقتي سيستم يعيش سنين، مش سكريبت لكل فتحة.  
نسجّل الوكيل مرة لكل مستخدم، وبعدها يلاقي Varos من أي فولدر؛ لو الاختيار ملخبط يسأل بدل ما يعدّل بورد غلط.  
هوية الاتصال تفضل في Keychain، والجلسة تتجدد كل تشغيل؛ مفيش توكن في الأوامر ولا ملفات إعداد الوكلاء.  
أول اقتران يطلب موافقة الإنسان في Varos، بصلاحيات واضحة تتفتكر وتتسحب؛ أي واجهة لازم صور وموافقة قبل الكود.  
كذا وكيل يشتغلوا مع بعض بمراجعات وإيصالات منفصلة، والإنسان له الأولوية والحذف والتراجع محتاجين إذن مضبوط.  
ده اقتراح مستني المالك: أربع قطع ببوابات، والريموت والويب ليهم مكان في التصميم بس مش تنفيذ دلوقتي.

- **Date / decision owner:** 2026-10-07 / Ahmed.
- **Supersedes:** Nothing while proposed. On acceptance, replaces only ADR-0009 §5's per-launch credential as the normal connection mechanism and refines §7's connection handshake; API, §8 protections and local-only shipping boundary remain.
- **Related:** [vision](../VISION_AI_NATIVE.md), [ADR-0009](ADR-0009-varos-bridge.md), [PLAN](../PLAN.md), [standing rules](../../CLAUDE.md).

## 1. Context and decision boundary

Owner report today: Claude Code connected to a live slice-1 Varos and worked. This is owner-reported acceptance of that exercise, not a new test performed for this ADR. His direction: «عاوزين نعمل السيستم للمستقبل مش دلوقتي بس».

Inspected baseline: [README](../../varos/crates/varos-bridge/README.md), [ipc.rs](../../varos/crates/varos-bridge/src/ipc.rs), [bridge-connect.sh](../../tools/mac/bridge-connect.sh). The listener publishes `api/socket/token/epoch` under `$TMPDIR/varos-bridge-<pid>-<launch>/endpoint.json`, checks peer uid, and issues a new token each launch. `Hello` carries a token and caller-chosen client ID; it is not a persistent agent identity. The script selects the last live-looking endpoint, builds repo binaries and writes a project `.mcp.json` containing the token in arguments. That proves a slice, not durable discovery, identity or installation.

**Decision:** separate discovery, authenticated principal, authorization, connection session and Bridge commands. Use a per-user local registry, Keychain-backed identities, human pairing, revocable capabilities and a bundled stdio proxy. Keep core pure: registry, credentials and approval belong to host/adapter code. This document specifies future behavior, not implemented flags or approved UI.

## 2. Discovery and deterministic attachment

Choose a well-known **per-user runtime directory with one registry entry file per live host**, not one shared mutable endpoint file or a mandatory daemon. On macOS resolve the OS user temporary directory via the platform API (not the shell's inherited TMPDIR), then use `varos-bridge/hosts/<instance-id>/endpoint.json`. Keep socket basenames short and validate the platform socket-path limit; C1 must verify the concrete layout on real accounts. Linux later uses validated `XDG_RUNTIME_DIR/varos-bridge`; no unsafe shared `/tmp` fallback. Windows later uses a user-ACL registry directory and named pipes with user-SID checks; runtime support remains deferred.

Directories are 0700; records/sockets 0600. Create without following links, publish bounded records by atomic rename, and validate directory/file ownership, type and identity using directory-relative operations. Each host owns only its entry; concurrent starts require no global read-modify-write. Remove only that identity-checked entry on exit. Bounded stale-entry cleanup never recursively follows arbitrary paths.

A registry record contains `discovery_version`, random `instance_id`, launch `epoch`, pid/start identity, host mode, local transport address, connection-version range, app build and host public-key fingerprint. **No credential, board name, file path or document contents.** Registry contents locate a candidate; they never authorize or authenticate it. Verify live peer uid and a nonce challenge signed by the paired host identity, bound to epoch and negotiated versions; a pid/socket existing is insufficient. Ignore stale/dead entries with a bounded diagnostic; never choose the newest/last record by accident.

The proposed host model lets one process own several windows and board sessions; several launches have distinct instance/epoch pairs, even with one per-user host identity. A headless host publishes the same record with `mode=headless`, sharing the contract but owning its own boards. Authenticate before listing authorized boards or resolving a supplied file selector. Names are display data; the address is `(host identity, instance, epoch, board handle)`.

Proposed command contract:

- `varos-bridge mcp` equals `mcp --attach auto`; no launch token, cwd or repo lookup required. MCP initializes promptly with static tools even if Varos is absent or pairing pending; tool calls return typed connection status, avoiding startup-timeout approval loops.
- `--attach auto` attaches only when one eligible paired host remains, or one explicit owner-selected target matches. With no paired host, a sole local candidate can enter pairing (no document access); several unpaired hosts require owner selection. With several hosts return `ambiguous_target` and a bounded authorized candidate list; do not silently favor desktop over headless or newest launch.
- `--instance <id>` or `--pid <pid>` narrows the host; pid is only a selector, verified with start identity/epoch. `--board <handle>` requires its host/epoch; `--file <path>` resolves an already-open authorized backing file by canonical file identity, never opens/scans disk or grants file access. Duplicated open files remain ambiguous.
- For an unqualified board request, choose only the sole eligible board, then pin its handle. Several windows/boards require an explicit board or owner-selected target. Do not follow focus changes, silently switch tabs, or rebind queued edits after a close/relaunch. Keep slice-1 inactive-board mutation refusal until a separately accepted host policy changes it.
- No running host: return `host_not_running` with a human launch action; no implicit headless host, document creation or network fallback. After launch, re-discover on the next tool call. Reconnection authenticates anew; changed epoch returns `session_reset`, invalidates handles and requires resync before new edits.

| Alternative | Decision / reason |
|---|---|
| Single endpoint file / last-live socket | Reject: races across launches and silently picks the wrong document host. Per-host registry files scale without a broker. |
| launchd-style socket activation | Defer: useful for an explicitly installed headless service later, but adds daemon lifecycle/upgrade/consent complexity and does not solve board selection. An activated host must still publish/authenticate through this contract. |
| `varos://` as discovery transport | Use only as a human-facing launcher (§4); URL dispatch cannot authenticate a host or carry capabilities safely. |
| mDNS / LAN probing | Reject for default discovery: exposes presence, invites spoofing and expands the network boundary. Future remote hosts are explicitly enrolled by address and identity. |

## 3. Identity, pairing and capabilities

Choose long-lived **per-user installation identity plus separate per-agent key credentials**, stored in macOS Keychain, over re-entering a bearer token every launch. The host has its own key; each paired agent profile has a different key/id. The proxy accesses credentials by opaque reference, proves possession over a fresh challenge, and never gives a private key or reusable bearer to the model. Per-launch epochs and short-lived authenticated sessions remain; persistence of identity must not mean persistence of an old session's authority.

Use reviewed crypto/platform APIs, no custom cryptographic primitives. Keychain access should be restricted to the signed Varos host/helper; exact signing/access-control behavior across upgrades, development builds and headless logins is a **C1 release blocker to test**, not a claim that any process with the same uid is isolated. Locked/unavailable Keychain returns `credential_unavailable`; never downgrade to a plaintext file. Plan a `CredentialStore` abstraction for Windows Credential Manager/DPAPI plus SID-bound transport later; do not pretend this is implemented. Headless service accounts need their own store and owner pairing, not copied desktop keys.

An agent identity is a paired key/profile, not an LLM, conversation title, pid or self-declared MCP `clientInfo.name`. No-argument mode uses that client's remembered profile where unambiguous; unknown/ambiguous clients pair or select a public `--identity <profile-id>` reference. Registration may record that reference, never its secret. Client metadata can suggest a label and locate a previously owner-approved profile mapping only; key possession is still the authenticator. Never select another profile merely because it has broader grants. Standard stdio MCP does not authenticate the calling vendor; stronger executable provenance is platform-specific and must be tested. Same-uid clients able to invoke the helper/profile may impersonate it; per-agent grants organize and revoke trust, not sandbox a compromised account.

First-pairing flow (states and content, **not pixels**):

1. A new local proxy checks uid, discovers candidates and requests pairing. No board metadata is released yet. Host records a bounded expiring nonce, candidate public key, requested scopes and unverified client label; repeated attempts coalesce.
2. Varos asks the human once: identify the pending connection by matching nonce/fingerprint, show what identity is verified versus merely claimed, target host/boards, scope and duration. Choices: deny, allow this session, or remember selected capabilities. Never auto-approve because the agent says the owner agreed.
3. Host approval binds the key, selected profile, board policy and capabilities. The proxy pins the host key through this trusted ceremony. Timeout/denial leaves no grants; the agent receives `pairing_required`/`pairing_denied`, not a hanging document call. A later call resumes after approval.
4. Remembered grants survive restarts for that host/user/profile. Default board policy is explicitly chosen boards/files; session-only unsaved boards expire with their handles. “All boards, including future boards” requires an explicit owner choice. Reopen a remembered file only by validated file identity; replacement/mismatch requires reauthorization.
5. Scope expansion, changed host/key or invalid identity asks again. Headless pairing uses an owner terminal/control channel separate from agent stdio, or an exact pre-provisioned owner grant; unattended unknown agents are refused.

Every visible pairing, connection status, scope/revocation control and copy item requires saved mockups and an owner-reviewed plan **before UI implementation**, per CLAUDE.md / ADR-0009. Pairing once does not eliminate client-vendor approvals or operation-specific human confirmations.

| Capability | Host-enforced meaning |
|---|---|
| `read` | List/describe only authorized boards; no Recent entries, arbitrary files or unapproved board metadata. |
| `edit` | Ordinary supported reversible edits and intentional selection on granted boards, with revision checks. Does not include history or filesystem writes. |
| `destructive` | Eligibility to request delete, replacement, ungroup or discard; **not** standing confirmation of any particular destructive payload. Also requires edit and an exact host-side grant. |
| `files` | Separate read/open, save and export/write grants for explicit files/roots; use-time identity/path checks under ADR-0009 §8. No shell or home enumeration. Existing-destination overwrite also needs an exact confirmation; current-backing-file save follows its narrowly scoped save grant. |
| `history` | Eligibility to request shared undo/redo; exact host-side confirmation still required, including when the top entry belongs to a human. |

Host evaluates the intersection of remembered grant, session scope, board/file policy and implemented verbs on **every call and again before commit**. An agent cannot widen it, delegate its identity or enable an unsupported verb. New scopes default denied after upgrades. Revoke a profile/key or a single grant through a trusted owner channel; persist a revocation list and monotonic trust generation shared across local hosts. Serialize the authoritative generation check and publication against revocation using a shared trust-store lock/transaction; never rely only on cached notifications. Cancel queued work and disconnect revoked sessions; failed trust-store reads deny. Already committed edits remain in ordinary history, not silently rolled back. Rotation revokes the old key; “reset all trust” revokes all profiles and requires pairing again. Revocation must outlive backups containing old agent configs.

Keep bounded owner-only audit records: time, agent/session ID, request ID, verb, opaque board handle, from/to revision, result/error code and approval/revocation event. No names, paths, prompts, geometry, document contents, payloads or credentials. Propose rotation at 10 MiB and deletion after 30 days, whichever bounds retention first; owner can clear sooner. Record truncation/loss honestly. The local audit is diagnostic, not tamper-proof against its owner. Reserve audit capacity before mutations; storage failure denies new mutations and keeps human editing available.

## 4. One-time installation and registration

Ship the signed `varos-bridge` executable **inside** `Varos.app/Contents/MacOS/varos-bridge`. Standard stable installed path: `/Applications/Varos.app/Contents/MacOS/varos-bridge`; never a Cargo target, worktree, version-numbered folder or download-on-connect package. App and helper update together under the visible update policy. A user-selected nonstandard app location uses its absolute path; moving it needs registration repair, not re-pairing if identity is preserved. An optional PATH shim is convenience, not a runtime dependency.

One install-time setup per chosen agent client, not per project or app launch. These are **proposed Varos commands after C1–C3**, with vendor registration syntax checked on 2026-10-07:

```sh
claude mcp add --transport stdio --scope user varos -- /Applications/Varos.app/Contents/MacOS/varos-bridge mcp
codex mcp add varos -- /Applications/Varos.app/Contents/MacOS/varos-bridge mcp
```

[Claude Code's official MCP docs](https://code.claude.com/docs/en/mcp#option-3-add-a-local-stdio-server) document stdio command arguments after `--` and user scope across projects (`~/.claude.json`). [Official Codex MCP docs](https://developers.openai.com/codex/mcp) document `codex mcp add NAME -- COMMAND` and default user configuration `~/.codex/config.toml`; there is no Claude-style `--scope user` in the inspected local `codex mcp add --help`. These checks establish syntax only; no registration or live-client test was performed for this ADR. Managed settings, project overrides, disabled servers and client sandbox policy can block access and must produce actionable diagnostics, never bypasses.

Generic `.mcp.json`-style payload for clients accepting `mcpServers` (merge only this entry; preserve unrelated servers):

```json
{
  "mcpServers": {
    "varos": {
      "command": "/Applications/Varos.app/Contents/MacOS/varos-bridge",
      "args": ["mcp"]
    }
  }
}
```

This is a shape, not a universal filename. [Cursor's official instructions](https://cursor.com/help/customization/mcp) specify `~/.cursor/mcp.json` for all projects and `.cursor/mcp.json` for one project; use the former for this goal. Other clients choose their documented user-level location. An optional `--identity <public-profile-id>` in args references Keychain; neither args nor env/config contains a token. Fresh conversations reuse the paired profile; a genuinely new client identity pairs once.

Provide `varos://connect` and a “Copy connection for agents” action as human handles after mockup approval. They open connection/setup guidance with token-free registration text; they never grant access, execute shell text, overwrite client settings or accept secrets in URLs. Treat every URL as untrusted; allowlist route/parameters, reject arbitrary commands/paths and do not rely on scheme ownership as authentication. OS scheme registration belongs to app packaging.

Later publish a namespace-verified `server.json` in the [MCP Registry](https://modelcontextprotocol.io/registry/about) for product/package discovery. The registry describes installation; it is neither a live local endpoint directory nor a trust authority. Native-app distribution metadata/schema support must be verified at that gate; do not invent an npm dependency or assume listing auto-registers every client.

## 5. Connection versioning and slice-1 transition

Define connection protocol `connection="1.0"` independently of Bridge API `api="1.x"`, MCP's date-based revision, app release and `.vrs` format. Registry schema has its own `discovery_version`. The authenticated handshake negotiates connection major/minor, Bridge version, transport, host mode, epoch, principal/session, supported verbs, scopes and limits. Bind negotiation into authentication to prevent downgrade; never reveal document data before auth. Connection major changes auth/framing/required semantics; minor adds explicitly negotiated optional capabilities. Unknown required features fail closed; tolerate additive response fields, reject unknown request fields. Publish frozen fixtures and supported-version ranges with each release.

| Client / host | Transition policy |
|---|---|
| Old slice-1 proxy → upgraded host | Keep a separate opt-in legacy listener/endpoint using the exact old Hello/token/API contract. Default off for new installations; owner migration opt-in retains current operation temporarily. Never silently accept legacy auth on the new listener. |
| New proxy → old host | Explicit legacy-endpoint mode only, using a selected owner-only endpoint file read in-process. Never scan/import every token, pair it as durable trust or automatically downgrade after auth failure. |
| New proxy → new host | Registry + paired identity; configs carry only executable/args and optional public credential reference. No legacy token issuance by default. |

Migration setup detects conflicting project/user registrations, explains precedence, replaces only the Varos entry after owner choice, and removes token-bearing Varos args/config entries without logging secrets or making secret backups. Disable the old listener to revoke its token; deleting `.mcp.json` alone does not revoke an already leaked token. Warn that legacy connections retain their old coarse permissions and cannot claim C2 per-agent isolation. Existing scripts/configs keep working only while legacy mode is deliberately enabled; this docs task changes none of them.

Keep the compatibility adapter for at least two stable releases after C3, then retire only after an announced release and owner acceptance of migration evidence. No silent retirement or indefinite insecure fallback. The new proxy reports a clear reconnect/upgrade error for a retired legacy endpoint, including when its socket is gone. API 1.x need not change simply because authentication changes. Host restart loses slice-1 in-memory receipts: never replay mutations automatically across epochs. Persisting receipts is a separate decision.

## 6. Several agents and the human

Allow simultaneous attachments with separate authenticated principals and host-issued sessions. Separate per-session request sequences, cancellation maps and receipt access; key receipts by `(principal, session, epoch, request_id)`. Reconnect can resume a retained session only after same-principal proof; another agent cannot choose its ID to read/cancel/replay its requests. Retain current bounded receipts/high-water rules; expired receipt means unknown outcome requiring resync, not proof of rollback. Selection and undo history remain shared human-visible document state, not private agent copies.

Choose optimistic concurrency with short atomic commits, **no long-lived board-level edit lease**. A lease would let a dead/slow agent block the owner and still cannot replace revision validation. FIFO orders admitted agent work; it does not serialize every current pointer/panel edit. Stage each batch, settle valid human fields as separate human work, and compare `expected_rev` on the owning thread immediately before publication. Invalid fields/active gestures return `busy`; another edit returns `revision_conflict` with actual revision and a bounded authorized change/actor summary, or `resync_required`. The agent must reread/replan; no automatic merge or blind retry with a newer revision.

Human always wins scheduling: an active human gesture is never cancelled or delayed to preserve an agent's plan; stale staged agent work is rejected. Reserve responsiveness for the owner, bound agent work, and allow owner revoke/disconnect while agents are queued. If an agent edit already committed before human input, retain the honest receipt and ordinary undo; priority does not retroactively erase committed work. Preserve ADR-0009's cooperative file writer ownership and external fingerprint checks; this is not CRDT co-editing across processes.

## 7. Threat model and required refusal tests

| Threat | Boundary / mitigation and acceptance evidence |
|---|---|
| Compromised local process with same uid | Peer uid is necessary, insufficient. Keychain/key proof and explicit grants reduce accidental exposure; a compromised account/helper may impersonate profiles or tamper with trust/audit. Test cross-uid refusal, copied references without keys, key mismatch and locked store; do not claim same-uid malware containment. |
| Token leakage through argv / `ps` / shell logs | Normal path has no bearer in argv/env; challenge proof stays on private IPC. Test process arguments, diagnostics and copied text for secrets. Legacy mode is explicitly temporary. |
| Tokens in today's `.mcp.json` | Credential by public reference only; migrate and revoke legacy listener token. Test that repo/user configs and migration backups contain no secret; preserve unrelated config. |
| Stale endpoint, pid reuse, forged registry | Validate pid/start/epoch and authenticated host proof; reject wrong host keys, reused pid, malformed/oversized records and dead socket. Never fall back to TCP or a different board. |
| Symlink races / socket or parent replacement | Owner-only directories, no-follow directory-relative create/open, atomic records and identity rechecks; test link swaps, wrong ownership/modes and bounded cleanup. Peer/key verification remains mandatory after connect. |
| Prompt injection in board names/metadata | JSON-escape control text, delimit as data; never construct instructions, commands, URLs or identity from names. Test names containing fake approvals, newlines and shell syntax. |
| Destructive operation / shared undo | Reaffirm ADR-0009 §8: `confirmation_required` → trusted **host-side** short-lived single-use grant bound to principal/session, epoch, board, verb, IDs/counts or destination, revision/fingerprint and payload digest. Recheck/consume atomically. Scope alone, MCP approval, `confirm:true` or model text cannot issue it; unattended calls without exact grants refuse. |
| Temporary history policy abused | Today's desktop `VAROS_BRIDGE_ALLOW_HISTORY=1` is a host-owned slice-1 policy, not an agent flag. C2 normal mode retires it in favor of exact owner grants; legacy-only compatibility stays explicit. Test that no client can enable it or replay a consumed/stale grant. |
| Resource exhaustion / pairing spam | Keep slice-1 frame/op/target/queue bounds; add per-principal and global rate budgets, fair admission, capped unauthenticated handshakes and coalesced pairing. Proposed starting caps: 10 calls/s, burst 20 per principal; 2 concurrent admitted calls/principal; 3 pairing attempts/min/uid. Return retry delay, no mutation, and reserve human capacity; benchmark/tune before release, not a latency promise. |
| Revoked session races with commit | Check trust generation and scopes at publication, cancel queued requests and invalidate sessions/grants across every local host. Test revoke during staging, restart and old-key restore. Already committed receipts stay honest. |

## 8. Remote/headless/web seam — design now, implement later

Define `EndpointResolver`, `CredentialStore`, `Authenticator` and a host-side authorization context `(principal, host, session, epoch, capabilities, trust generation)`. Local UDS/peer-uid proof and later remote authentication produce the same context; domain handlers never trust transport-supplied scope strings. Local headless uses the existing local boundary; a remote headless service is a different security deployment.

For future remote native hosts, require explicitly enrolled TLS host identity and prefer mTLS device/client keys; short-lived signed, audience/host-bound capability tokens may carry delegated scope with expiry, nonce/token ID and revocation generation. Validate issuer, audience, proof/channel binding and revocation before dispatch; no reuse of desktop launch tokens or transmission of the per-user private key. Both client and server need credential rotation and compromised-host recovery. Remote principals are enrolled independently; Unix uid has no cross-machine meaning.

A browser/WASM mirror cannot read Keychain or a UDS and generally cannot manage native mTLS keys like a CLI. Its later design needs an explicit HTTPS gateway/authorization flow (evaluate current MCP HTTP authorization/OAuth then), origin/CSRF protections and browser-safe short-lived credentials. Signed tokens are not by themselves a complete login flow. Sharing a cloud file does not grant Bridge control. Certificate issuance, token lifetime, remote revocation freshness, gateway storage and consent UX remain C4 decisions with threat review before any listener ships. ADR-0009's no-network boundary remains in force until a separate accepted remote work order changes it.

## 9. Gated delivery and owner acceptance

| Piece | Deliverable and gate before advancing |
|---|---|
| **C1 — stable identity + auto-attach + Keychain** | Registry/handshake/credential-store contract and local implementation; owner-terminal provisioning only for a bounded pilot until C2. Verify restarts, multiple launches/windows, ambiguous/absent targets, same-uid limits, locked Keychain, signing/upgrade behavior, stale/symlink attacks and connection-version fixtures. Never enable read/edit merely because a key exists. |
| **C2 — pairing/scopes/revocation/audit** | Mockups accepted before visible UI code; implement trusted pairing and exact confirmations. Verify per-profile least privilege, no metadata leak before pairing, scope expansion refusal, cross-host revocation races, audit bounds, simultaneous-agent conflicts/receipts and human priority in the real window. |
| **C3 — bundling + one-time cross-agent registration** | Signed helper in the installed .app; token-free Claude/Codex/Cursor user setup, optional profile references, safe legacy migration. Test each installed client from two unrelated folders, after Varos/client restarts and app upgrade; no Cargo/repo/PATH assumptions. Registry listing is later distribution work, not required to pass C3. |
| **C4 — remote seam** | First land transport-neutral interfaces/fixtures and local/headless semantic parity as headless becomes available; no listener implied. Remote implementation waits for accepted deployment/auth threat review and browser/native interoperability evidence; offline local use must remain independent. |

**End-to-end acceptance:** after one-time registration and first pairing, Ahmed says **“use Varos”** to a fresh session of each supported local agent from any folder and it discovers the intended running board, describes it, edits within the grant and returns an ordinary undoable receipt—**no scripts, token copying or re-registration per launch**. A truly new agent identity still asks once; ambiguous targets require selection, unavailable/managed-blocked clients explain refusal. Repeat after app restart/upgrade, with two agents and human input; those safety refusals are part of success, not exceptions to hide.

Each implementation piece needs its own work order, applicable repository gates, independent review before merge and Ahmed's window acceptance before the next. This docs-only change runs document/link/diff checks, not Rust/runtime acceptance. Open evidence gates include actual multiwindow hosting, Keychain access/signing, client identity provenance, native package registry support and remote authorization details. Owner acceptance of this ADR does not approve unseen UI, ship remote access or claim any of C1–C4 already works.
