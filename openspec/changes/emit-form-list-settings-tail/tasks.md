# Form ListSettings typed-tail implementation

## Task 1: Add bounded writer-evidence schema

**Status:** `[x]`

- [x] Parse the committed DCS writer-evidence JSON with strict bounds.
- [x] Reject unknown, duplicate, oversized, unverified, or drifted facts.
- [x] Expose only the verified Form ListSettings tail policy.

## Task 2: Add typed-tail XML emission

**Status:** `[x]`

- [x] Emit the two fields in verified order with XML escaping.
- [x] Omit `QuickAccess`, empty strings, and absent values.
- [x] Keep the full DCS preflight blocked on exactly four missing facts.

## Task 3: Delegate the Form tail

**Status:** `[x]`

- [x] Replace only the two manual tail branches in `form_body`.
- [x] Preserve wrapper, prefix, indentation, and complex section bytes.

## Task 4: Validate

**Status:** `[x]`

- [x] Add schema, XML, and Form regression tests.
- [x] Run focused tests, formatting, validators, and strict OpenSpec validation.

## Task 5: Apply independent P2 corrections

**Status:** `[x]`

- [x] Exact-join the writer constant and lexical enum default across verified corpora.
- [x] Restore context-sensitive full-preflight missing-fact diagnostics.
- [x] Reject forbidden XML 1.0 characters and invalid or reserved prefixes.
- [x] Add focused mutation, context, character, and prefix tests.
