# Maintained morph1c adaptations

This document records the exact local patch scope approved by the maintainer for [PR #522](https://github.com/Untru/ibcmd-rs/pull/522). It is a historical approval record, not permission for future vendor changes.

## Approved scope

- Source baseline: `1688c73b9bef00df31aa87aa0f259a1133fc3624`.
- Reviewed source snapshot: `e2ef236ade9b5e2a7d291ab39b241c036904a4be`.
- Retain exactly 19 Rust paths (18 changed and one added), their 19 corresponding `LOCAL-FILES.json` records and the seven adaptation notes below.
- The other 659 manifest records are unchanged. Inherited manifest mismatches and unlisted files are outside this approval; no blanket refresh or cleanup is authorized. Five of the changed source paths already had inconsistent manifest hashes in the baseline; these are part of the same 19 paths, not additional approved changes.
- No production source or manifest content changes are made by this policy record. Upstream provenance remains in the pinned vendor metadata.

The source snapshot and hashes define the approved behavior. Any new path or semantic change requires fresh explicit maintainer approval, semantic review, positive and rejection tests, and updates confined to the corresponding manifest records and adaptation notes. Passing tests or an existing adaptation alone do not grant approval.

## Source files

Paths are relative to `crates/ibcmd-edt/vendor/morph1c`.

| Path | Approved SHA-256 | Source PRs |
| --- | --- | --- |
| crates/core/src/ir/form/body.rs | 771c0abb0265aaacc1e49ea8a4bf9363514bd3269815904461c7b70d669f36e9 | #504, #507 |
| crates/core/src/ir/form/dcs.rs | c3d6af468525fedeb9fcd6f80ea53d68003d9de8e81f8bbb81ac086af1f7f466 | #504, #507 |
| crates/core/src/ir/mod.rs | 9f9fffbab06af9cdcac43127bdd49a93005cee340a4f4766f5ef101b678e94ba | #504, #507, #514 |
| crates/formats-xml/src/form/dcss/form_settings.rs | 4c4a493e51dd3e123e3d34bbfb150029dd9cf65f767778adea2d663e332e06c6 | #504, #507 |
| crates/formats-xml/src/form/dcss/list_settings.rs | bd0cd4258d74f1f2f657fae3308d33fbc4604c89875c5b08c6413f9a323e924f | #504, #507 |
| crates/formats-xml/src/form/mod.rs | b911e89522fb6bba311175a02c14cdc46ec20a772d6e29a3e7c07884ef122eec | #504, #507 |
| crates/formats-xml/src/form/read/dcs_settings.rs | 04dbaf6dc84020ba0b70cc47828532b3f8e46584c74b5b0ae62dafc6b585df7c | #504, #507 |
| crates/formats-xml/src/form/read/mod.rs | 7d24fe63f0b24c0c8081463e9e9adf743af8011100546069d8965456edb0caaf | #504, #507 |
| crates/formats-xml/src/form/write/designer_dcs.rs | bf4b316fbbb4f5940f7493bc0bcc08ef36e825b17aa06e49898e137f915ac850 | #504, #507 |
| crates/formats-xml/src/form/write/edt.rs | a3e80128a8b7ec70c9c02c0091d80f7e965f17064d270cdb9b228e343351c899 | #504, #507 |
| crates/pipeline/src/config_interface_read.rs | 2356c76be3aa27eecb65f7d3903c070924250f7f715ce824f92ef0584d1653a1 | #504 |
| crates/pipeline/src/dcs_read.rs | 8d3470d1a71f2f6a3efade855bfa2ab11280f66e0f160d0f91bcb6c3ed5700c5 | #504, #507 |
| crates/pipeline/src/ext_read.rs | e4d2333feb9d209646d4efcd7a865ff17dc1331bcace7f9d9eb61281d981a0ba | #514 |
| crates/pipeline/src/ext_read/parent_distribution.rs | 531692403564616ad480953a7cb25ba541f2e92db142d48d4502ee43b274b762 | #514 |
| crates/pipeline/src/form_read.rs | c18ab9b37f943992349c566c7759ecda1bb02742e92616e285ee23fda7dbde53 | #504, #507 |
| crates/pipeline/src/form_write.rs | 8968fe171bc5073fb4a036c7f836e831e369507de0f85a083b7d18f6f0ed1ab2 | #504, #507 |
| crates/pipeline/src/lib.rs | 461d5441ee463e1b0073cc8ec6fce7b8edbb8f88760c1c1435328d6465e4d81e | #504, #507 |
| crates/pipeline/src/module_read.rs | 5731a29355a9a4ef6e707112cc8e81c50ee3a77e20472be580c99acd390b7abf | #504 |
| crates/pipeline/src/sdk_body_projection.rs | 0ac643feea8c1ee9cfff2008519508e32fad4d24a89815580b2d5e4c7a1b4fa9 | #504, #507 |

## Existing adaptation notes

- EDT 2026.1 Configuration.distr reads as the root ParentConfigurations.bin in the .distr item order and is written back as .distr; only the witnessed supplier shapes, canonical UUID identifiers and item modes are accepted
- Witnessed client-interface direct panel versus group shape and ordered declared panel subsets are typed; exact installed EDT LF framing is retained separately from semantic data, with full byte guards and legacy CRLF behavior preserved
- Witnessed current EDT LF form framing is a source-only lexical facet; declaration-bound verbatim text retains explicit carriage-return edits while legacy CRLF defaults and native envelope validation remain unchanged
- Witnessed current EDT ListSettings.dcss LF framing is a source-only lexical facet with verbatim typed text; legacy CRLF emission, semantic emptiness and native empty ListSettings self-closing spelling remain unchanged
- Witnessed current EDT ConditionalAppearance.dcssca LF framing uses an explicit framed API and a source-only FormBody facet; legacy CRLF API/defaults, complete ordered typed accounting and empty-sidecar omission remain unchanged
- Source-bound pure-LF empty Russian form-module region scaffolds use native CRLF framing only after complete current syntax revalidation; generic BSL text, current raw semantic identity, same-format bytes, provenance and unknown/executable text remain unchanged
- Witnessed current EDT DCS element-only structural LF becomes canonical CRLF with a source-only owner facet restoring same-EDT output; mixed content, CDATA, xml:space scopes and leaf values remain exact and mixed eligible source separators fail closed

## Verification and limits

The retained source changes and corresponding manifest hashes were reviewed independently. Their positive/rejection cases are covered by the existing workspace suites and the integration gates. This records permission for the code location and exact patch scope; it does not establish new native acceptance or authorize data-bearing fixtures. The general no-edit rule remains in force outside this scope.
