# Varos UI handoff — 2026-10-27

## Status and source of truth

The user explicitly approved the detailed HTML interface with: “Excellent, this is what is required” (English translation). Preserve this direction. This package is a dated snapshot for Claude Code, not a new design proposal.

1. **Approved interface:** [01-approved-html/index.html](01-approved-html/index.html). This is a byte-for-byte copy of the approved standalone HTML. Open it directly in a browser; no installation or build is needed to use the prototype.
2. **Agreed rules and architecture:** [02-architecture/AGREED_ARCHITECTURE.md](02-architecture/AGREED_ARCHITECTURE.md).
3. **Observed implementation/tokens:** [02-architecture/IMPLEMENTED_DESIGN.md](02-architecture/IMPLEMENTED_DESIGN.md).
4. **Blender references:** [03-references/BLENDER_INDEX.md](03-references/BLENDER_INDEX.md), plus the modern visual reference in `03-references/visual-direction/`.
5. **Earlier studies:** `04-earlier-studies/`. Historical material, not the approved visual baseline.
6. **Evidence and tests:** [05-verification/README.md](05-verification/README.md).

## Instruction for Claude Code

Read this file and the agreed architecture before modifying anything. Use the approved HTML as the visual and interaction baseline. Adapt the real Varos tools to this system while preserving independent areas, arbitrary divider icon rails, the independent Properties editor, and the small hidden top-edge grip. Do not reintroduce a fixed global sidebar or a permanent generic panel drag titlebar. Product labels and tooltips remain English.

Inspect the actual current Varos checkout and its local instructions before implementation. This handoff does not establish a production branch, installed build, or merged application change. The HTML is a prototype and should guide the native application rather than imply that the rendering engine has been replaced.

The agreed contract takes precedence over earlier Blender/Figma study variants. The HTML governs the approved appearance. Where a desired behavior exceeds the sample, use the status table in the architecture document rather than assuming the sample is a complete implementation.

## What is included and excluded

Included: the approved self-contained HTML, architectural decisions, original user-provided Blender references, modern style reference, earlier Figma study exports/authoring sources, current native panel inventory, 27 modeled interaction checks, and SHA-256 file manifest.

Excluded: unrelated poster/design-production documents, credentials/session tokens, installed application binaries, third-party Blender source code, and browser-local edits/layout changes made after the HTML was loaded. The user browser's localStorage was not extracted. The HTML source contains its default layouts and demo document; runtime state can be exported through the prototype's “Save workspace snapshot” command.

No upload, email, external message, or production code edit was performed. The folder and ZIP are local artifacts.
