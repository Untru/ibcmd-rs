# Mobile signature compatibility evidence

The installed EDT 2025.2.3.30 reader mishandles the zero-count branch of
`MobileDigiSignResource.readDigestData`: for its fourth empty group it consumes
the outer digest-list terminator, then logs an unsupported-version diagnostic.
The genuine UH83 signature has version 2, empty key/digest, all four empty digest
groups and `converted=false`; its native source is 44 bytes (BOM and CRLF).
The same complete `ConvertedMobileClientSign` model serialized by the genuine
SDK resource is 35 bytes (no BOM, LF). The reader has the same defect on those
35 bytes. Version 0 is a different EObject class and is not a substitute.

For this complete empty model, the private adapter emits a
39-byte EDT carrier with four `{-1}` empty-count spellings. The installed reader
executes zero iterations without its duplicate zero-branch terminator read.
Standalone genuine `Resource.load` returns one `ConvertedMobileClientSign`;
`EcoreUtil.equals` against the complete expected EObject is true, including all
four digest groups and the false converted flag. Genuine `Resource.save` emits
the original canonical 35 bytes. The implementation uses complete token
recognition, rejects malformed/reserved negative carriers and does not join
separated integer tokens. The subsequent full version-2 framer below extends
this adaptation to witnessed partially empty groups and both boolean flags.
Unknown legacy signature bodies retain their verbatim contract.

The canonical private model uses those 35 bytes. A serde-skipped lexical facet
keeps the original native-compatible 44/35 spelling and emits it to native XML
only while its independently decoded complete model remains unchanged.
The carrier decodes to canonical native 35. Public unchanged native-to-EDT-to-
native conversion returns the complete original tree, including all 44 signature
bytes. Edited models cannot replay the facet or restore stale source through
forged generated-file hashes.

## Independent evidence and remaining strict failure

All experiments use disposable F-drive copies; original corpora and installed
SDK files are read-only. Heavy commands acquire the shared FIFO individually.

- `F:\ibcmd\lab\07\mobile-carrier-research-r1`: exact installed jars, SDK
  reader/model/serializer probes and raw stdout. The full model equality,
  canonical35 output and unequal version0 negative control are recorded.
- `F:\ibcmd\lab\07\mobile-carrier-headless-r1`: genuine installed import /
  validate and export commands succeeded, validation TSV is empty, project
  hashes remain unchanged. Exact empty-control83-r2 diagnostic comparison has
  no unmatched errors; raw ambient errors remain in the captured logs.
- **`F:\ibcmd\lab\07\mobile-carrier-native-r1` remains FAIL.** Installed EDT
  XML export copies the raw39 carrier. A fresh native SDK database imports it
  but native export fails with a stream-format error. This raw installed-export
  acceptance gate is neither waived nor reported as passing.
- `F:\ibcmd\lab\07\mobile-carrier-canonical-native-r1`: a separately
  identified synthetic input copies that tiny installed export and replaces
  only the signature with genuine `Resource.save` output from the same complete
  EObject. Fresh native create/import/export succeeds and preserves exact35.
  This establishes the model serializer/native chain, **not** the raw39 gate.
- `F:\ibcmd\lab\07\mobile-carrier-semantic-chain-r1.json`: scope and paths
  of both chains, including their differing outcomes.
- `crates/ibcmd-edt/tests/mobile_empty_signature.rs`: exact authentic EDT44
  native emission, complete public original return, stripped carrier decoding,
  stale lexical/model/hash negatives and full verbatim nonempty-body retention.

The single-file experiment does not establish successful raw installed XML
export of the carrier. The independent two-file gate below resolves that exact
empty-model case. Whole-UH acceptance remains a separate requirement for
universal conversion support.

## Nonempty model controls

