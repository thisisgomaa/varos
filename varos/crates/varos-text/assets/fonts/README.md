# Deterministic text fixtures

These are unchanged repository font bytes, not host-installed fonts. Inter and
IBM Plex Sans Arabic Regular came from varos-app/assets/fonts; Noto Sans Arabic
came from vendor/cosmic-text/fonts (0.19.0). Adjacent OFL notices apply. Compile-time
include_bytes! intentionally fails if a fixture is missing. SHA256SUMS pins bytes.
Noto's fixture lacks Latin letters: the mixed `v2،` joining/coverage segment needs
Plex as a declared fallback (Inter lacks the Arabic comma). Tests record/use that
fallback explicitly; they never pretend Noto covers the whole mixed segment.
