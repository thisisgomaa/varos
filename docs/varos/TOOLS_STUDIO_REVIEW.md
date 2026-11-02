# Tools header and Studio workspace review

Status: 2026-11-01 — implementation candidate; owner review pending.

Scope: update the same Varos Panels Preview 2 app, bundle build 20261101.7. Remove the redundant Tools name tab and hamburger in both floating widths, docked Tools, and mixed column layouts. Keep the dark top grip and existing width arrows.

Studio is a built-in starter workspace: Tools left (80 logical px), Control top, Properties above Layers right (320 px), and a 44 px adjacent icon strip with Color, Swatches, Gradient, Stroke and Appearance as separate groups. Window > Workspaces selects Studio; Reset restores its baseline. Fresh preview profiles start with Studio; existing saved arrangements are retained. This delivery selects Studio once after saving the prior layout as a named workspace.

Engineering checks: focused panel model/pointer tests, UI/app clippy, native build/signature and startup screenshot. Exact outcomes in dist/tools-studio-2026-11-01/verification.json. Native interaction acceptance belongs to the owner; no claim of full platform CI or final release.

Review steps: confirm Tools has no redundant second row when detached or docked; toggle its two widths; inspect the initial Studio arrangement; switch to the saved previous layout and back. Remaining backlog proceeds one owner-reviewed slice at a time.