Seven distinct genuine version-2 signature bodies from the installed D-drive
corpora were copied to `F:\ibcmd\lab\07\mobile-nonempty-model-research-r1`.
The isolated `bounded-512-model-r4` probes use the installed SDK with a minimal
resolved classpath, a 512 MiB heap, SerialGC and two processors. All seven
actual load/save/reload commands exit successfully and preserve the complete
EObject under `EcoreUtil.equals`, including all four nonempty digest groups,
key, digest and converted flag. Independent byte comparison finds only BOM and
CRLF serialization differences. `bound-proof.json` binds original unchanged
source hashes, tool/JAR hashes and every raw command/log/output.

Earlier 64 MiB probe failures are retained as laboratory resource failures;
they do not classify these configurations as invalid. This evidence verifies
those seven SDK resource models, not whole-project acceptance or an unknown
future signature schema.

## Official XML exporter filename precedence and independent native gate

The installed `MetadataXmlExporter.copySignatureFile` first copies the sibling
`MobileClientSignature.bin` for runtime versions above 8.3.19 and falls back to
`MobileClientSign.bin` only when the first file is absent. The model importer
supports the latter filename. Bound disassembly is recorded in
`mobile-carrier-research-r1/MetadataXmlExporter.javap.txt`.

The separate disposable `mobile-dual-name-headless-r1` project contains the
complete empty reader39 plus the model-equal native35 sibling. Actual installed
EDT 2025.2.3.30 validation produces an empty TSV; unmodified official XML export
copies exact35. The original project remains unchanged, and full captured
diagnostic records match the bound empty-project control without unmatched
errors. `bound-proof.json` binds both file inventories, exact tool and command
hashes, raw logs and control evidence.

`mobile-dual-name-native-r1` then creates a fresh disposable database with
native SDK 8.3.27.2214 and imports/exports that unmodified official export.
All commands succeed and native output preserves exact35.
`mobile-dual-name-headless-r1/bound-native-chain.json` binds the reference helper's
CAPTURED result, fresh-database identity, required commands, exact input/output
inventories and both signature hashes. This gate uses official raw export;
there is no post-export rewrite or SDK modification. The original single-file
raw39 failure above remains part of the evidence.

The production adapter emits both files for the completely decoded version-2
Converted model. Native-compatible framing is retained in the preferred
sibling only while its independently decoded full model matches. Both files
must agree on that model when read; carrier count atoms in the native sibling,
missing reader, mismatched flags, keys, digests or members are rejected. Unknown
legacy single-file bodies retain their verbatim contract. This scoped proof
does not establish whole-configuration UH success.

## Complete version-2 framing and partial groups

The private Rust framer decodes both quoted key/digest fields, exactly four
ordered digest groups, every member UUID/name and the boolean converted flag.
Declared counts must match actual members; `-1` is accepted only as the EDT
carrier spelling of an empty top-level group. It changes only those count
atoms, preserving nonempty member bytes and quoted data containing `{0}` or
`{-1}`. It does not search or replace nested negative numbers. Invalid known
version-2 frames fail closed, while unknown legacy bodies remain opaque.

`F:\ibcmd\lab\07\mobile-model-shape-probe-r1\partial-r4` records genuine SDK
full-EObject equality and exact native serialization for all four individually
populated group positions under both boolean flags, plus both complete empty
models. `quoted-r5` independently covers nonempty Cyrillic key/digest/member
text, escaped quotes, embedded CRLF and quoted count-like text. These are
standalone resource proofs; they are not installed whole-project or fresh
native database acceptance of every partial shape.

`mobile_signature_groups.rs` checks the same canonical serialization against
the official outputs and all seven genuine nonempty bodies. Public untouched
conversion restores every original byte; edited members, keys, UUIDs or flags
cannot replay lexical facets or forged provenance. The complete 11-test run,
including real fixtures, is captured in
`F:\ibcmd\lab\07\mobile-general-all-genuine-r1.log`; independent reruns are in
`mobile-general-adapter-independent-r1.log` and
`mobile-general-adapter-empty-independent-r1.log`.
