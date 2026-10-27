# Codex Cloud for Varos — keep the Mac cool (2026-10-27)

Owner decision: implementation lanes and their gates (tests, clippy, fmt) run in **Codex Cloud**
(Linux); the Mac only does the final `cargo build --release -p vectorcraft` for a bundle when the
owner wants to hand-test. Repo: `github.com/thisisgomaa/varos-next` (private). The `varos-text`
dependency is the public `thisisgomaa/varos` over HTTPS (no SSH key needed in the cloud).

## One-time: create the environment (owner, in the ChatGPT/Codex app)
Codex → Environments → **Create** → repository `thisisgomaa/varos-next` → container image:
*universal* (has Rust) → **Setup script** (paste the block below) → Internet access: **On** during
setup (crates.io, GitHub) → Save. Then give the moderator the environment id (looks like
`ccarenv_…`; it appears in the environment's page/URL).

```bash
set -euo pipefail
# System libraries the egui/winit crates need at compile time (same list as .github/workflows/ci.yml)
sudo apt-get update -qq
sudo apt-get install -y -qq pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxcb1-dev libxcursor-dev \
  libxrandr-dev libxi-dev libgl1-mesa-dev libasound2-dev libfontconfig1-dev
# Rust: the workspace pins rust-version 1.95; use stable and the components the gates use
rustup toolchain install stable --profile minimal -c clippy -c rustfmt
rustup default stable
# Warm the caches so the agent's first build is fast and works offline afterwards
cargo fetch
cargo build --release -p vectorcraft-ui-egui --tests -j "$(nproc)"
```

## Per task (moderator)
```bash
codex cloud exec --env <ENV_ID> --branch <lane-branch> "<brief text or: read docs/varos/lanes/<brief>.md and do it>"
codex cloud status <TASK_ID>
codex cloud diff <TASK_ID>      # review
codex cloud apply <TASK_ID>     # lands the diff locally on the lane branch; moderator gates + merges
```
Rules unchanged: author ≠ reviewer, one owner-visible stage per lane, no commits by agents, the
moderator merges and hands off. Known release-only failures (9 `panels::gradient::tests::*` +
`tests_rotateview::escape_after_closing…`) are expected on Linux too.
