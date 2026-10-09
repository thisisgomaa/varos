# ADR-0017 — Explicit colour management (FORMAT v12)
Status: proposed
Date: 2026-10-09

Colour is `Rgb`, `Cmyk { c, m, y, k }`, `Gray`, or `Spot { name, tint, alt: Cmyk }`.
Existing Rgba and untagged solid JSON remain compatible. A tagged managed paint retains
source colour and alpha; its explicit screen conversion supplies sRGB to existing readers.
Swatches retain managed paints, including global and spot identity. Document colour_mode
is Rgb (omitted default) or Cmyk; selecting a mode does not silently rewrite artwork.
ICC bytes are bounded, validated with offline moxcms, and owned by the document.
Conversions take profiles explicitly: no global or thread-local working space.
Unprofiled CMYK uses the documented naive formula RGB=(1-CMY)*(1-K); this is not a
press simulation. Every export conversion carries a report note. Profile conversions
must fail visibly if the supplied profile is unsupported; no silent naive fallback.

PDF retains DeviceCMYK, DeviceGray and Separation tint functions with CMYK alternates.
Profiled process colours use ICCBased and the output profile supplies OutputIntent.
PDF/X-4 requires an actual output profile and validation of its conformance prerequisites;
the initial preset refuses images and page-info fonts until their preflight is implemented.
Its vector structural checks are headless; external PDF/X conformance remains unverified.
RGB-only content follows the existing writer byte-for-byte (apart from native v12 stamp).

v11→v12 is a named pure identity migration: missing colour fields decode to RGB defaults.
This isolated lane bridges the reserved v10/v11 eras with identity steps; the integrator
rechains other lanes. New keys/managed paint tags in pre-v12 bodies fail closed.
Frozen fixtures cover authored colours and refusals, with old RGB bodies unchanged.

New Document, Document Setup, picker CMYK/Spot sliders, PDF preset, Proof Colours and
Overprint Preview use existing kit controls in new modules. UI is provisional, owner
review pending. Proof/overprint must describe their approximation until a calibrated
separation compositing implementation is validated. Commands, Bridge 1.2 progressive
schema and CLI share validation and headless tests; legacy Bridge fixtures stay frozen.
