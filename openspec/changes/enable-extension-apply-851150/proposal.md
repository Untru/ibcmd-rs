# Enable extension load and activation on 8.5.1.1150

## Why

Issue #578 requests the missing extension write path on the exact platform build 8.5.1.1150. Export parity does not establish that XML edits can be staged and made active. Existing extension staging/publication is admitted only on 8.3.27.2214, and the drop-in import/apply rejects extensions.

## What changes

- Measure native extension staging/publication and derived storage on disposable 8.5.1.1150 SQL twins.
- Admit only the exact measured 8.5 profile and implement its own XML staging and activation, including the standard import/apply extension CLI.
- Keep main configuration and unrelated extensions unchanged, preserve optimistic predicates and transactional rollback, and reject unsupported schema changes before publication.
- Verify synthetic extension edits and a versioned ServiceDesk copy through independent native re-export. No production runtime dependency on ibcmd is introduced.

Main configuration apply, arbitrary platform builds and LIVE/worker activation are outside this change.

The standard CLI also admits exact 8.3.27.2214 after repeating the same independent native twin control, including storage format 80327 and registry Version. No other 8.3.27 build is inferred from that result.
