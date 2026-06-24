# Varos — Fill & Stroke (build notes)

The next step after shapes. Right now every path renders in ONE hardcoded accent colour (the CSS `.pathline` class). This task gives each object its own **Fill** and **Stroke** so drawings can actually be coloured. First read `ILLUSTRATOR_TOOLS_CATALOG.md` for the full picture and the build order — Fill/Stroke is step 2.

## The model (the foundation — get this shape right)
Every path/shape stores TWO paint attributes ON the object itself (not in CSS):
- **fill**: a colour (hex) OR none.
- **stroke**: a colour (hex) + width (number) OR none.

Render each path using its OWN fill/stroke, not a shared class. *(This is also where the "an object has editable properties beyond geometry" pattern begins — the seed of the future inspector/single-schema idea. Keep it clean and generic.)*

## Toolbar controls (Illustrator model)
- A **Fill swatch** + a **Stroke swatch** (two squares); one is "in focus" at a time.
- **X** = toggle focus · **Shift+X** = swap fill/stroke · **D** = default (white fill, 1px black stroke) · **/** = set the focused one to None.
- A small **colour picker** (hex + RGB; a native `<input type="color">` + hex field is fine for now) that sets the focused attribute's colour.
- A **stroke-weight** number input for thickness.

## Behavior
- With object(s) selected (V or A), changing fill / stroke / weight updates the selected object(s) live, as ONE undo step.
- New shapes drawn afterward pick up the current fill/stroke (a "current paint" state).
- None fill = transparent interior (objects behind show through); None stroke = no outline.

## Keep scope tight (now)
- **Flat colours only.** Gradient, pattern, dashes, caps/joins, stroke alignment = **LATER**. Just colour + width now.

## Close follow-on (optional, if quick)
- **Eyedropper (I)** — click an object to copy its fill/stroke onto the selected one. Cheap, high value.

## ★ Pattern note
This establishes how an object stores + edits a NON-geometry property. Make adding the *next* property later (opacity, etc.) a copy of this pattern — that "easy to add a property/tool" is Varos's core strength. Don't hardcode one-offs.

## Working rules (unchanged)
Hand Ahmed one small piece at a time; he verifies in his real browser; simple short Arabic; if he says rebuild, do it the first time.
