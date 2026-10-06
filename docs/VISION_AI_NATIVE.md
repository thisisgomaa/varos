> **Status:** current — owner vision, recorded 2026-10-06. Governs direction; each concrete step still gets its own ADR / work order before code (FOUNDATION_CHARTER §3).

# Varos — the vision: a smart Illustrator alternative that humans and agents work on together

Recorded from Ahmed's words on 2026-10-06, after the cycle close `cycle-2026-10-06`. This is the *why* and the *shape*; nothing here is a schedule or a promise.

## In one paragraph (owner's words, paraphrased)

Varos is the smart alternative to Illustrator that we can keep developing for years: more features, more elements, and above all **humans and agents working on the same files** — intelligently, fast and cheap in tokens — **outside the walls of the companies that hold us by the neck and ask for money every month.** Free, open source, local-first.

## Principles

1. **The document is the API.** A `.vrs` board is readable JSON and every user action is already a command (`EditCommand` → one FIFO queue, one undo history). Agents get the *same* commands by name; nobody drives the UI with pixels and clicks.
2. **Tokens are a design constraint, not an optimisation.** Agents read summaries, not files; diffs, not re-dumps; semantic verbs (align, distribute, boolean), not coordinates; stable element ids; a small CPU snapshot only on request; a batch of commands = one undo step.
3. **Human continues where the agent stopped.** Agent edits are ordinary edits: visible in Layers, undoable with ⌘Z, saved like anything else. No separate "AI mode".
4. **No middleman, no keys required.** Varos never talks to OpenAI/Anthropic itself. It drives the user's own **Claude Code / Codex CLI** on the machine (their subscription, their login). API keys are optional, never needed. Zero running cost for the project.
5. **One codebase, several hosts.** Desktop (macOS native, GPU) for people; `varos-core` headless (CLI / library / server) for agents; the web build is a **mirror** of the desktop app from the same code, not a second product.
6. **Open standards and open-source building blocks** wherever a choice exists (MCP for agent tools, WebDAV/Drive APIs for files, open fonts, open formats SVG/PDF).

## The three hosts

| Host | Who | What it must do | Status |
|---|---|---|---|
| **Desktop app** (macOS first) | humans | the full editor; box system, panels, tools | exists |
| **Varos Bridge** — `varos-core` without a screen | agents, scripts, servers | open / describe / edit / export PDF / snapshot, through ~20 verbs, exposed as an **MCP server** and a **CLI** from one implementation; runs with or without the desktop app (attached to a live window, or headless) | **next** — ADR first |
| **Web mirror** | sharing, review, quick access | the same app compiled to WASM (winit + egui + wgpu on WebGPU), reduced: open from the user's cloud, view, light edits, export, comment; no native menu / Finder / Trash | later, after the Bridge |

**First users of the Bridge are us** (Ahmed + the agents building Varos itself). The in-app **chat box comes later**; proving external control (Claude Code / Codex driving a live board) comes first.

## Files and sharing (owner's picture, recorded as ideas, not decisions)

- A board lives wherever the user keeps files: on disk, or in **their own cloud drive** (Google Drive first; open-source/self-hosted alternatives such as Nextcloud/WebDAV on the same seam).
- **Access = sharing on the drive.** The web mirror lists the boards the user owns, was given access to, or that sit in a folder they allowed. Paste a Drive link into Varos → it opens.
- A collaborator (e.g. "Abbas on the server") creates a board and shares it → it shows up for Ahmed → he opens it on the web or on the desktop, edits, saves → the owner receives it as a **change request, like a GitHub pull request**: see the diff, accept or reject. New uploads happen by putting the file on the drive and dropping the link into Varos.
- Candidate open-source tech to evaluate later: cloud APIs behind one `FileStore` trait (local, Google Drive, WebDAV/Nextcloud, S3-compatible); board diffs from our own JSON model (we already have a migration/diff-capable format); versioning either on the drive's own file versions or a git-style history; live co-editing (CRDT: Automerge / Yrs) only if ever needed — not before.

## What changes in existing decisions

- **ADR-0004** ("no introspectable schema / no plugin API yet") will be **superseded** by the Bridge ADR: a versioned, documented command API becomes a product promise.
- **ADR-0001** (native GPU UI, no web views) stays for the desktop app. The web mirror is the *same* native stack compiled to WASM, not a DOM/Electron app — ADR-0001's reasoning holds.
- Everything else (box system, design-first, format v3, gates and cross-review) stays.

## Order of work (direction, not dates)

1. **ADR "Varos Bridge"** — the 20 verbs, the summary format, batch/undo rule, attach vs headless, account-based CLI driving, versioning of the API. Then a first slice: an MCP server with ~5 verbs tried by Ahmed from Claude Code on a live board.
2. **Headless core** — `varos-core` + CPU raster (`thumbs`) as a library/CLI/server; the Bridge runs without a window.
3. **File stores** — the `FileStore` seam (local first, then Google Drive), board diffs, change requests.
4. **Web mirror** — same code on WASM/WebGPU, opening from the cloud store.
5. **Chat box** inside the desktop app (a normal box in the box system) that drives the user's CLI through the Bridge.

In parallel, the editor keeps growing as a product (next big product gap: the **Text tool** with Arabic shaping — the moat).
