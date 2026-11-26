# Witnessed current EDT DCSS framing

The authentic current EDT `2026.2.1.3rc0` NativeControl capture from the public
DcsFilterEvidence configuration contains an LF-only `ListSettings.dcss`:
1,869 bytes, SHA-256
`ba5415251a911a56c85350f53a8b8d82938852f77a758c7e0908af7e35995d90`.
The fixed reverse candidate failed before publication because the DCSS reader
required CRLF. The original failed report and immutable source remain retained.

The DCSS reader now captures declaration-bound LF framing and uses the existing
verbatim-text policy for that source. Its writer retains LF for same-dialect
emission. New settings and legacy sources keep CRLF. The lexical facet is
excluded from semantic serialization and cannot create settings or a sidecar.
Native empty settings still use `<ListSettings/>` after clearing LF-origin
values; the prior namespace-presence behavior is unchanged. BOM, unknown cells
and unsupported declarations remain errors. No generic whitespace normalization
or raw source replay was introduced.

Three in-memory DCSS regressions and two existing form-framing regressions pass
with Rust 1.95. The DCSS controls cover current LF exact emission, legacy bytes,
explicit CRLF typed edits, serde exclusion, negative inputs and complete native
empty-setting output in XML 2.20/2.21.

A standalone offline probe reads and regenerates both the authentic 1,869-byte
LF sidecar and its generated legacy 1,911-byte CRLF counterpart byte for byte.
Both have the same typed semantic SHA-256
`81aca570f599365ee5e3783251dd0e853635cc4e7d20bf71a2752d6ceb04d568`.
Both inputs are unchanged. This fixes one independently reproduced boundary;
the full reverse/current-native rerun belongs to #356. It does not establish
full BSP/UH, current schema or installed-EDT acceptance, and does not rename any
legacy profile.
