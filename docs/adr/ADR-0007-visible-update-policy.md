> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# ADR-0007: Visible, user-controlled updates

- **Date:** 2026-07-11
- **Decision owner:** Product owner
- **Supersedes:** The silent background-update promise in `docs/VAROS_CONSTITUTION.md:14`
- **Superseded by:** None

## Context

The old constitution requires silent, background, incremental auto-update (`docs/VAROS_CONSTITUTION.md:14`). The product owner rejected that promise and chose visible, user-controlled updates (`docs/foundation/FOUNDATION_CHARTER.md:86`, `docs/foundation/FOUNDATION_CHARTER.md:121`). The foundation audit also records that no updater or release channel currently exists (`docs/audits/2026-07-11-CODEX-FULL-PROJECT-AUDIT.md:241-242`).

## Decision

Varos updates must be visible to the user and remain under user control. Varos will not silently install background updates.

This ADR selects policy only. It does not select an update service, transport, installer technology, release channel, notification cadence, or offline package format; those require an implementation proposal and review when distribution work begins.

## Consequences

- Current documentation must stop promising silent incremental auto-update.
- A future updater must expose what is happening and require an explicit user-controlled update path.
- Security and compatibility messaging cannot depend on an invisible forced-update mechanism.
- No updater dependency or network behavior is added by this policy ADR.

## Status

Accepted — product owner, 2026-07-11.
