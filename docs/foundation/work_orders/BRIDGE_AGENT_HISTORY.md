> **Status:** proposed 2026-10-08 — design for the next Bridge piece after open local trust (PLAN A3-next). No code yet.

# Per-agent history — "what did each AI do, and can I review it"

Owner, 2026-10-08: «في المستقبل هيكون في هيستوري منفصل لأي AI بحيث يبقى في مراجعة عن اللي هو عمله». With pairing and scope gates gone (ADR-0011 Amendment 3), review is the owner's safety model: undo + a readable record of who did what.

## What exists today

- Core history is `undo: Vec<Document>` / `redo: Vec<Document>` (snapshots, cap 200) in `varos-core/src/editor.rs`; `commit()` pushes the pending snapshot and bumps `rev`. No entry knows **who** made it or **what** it was.
- Every Bridge batch is one `commit()` (one undo step) with an edit receipt (created / changed / removed ids, artboard, rev).
- The Bridge audit log (`conn/audit.rs`) records verb, board, revision, result, `profile_id` and client `label` per request, without payloads. It is per-user, not per-board, and is not visible in the app.
- Human edits are committed by the UI with no author either.

## Decision shape (proposed)

1. **Authored history entries in core.** Add a parallel `Vec<HistoryEntry>` beside `undo` (same length, same cap, same push/pop/clear discipline), where
   `HistoryEntry { rev_after: u64, actor: Actor, summary: Summary, at: UnixMillis }`,
   `Actor = Human | Agent { profile_id: String, label: String }`,
   `Summary = { verbs: Vec<&'static str>, created: u32, changed: u32, removed: u32, artboards: Vec<ArtboardId> }`.
   `commit()` keeps its signature; a new `commit_as(actor, summary)` is what the Bridge host calls; the UI keeps calling `commit()` (= `Human`, summary derived from the pending diff cheaply: counts only).
   Not persisted in the `.vrs` (history never was); not part of equality; `FORMAT_VERSION` unchanged.
2. **`history` verb grows a read side** (API 1.1, additive): `history {action:"list", limit, cursor}` returns entries newest-first with `actor`, `summary`, `rev_after`, `at`, plus `undo_depth`. `undo`/`redo` unchanged (shared history; an agent may undo a human step — ADR-0009 §3 stays).
3. **"Undo this agent's last edit"** = allowed only when the top of the undo stack is that agent's entry (no out-of-order undo; snapshots are linear). The UI and the CLI expose exactly that rule; nothing clever.
4. **Review surface in the app** (design-first, mockups before code): a History list in the Layers box tab strip or the Board section — rows "14:32 · Claude · edited 3 objects on Poster", click = select the changed objects (ids kept in the entry while they still exist), ⌘Z semantics unchanged. Agent rows use the `AGENT` colour (orange) from the on-canvas presence law; human rows plain.
5. **Audit stays the forensic record** (per user, append-only, no payloads); history is the per-board, in-memory, reviewable one. They are not merged.

## Not in this piece

Branching history, per-agent revert of non-top steps (needs an operation log, not snapshots), persisting history into the file, cross-board review, and the presence animation (separate, landed/landing).

## Gates

- Core: entry vector stays in lock-step with `undo`/`redo` through commit/undo/redo/cap/`replace_doc` (property test); human commits unchanged in behaviour and cost (benchmark the counts-only summary on a 10k-object board).
- Bridge: `history list` frozen fixture; actor correctness for human vs two agents; "undo mine" refusal when not on top; API 1.0 fixtures byte-identical.
- App: mockup picked by the owner; list rows ≥ 24 pt; select-changed works; ratchets.
- Independent review (Opus) + owner hand test.
