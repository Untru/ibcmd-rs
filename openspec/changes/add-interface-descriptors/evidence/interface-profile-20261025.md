# Interface metadata profile — 2026-10-25

Refs #558, #544, #525.

## Independent provenance

The tool1cd repository is research documentation only; no code, serialization tree, private native output or dependency is imported. Inspected immutable revision: [7c4b91cb7a7bb48a32be00b9f32afe740dada6e1](https://github.com/jjzhang166/tool1cd/tree/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1).

- [Interfaces collection identity](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L20339), [collection serialization](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L53946).
- [Interface descriptor serialization](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L55949).
- [Metadata UUID registration](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaData.cpp#L7495).

These are hypotheses checked against native controls, not evidence of modern behavior on their own.

## Measured native descriptor

Native controls: Windows 8.3.27.2214 and 8.5.1.1150, original supplied interface set and own edits to synonyms, comments and subsystem membership. The 8.3 input archive was built on 8.3.27.2342; the actual local reader/compiler is 8.3.27.2214.

| Coordinate | Measured meaning |
| --- | --- |
| Configuration family | Interfaces / Interface, class 39bddf6a-0c3c-452b-921c-d99cfa1c2f1b |
| XML order | after CommonPicture, before SessionParameter; Bot precedes PaletteColor on 8.5.1 |
| Outer descriptor | three members: marker 1, payload, zero child-collection count |
| Payload | revision 7, inverted Switchable flag, standard metadata header, marker 1 |
| Switchable | stored 0 → XML true; stored 1 → XML false |
| Header | UUID, Name, localized Synonym, Comment; default identity/tail members must match |
| Text | a stored CR-CR-LF in an Interface comment exports as a literal XML CRLF, not a CR entity followed by LF |

A native source control containing two synonym languages, escaped XML text and a multiline comment compiled/applied/exported successfully on both platforms. A separate native source-to-CF control with a Subsystem.Content reference to Interface compiled and exported successfully on both platforms. Membership is carried by the subsystem reference: the Interface descriptor profile is unchanged, and no fictitious Subsystems property is added to its XML.

Unsupported header members, descriptor revisions, flag codes, child collections, XML properties, namespaces and ambiguous/missing required values are refused. This descriptor evidence does not establish support for arbitrary populated interface menu/body payloads or older unmeasured descriptor revisions.

## Regressions

On base 12a81c0529, the new positive tests failed with `not yet: no row decoder for Interface` and `Interface must be in root inventory` (2 failed, 0 passed).

Latest narrow checks before the final native candidate:

- `cargo test --locked -p ibcmd-rs --test interface_metadata --no-default-features`: 5 passed. Exercises both dialects and Switchable values, native multiline text, exact synthetic CF output, unknown profiles, source admission and default CLI refusal.
- `cargo test --locked -p ibcmd-rs --lib interface_root_tests --no-default-features`: 2 passed. Exercises actual production root classification, a missing/class-declared future Interface record, and refusal instead of silently dropping the root inventory.
- `npx --yes @fission-ai/openspec@1.6.0 validate add-interface-descriptors --strict --no-interactive`: passed.

## Native CF / SQL measurements

Measured candidate: implementation revision 3272b0898e61facbcaafb7e9edae576e45c9057c, iter binary SHA-256 `490233f9633e90f28ff3838520751a2b00eba1deb457b8877bebcb5ec2fa53bd`. Later integration with master changes no production code in the Interface diff. Counts below are raw file-byte comparisons, without XML normalization.

| Route | 8.3.27.2214 | 8.5.1.1150 |
| --- | --- | --- |
| Interface-set CF → XML | 5/5 exact | 5/5 exact |
| CF with Subsystem.Content → Interface | 6/6 exact | 6/6 exact |
| SQL → XML, valid native reference | 6/6 exact | 6/6 exact (full-CF control below) |
| XML → SQL (`--verify --base-free`) → native apply → native XML | 5/6 exact; only ConfigDumpInfo.xml differs | 5/6 exact; only ConfigDumpInfo.xml differs |
| Native and own SQL readers after the same import/apply | 6/6 exact | 6/6 exact |
| Full combined CF: Configuration.xml | byte-exact, 337 ChildObjects | byte-exact, 338 ChildObjects |
| Full combined CF: Interface descriptors | all byte-exact | all byte-exact |
| Full combined CF: entire tree | 314 exact; 26 differing/missing paths; 340 native / 332 own files | 315 exact; 26 differing/missing paths; 341 native / 333 own files |

The reverse-route ConfigDumpInfo.xml difference is retained in the raw comparison: native import/apply changes generation versions. It is not removed from the exact-file counts. The two readers of the same resulting SQL database agree on this file too. Native apply validates the written configuration; this is not evidence for the own apply implementation.

The initial 8.5 SQL source-import/apply control returned exit 0 but re-export omitted the added subsystem and its stored version inventory was incomplete. The candidate refused that database with a manifest mismatch. This failed attempt is retained, not counted as a successful six-file control. An independent native source-to-CF control actually produced the six-file subsystem configuration. Loading that full CF with native into the disposable SQL reference database, applying it and exporting it established the valid six-file reference used in the table. The own export and subsequent base-free SQL round trip then passed against this reference. This does not establish why the initial native XML import produced an incomplete inventory.

SQL transport: installed sqlcmd/bcp 17 for reads and 18 for writes. bcp 17 rejected the write transport option `-u`; that failed attempt is retained. Model verification stayed enabled. The initial empty-database import proof uses the previous candidate; the final candidate recompiles every source with `--base-free` on the same disposable target, after confirming no pending ConfigSave rows.

Real CF reports retain three technical opaque entries (root/version/versions); the real controls do not use `--fail-on-opaque`. The synthetic descriptor-only archive exercises the strict option. The combined results above do not claim complete configuration parity: 26 paths still differ on each platform outside this Interface/root-inventory correction.

Private manifests retain exact argv, executable SHA-256, process result, both output EOFs and per-file hashes. Native archives, source trees, raw storage and machine paths are intentionally not published.

## Complete verification

The complete local `cargo xtask ci --keep-going` passed all 21 steps on the merged branch: format, governance and physical-policy guards/self-tests, strict OpenSpecs, workspace clippy, six Python suites, schema tests, workspace crates, root integration suites and the full root library. Workspace harnesses: 1,306 passed / 0 failed / 81 ignored (including doc tests); root integration harnesses: 498 passed / 0 failed / 4 ignored, plus the process-boundary scenario runners; root library: 4,191 passed / 0 failed / 16 ignored. Ignored counts are existing suite classifications; no test was newly ignored. The new OpenSpec also passed its explicit strict validation.

An initial full test run exposed outdated collection classifications and property fixtures that omitted the populated child inventory. Tests now admit Interface as a root family, provide the fixture's complete child references when testing root properties, and require a failed root export for unresolved inventories. Existing property-content assertions remain in place.

On Windows the Python oracle suite additionally needs `PYTHONUTF8=1` when TEMP contains non-ASCII characters: its existing helper reads UTF-8 JSON with the system default encoding. The same failure was reproduced on unchanged base 12a81c0529 with the same TEMP, while both base and candidate passed with the normal ASCII TEMP. The final gate uses UTF-8 mode without changing or skipping the tests.

Independent fresh-context review approved the production changes and the subsequent fixture updates; no open code findings. Release build, hosted native micro-corpus, SBOM and release audit are left to the existing GitHub matrix and are not claimed as locally executed. Mandatory checks and workflows are unchanged by this PR.
