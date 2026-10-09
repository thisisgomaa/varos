# Text programme — 2026-10-09

Status: headless engine work authorized by Lane T; product milestones below remain owner-gated.
Authority: ADR-0010 Amendment 1, ADR-0012, PLAN Phase 14. The historical v4/v5
TextBox numbers are superseded by PLAN's next numbered bump (currently v14+).
This lane changes no persisted format and introduces no core → text dependency.

## Rules of record

Source: `TEXT_PRIOR_ART_KASHIDA.md`, `TEXT_PRIOR_ART_EVALUATION.md`, and the durable
BStudio `FIGMA_ARABIC_RULES.md`. Preserve logical Unicode source; never reverse
Arabic, store presentation forms, or inject direction controls as a workaround.
Arabic letter spacing is zero. Elongation uses HarfRust SAFE_TO_INSERT_TATWEEL,
joining pairs, priority and per-word limits; never split lam-alef, ligatures or
base/mark clusters. Off is the default; deliberate display use and explicitly
selected paragraph justification are separate from authored literal tatweel.
Headings need at least 130% leading; Arabic body defaults to 150–160%, expanded
when actual mark ink exceeds it. Arabic defaults right/centre; RTL content
layouts mirror icon/order placement (not an authorization to mirror the shell).
Use Arabic ؟ and ، in authored Arabic copy; never silently rewrite punctuation
or digits. Keep original text editable; transient outlines for rendering/export
are not destructive Create Outlines. Brand-specific Figma fonts are not engine
requirements. Fonts arrive as bytes, missing fixtures fail, no machine discovery.

## Ordered stages and exits

| Stage | Implementation contract | Headless exit | Owner exit |
|---|---|---|---|
| P2 TextBox + next bump | Core owns source, style spans, paragraph settings, frame kind, immutable font references; NodeKind indexes Document.text_boxes. Plain serializable data only, no COSMIC types/caches. Point/Area/Path discriminant; styles use source byte ranges, direction + affinity are runtime. Named migration, resource limits, old-reader refusal and unchanged plain-path fixtures. Core → text only at T4. | Save/reopen/migrate/refuse/undo; missing font snapshot; NonZero CPU/PDF/SVG parity; P1 performance and WASM runtime evidence. | Approve numbered schema work order, Arabic proof sheets and text interaction mockups before wiring. |
| P3 point text | Native text tool, baseline anchor, IME/preedit/copy/paste, source-aware selection and hit tests. | Latin/Arabic mixed edit commands, cancellation, undo and transaction tests; no GUI tests. | Type/edit at several zooms; IME, clipboard, tabs, recovery; readable handles. |
| P4 Arabic/RTL excellence | Greedy/every-line legal paragraph composer, opt-in kashida policies, split carets, mark-safe metrics. | Bundled-font ligatures/marks/mixed digits/Arabic punctuation corpus; exact source maps; width residual ≤0.5 px when feasible; explicit infeasibility otherwise. | Read real body/headline proofs at 12/48/200; approve priorities and spacing visually; native arrows/selection. |
| P5 area in any shape | Geometry supplies one or more intervals per baseline; exclusions and threaded frames retain one logical source. Widows/orphans constrain frame splits, not paragraph line breaking. | Concave/disjoint/empty intervals, overset, flow cycles, legal breaks, deterministic resize; cache equivalence. | Resize irregular frame, thread/unthread, see overset; no lost text. |
| P6 type on path | Arc-length placement, start/end offsets, side/flip and baseline offset; source order independent of path direction. | Zero-length/cusps/closed/reversed paths, RTL + combining marks, finite geometry and overflow. | Drag brackets, reverse path, edit mixed script without broken joins. |
| P7 styles/OpenType | Character/paragraph style inheritance, real weights, features, variable axes and join-safe tracking (Arabic zero). | Range splitting, style cycles/refusal, feature/axis cache keys and outline/shaping parity. | Review one Properties home and deliberate feature defaults; compare Arabic marks and real weights. |
| P8 PDF text | Font resources, licence-aware embedding/subsetting, ToUnicode source maps; outline fallback explicit. | Subset glyph closure, extraction/copy, marks, ligatures, RTL, transforms, native/deliverable parity. | Print and copy Arabic/Latin from external PDF viewers. |
| P9 Package fonts | Snapshot package with hashes, face/axis identity and licence decisions; no silent redistribution. | Deterministic manifest, missing/restricted/corrupt font refusal; reopen offline. | Move package to clean host and verify appearance. |
| P10 Create Outlines | Explicit undoable conversion preserving independent glyph NonZero fills, paint order and curves. | Holes, overlaps, opposite font winding, marks, transforms, undo/source recovery. | Zoom/print compare and undo back to editable text. |
| Import | Map supported SVG/PDF typography to editable source with explicit loss diagnostics; otherwise preserve appearance. | Round-trip supported subset, unsupported features, malicious input limits, no invented text. | Compare imported appearance and editability against source. |

## This lane and gates

Implement pure paragraph/justification/metrics/path contracts in varos-text first;
no model/format/UI integration. Every implementation slice gets focused headless
regressions; final combined gates: fmt, dependency direction, offline workspace
tests (-j 2, no-fail-fast), native/Windows clippy -D warnings, installed WASM
check, and exact vendor patch reproduction. Preserve the original P1 corpus and
record timing separately for ordinary/incremental layout and new composition.
Greedy remains the incremental cache's existing policy until composed cache keys
and convergence get their own oracle tests. Every-line work must be bounded and
resource refusal explicit. Hyphenation is a provider seam, no heuristic dictionary.
No headless result substitutes for owner visual acceptance or WASM runtime parity.
