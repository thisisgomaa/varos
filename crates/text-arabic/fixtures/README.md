# Test fixtures (tests only, never compiled into the app)

`NotoSansArabic.ttf` is the unchanged OFL font that varos-text ships as a test
fixture (`varos/crates/varos-text/assets/fonts`, originally from cosmic-text 0.19.0's
`fonts/`). SHA-256 `ee489b994b3e62def9874c918145e32b133b625abaf98cec60502bdb40102c56`.
Licence: `NotoSans-LICENSE` (SIL Open Font License 1.1). The kashida tests in
`crates/text` include it with `include_bytes!`; collection (TTC) fixtures are
generated from it at test time, no other font file is committed.
