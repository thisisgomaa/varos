# Varos workshop v0

How agents build Varos. Agents only: there is no UI or dashboard for the owner. Updated 2026-11-02
(hook rule wording, local-gates note).

## Roles

| Role | Who | Does |
|---|---|---|
| Owner | the project owner | Decides design, scope, budget, publishing; hand-tests in the real window. |
| Moderator | the Claude Fable session | Plans, writes task cards, dispatches, runs fetch/build/gates, reviews, keeps `STATE.md` and `BACKLOG.md`, reports to the owner in plain Egyptian Arabic. Does not implement. |
| Implementer | Codex Sol 6.1, Opus 5.5 (later Grok) | Executes one task card inside its lane. Never commits to `main`, merges, pushes or publishes. |
| Reviewer | a different agent from the author | Reviews the diff against the card and the gates. Author is never the reviewer. |

## Task card template

```
Lane:            lane-<name>
Goal:            one outcome, in plain words
Out of scope:    what not to touch or "improve"
Base commit:     <sha on main>
Owned files:     files/dirs this lane may change
Forbidden files: files it must not change (other lanes' files, upstream code beyond hook lines)
Gates:           the commands from "Gates" below that apply
Reviewer:        <agent, not the author>
Budget:          max attempts / time; stop and report when hit
Hand-test steps: 3-5 steps the owner does in the real window, and what he should see
Report:          what changed, how verified (commands + real output), what is left
```

## Lane rules

- One git worktree per lane, branch `lane-<name>`, in a `lanes/` folder beside the repo
  (`AI workspace/lanes/lane-<name>`). Earlier lanes lived under
  `AI workspace/reference/artcraft/spikes/lane-*`.
- Codex's sandbox has no network and cannot write outside its working directory: the moderator
  runs `git fetch`, any dependency download, and builds that need a shared target.
- The owner's Mac overheats: **at most 2 lanes building at once**, always `-j 4`. A third lane may
  be read-only or design work. The moderator runs its own gate passes after lanes finish, one at a
  time, and never prebuilds what a lane will build itself.
- Lanes follow the hook rule in `FORK.md`: new code in Varos modules; upstream files changed only
  where unavoidable, with minimal hook lines marked `Varos`.
- Lane reports go to `docs/varos/lanes/<lane>-report.txt`.

## Gates (before any merge to `main`)

1. `cargo build -j 4 -p vectorcraft`
2. `cargo test -j 4 -p vectorcraft-ui-egui` (when UI code changed)
3. `cargo test -j 4 -p vectorcraft-text -p vectorcraft-text-arabic` (when text code changed), plus
   `cargo check -p vectorcraft-text --target wasm32-unknown-unknown`
4. `cargo clippy -j 4 -p <touched crates> --all-targets -- -D warnings`
5. `cargo fmt --all -- --check` and `git diff --check`
6. Every new asset has an `ASSETS.md` row, a `NOTICE` line and its licence text.
7. Independent review by an agent other than the author; findings fixed.
8. Anything visible: owner hand test. Only owner-accepted items become **done** in `STATE.md`.

Upstream's full `cargo xtask ci` is heavy; run it before publishing and after upstream merges.
All gates run locally: GitHub CI has never run on this project, so reports quote the local
commands and their output and never say "CI green".

## State files

- `docs/varos/STATE.md` and `docs/varos/BACKLOG.md` are the only truth for the fork. Chat history
  and memory are not.
- `docs/varos/FORK.md` holds the rules; this file holds the process.
- Lane reports under `docs/varos/lanes/` are evidence, not state.

## What goes to the owner

- **Design:** anything he will see or feel (screens, icons, cursors, colours, behaviour).
- **Scope:** adding, dropping or reordering backlog items; anything that changes upstream code
  beyond hook lines.
- **Budget:** when a card hits its attempt/time budget, or a usage limit is hit (report at once).
- **Publish:** any push, PR to upstream, release, or public post.

Everything else (how to implement, which agent, retries within budget) the moderator decides and
reports afterwards.
