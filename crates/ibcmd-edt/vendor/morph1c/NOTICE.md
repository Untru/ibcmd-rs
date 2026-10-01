# morph1c source-only codec snapshot

Original project: [Segate-ekb/morph1c](https://github.com/Segate-ekb/morph1c).
Pinned revision: `962eedddd14493a914bae11f11667951de0f23a7` (2026-08-12).
Original contributors retain their rights in the original source.

The upstream workspace/package manifests declare **MIT OR Apache-2.0**.
This distribution chooses the Apache-2.0 alternative and includes its standard
license text in `LICENSE-APACHE`. The pinned upstream revision contains no
separate license/notice files; the declaration is retained in each package's
manifest. This notice attributes the source and does not invent a copyright
assignment.

`UPSTREAM-FILES.json` records original source/data paths and SHA-256 values.
`LOCAL-FILES.json` records the adapted, distributed files. Changes are documented
in `../../../../docs/edt/morph1c-adoption.md` (from the repository root:
`docs/edt/morph1c-adoption.md`).

Only the metadata specs/engine/private codec IR, shared XML syntax/projections,
EDT and Designer XML connectors, and source-family descriptor/body orchestration
are included. No CF/brace/container backend, upstream executable, SQL client,
Java/EDT invocations, external tooling, testkit or research corpus is a production
dependency. Original unit-test text may remain adjacent to reused code but is
disabled in this private snapshot; the host adapter's boundary tests are separate.

Local modifications include source-only orchestration, strict error propagation,
bounded host validation, four-worker read cap, source lexical provenance and
host packaging/lint compatibility. The snapshot is not an unmodified upstream
distribution and does not imply that upstream acceptance proves host acceptance.
