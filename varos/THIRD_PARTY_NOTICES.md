> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Third-Party Notices

Varos uses the following third-party assets and macOS cursor bindings.

The tool cursors are the Varos cursor set v1 (`crates/varos-app/assets/cursors/v1/`), drawn for
this project — no third-party artwork. (Until 2026-09-23 the Pen cursor was derived from Font
Awesome Free's `pen-nib` icon, CC BY 4.0; that glyph is no longer shipped.)

## Lucide (icons)

UI panel icons (tool rail, control bar) are from **Lucide**.

- Source: https://lucide.dev — https://github.com/lucide-icons/lucide
- License: **ISC**

## objc2 (macOS cursor bindings)

The Retina cursor implementation uses these direct, macOS-only Rust dependencies:

- **objc2 0.6.4** — **MIT**.
- **objc2-foundation 0.3.2** — **MIT**.
- **objc2-app-kit 0.3.2** — **Zlib OR Apache-2.0 OR MIT**.
- Source: https://github.com/madsmtm/objc2

Versions and licenses checked against the resolved crates' Cargo.toml files. These crates
provide the Objective-C, Foundation, and AppKit bindings used to construct the 32-point
NSImage, its 32/64-pixel bitmap representations, and the NSCursor.
