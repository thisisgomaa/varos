# Varos Roadmap

Varos is a free, open-source vector editor built for AI from the ground up, not one with AI
added on. It is made for specialized studio work, packaging and print production among it, and
Arabic and right-to-left type are part of its core.

AI in Varos is a co-worker on the file: agents read, edit and save the document you have open,
you watch their edits happen, and you undo them like your own. They run on your own model and
key, with no subscription.

It is built on [VectorCraft](https://github.com/storytold/vectorcraft) by the ArtCraft team,
whose engine gives Varos its drawing, paint, formats and print. Varos goes its own way above
that engine.

## What Varos adds today

- **Agents built in:** any agent running as you on your Mac can read, edit and save the open
  document with no pairing step. Its edits appear live on the canvas, tagged in History, and
  undo reverts them.
- **Arabic typography:** its own kashida engine (Off / Auto / Force with a strength control,
  AAT fonts included), coloured harakat, an Arabic-first font menu, a partial Arabic interface translation (help wanted).
- **Its own interface:** one design system and a panel workspace that docks, floats and
  collapses.
- **Its own file format:** `.varos`, with independent pages.

## Where Varos is heading

An AI panel for chat and images on your own key, specialized studio work in packaging
(dielines, 3D box preview, box templates) and print production (preflight, print rules, press
conditions), working together on one file, and extensions. Details below.

No dates are promised here. Order shows priority.

## Where we are: 0.1 Preview (pre-alpha)

- macOS on Apple Silicon only, ad-hoc signed, no auto-update.
- Arabic today: shaping and bidi, automatic kashida (Off / Auto / Force with a strength
  control, AAT fonts included), coloured harakat, an Arabic-first font menu, a partial Arabic interface translation.
- Known gaps are listed in [KNOWN_ISSUES.md](KNOWN_ISSUES.md).

## Now: toward 0.2

1. **Stability of the new interface.** Panels, docking, floating, Preferences and the colour
   picker checked in a real window, not only in tests.
2. **Catch up with VectorCraft.** Bring in upstream fixes in stages, keeping Varos changes small
   and clearly marked.
3. **CI on every pull request,** so contributors see their checks pass.

## Next: Arabic typography depth

- Kashida-aware line breaking (a Middle Eastern paragraph composer).
- Stylistic alternates (`jalt`).
- Language tags on text (`ar`, `fa`, `ur`), used by shaping and by PDF export.
- Harakat: vertical adjustment and custom colours from the interface.
- Correct mixed-selection state in the Direction and harakat controls.
- Arabic PDF that copies, searches and re-imports correctly in other readers.
- Completing the Arabic interface translation and reviewing it with native speakers; Persian, Urdu and Hebrew interfaces.

## Planned features

These are already visible in the preview, marked **Soon**. Hover one to read what it will do.
None of them works yet.

**AI, with your own model and key.** No subscription; your files stay yours.
- Ask: chat with Codex or Claude Code, or with your own API key; project memory; recipes and
  quick scripts; agents on your own server; build an extension with AI.
- Images: generate, several versions, styles, find references; edit with a prompt, expand,
  remove an object or the background, upscale, adjustments, auto merge.
- History: a record of every AI run, and undo for a whole run.

**Packaging.** Make a dieline, preview the box in 3D, start from box templates.

**Print production.** Preflight, print rules in Document Setup, and choosing a press condition
for CMYK.

**Working together.** Share, comments, "Shared with me", and joining a file with a code.

**Extensions.** An extension library.

## Later

- Preview builds for Windows and Linux.
- Faster rendering for large documents (tile cache first, then a GPU decision).
- Signed and notarized macOS builds with updates.

## How to help

- Start with issues labelled
  [good first issue](https://github.com/thisisgomaa/varos/labels/good%20first%20issue).
- Arabic, Persian, Urdu or Hebrew speakers: interface translations and type review are the most
  valuable help right now. Look for the [i18n](https://github.com/thisisgomaa/varos/labels/i18n)
  label.
- Designers: try the preview and report what breaks, in English or Arabic.
- Engine, format and performance fixes are best sent upstream to VectorCraft.

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. Questions and ideas go to
[Discussions](https://github.com/thisisgomaa/varos/discussions).

The VectorCraft roadmap this fork tracks is kept in
[docs/UPSTREAM_ROADMAP.md](docs/UPSTREAM_ROADMAP.md).
