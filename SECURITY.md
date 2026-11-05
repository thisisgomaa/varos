# Security policy

## Supported version

Only the **latest Varos preview** receives security fixes. Earlier previews and the legacy engine
are unsupported. Update manually from [Releases](https://github.com/thisisgomaa/varos/releases).

## Report a vulnerability privately

Use this repository's **Security → Advisories → Report a vulnerability**, or
[start a private report](https://github.com/thisisgomaa/varos/security/advisories/new).
GitHub's private vulnerability reporting sends it to the maintainers. Include the affected version,
reproduction steps, impact and a minimal example with secrets and private artwork removed.
Do not post vulnerability details in public issues or pull requests.

If the button is unavailable, ask the maintainers to enable private vulnerability reporting in
a feature request, without disclosing the vulnerability. See
[GitHub's private reporting guide](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/report-privately).

## MCP bridge trust model

The default desktop MCP bridge is **same-user only**: it listens on loopback, authenticates
requests with a random token and proves the server's identity before the proxy sends requests.
The endpoint/key files containing the token and server key have mode **0600** (read/write for
the owning user only), inside a **0700** directory. Symlinks are refused. See
[the listener](apps/vectorcraft/src/varos_control.rs) and
[endpoint storage](crates/varos-agents/src/endpoint.rs).

Any process running as your macOS user can read these secrets and edit or save documents without
pairing. This is not isolation from other programs running as you. Undo and actor-tagged history
help review edits; actor labels are attribution, not authentication. Do not share endpoint/key
files or include them in bug reports.

The optional developer `--control PORT` channel is separate and has no authentication or the
default bridge's write guards. Do not expose or forward it to other users or networks.
