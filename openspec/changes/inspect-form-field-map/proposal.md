# Inspect a Form field map without disclosing source values

## Why

The six-file UT export remainder includes two dynamic-list UseAlways groups.
Sanitized XML proves an extra marker but cannot recover serialized field-map IDs,
required-ID membership or final-query selected aliases. Inferring the cause from
that marker would risk an incorrect production fix (Refs #381).

## What changes

- Add a bounded local-file diagnostic example using the existing Form parser,
  field-map parsers and selected-query resolver; no SQL/network/write operation.
- Report only fixed boolean/count fields and stable anonymous ordinals for
  canonical public DeletionMark candidates. Unknowns remain unknown.
- Keep raw bodies, names, IDs, queries and error payloads out of stdout/stderr.
- Preserve the ordinary export path with no observer and unchanged semantics.
