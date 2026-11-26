# Native AutoFont height and OEMFixedFont — 2026-11-12

Refs #608. The existing public `corpus-spreadsheet-facets` cases
`CovSheetAutoFontHeight` and `CovSheetOEMFixedFont` exercise the loss in
CF/file-infobase export and the refusal during XML compilation. This change
uses the pinned fixtures; it introduces no application data or dependency.

## Inputs, stored rule and source binding

- Merge base: `54b796f64da5daed3714acdf7810581fe0e58778`.
- Pinned `1c-fixtures`: `8d138b896f462c3053d16076cbabf0ffe5030c1c`.
- Archival native profiles: 8.3.27.2214 / XML 2.20 and 8.5.1.1150 / XML 2.21.
- Product build: `cargo build --locked --profile iter`, completed in 74m 50s.
- Clean source commit: `8336b725af3fd5814493cef3ea6d55c816de7063`;
  tree `dce0edc9b9e3f1aa3f2a1991016f95423ace9cc5`, identical to published
  `a0742270526e6de7459b2cc2b3d057ca32fa760d` before this evidence/baseline update.
- Converter executable SHA-256:
  `cd3cace9c571441fdfcca49fbfde7386e29bcc25bbad539d6b15954bd83eceac`.

Each public CF and every expected owned file were checked against the
fixture manifest before comparison. Decoded facet CF hashes:

| Profile | SHA-256 |
| --- | --- |
| 8.3.27.2214 | `0a10aa69f798ac50b5f0f70cee61f9dd91763df5117503f1418ff0d471bf1a28` |
| 8.5.1.1150 | `3490af9c79c00d0a67677378c44b61bdfc3e946f34b6828500eb13a5b4127eac` |

System font code 1 denotes `sys:OEMFixedFont`; the existing mappings for
codes 0 and 2 remain. The admitted height-only AutoFont is exactly six
scalar fields: revision 7 or 8, kind 3, the existing height mask 2,
nonnegative integer height in tenths, tail 1 and scale 100. For example,
`{8,3,2,90,1,100}` exports `<font height="9" kind="AutoFont"/>`.
Plain five-field AutoFont remains unchanged. Other masks, extra members,
nonstandard tails/scales and qualified or unknown XML attributes are refused.

The existing 8.5-to-8.3 and 8.3-to-8.5 form adapters share this exact tuple
predicate. Their fast scanner skips quoted text, including doubled quotes;
text that merely contains a font-shaped tuple is unchanged. Shared font
consumers retain their contracts: Gantt points refuse nonplain AutoFont
members that they cannot represent, and chart title/legend/axis font XML
declares `xmlns:sys` when emitting a system-font reference.

An inspection of 574 pinned native spreadsheet XML documents found 11
height-9 AutoFont occurrences, 24 plain AutoFont occurrences and 11 OEM
references. No other AutoFont attributes were observed in that cohort.
This bounds the admitted layout; it does not justify accepting other forms.

## Failing first and focused verification

Before the fix, the coverage test reported the target bodies different on
both profiles, in both facet and combined sets: the height-only AutoFont
and OEM font elements were missing. The focused unit command had four
failures and two compatibility controls passing. Down- and up-projection
tests each had two failures and one compatibility control passing.

```text
cargo test --locked -p ibcmd-rs --test integration coverage_corpus:: --no-default-features -- --nocapture
cargo test --locked -p ibcmd-compiler
cargo test --locked -p ibcmd-form-body
cargo test --locked -p ibcmd-form-writer
cargo test --locked -p ibcmd-moxel
```

Review regressions were also observed failing before correction: Gantt
accepted a height member that its XML emitter discarded; the fast scanner
rewrote quoted tuple text; a qualified height attribute was accepted; and
chart OEM font XML lacked its namespace declaration. The first three
regressions passed in a 15-test focused run after correction. The last
correction passed the complete 43-test Moxel crate.

`cargo xtask check --base upstream/master` then passed all **8 steps**:
format, crate clippy, root compile check, the four changed crates, and the
complete coverage suite. Crate results were compiler **137/0/0**,
form-body **574/0/0**, form-writer **40/0/1**, and Moxel **43/0/0**
(passed/failed/ignored); coverage **6 passed, 0 failed**. Two earlier
checks were interrupted to correct review findings and are not completed
gates. No existing test was ignored, weakened or removed.

The physical-adapter guard passed with 12 reviewed additive fingerprints:
three compiler, seven form-body and two form-writer entries. They describe
the exact bounded font layouts and strict XML checks above. All old and
stale entries remain; schema JSON, vendor code and dependency manifests are
unchanged. The final full local merge gate is reported separately in the PR.

## Product routes on both archival profiles

For each profile, use the hash-checked facet CF, the combined CF and an
isolated copy of native XML containing the two target templates. Isolation
removes other CommonTemplate declarations and their files plus
`ConfigDumpInfo.xml`; the target files themselves remain byte-identical.
The offline database routes start from a fresh decompressed public
`tests/fixtures/file-infobase/empty.1CD.gz` with SHA-256
`c79a7752a6afe4120316cd5fca7941be18ac840b6a836118f7beab4601d1f891`.

```text
ibcmd-rs cf export <native-facet.cf> <out> --source-version <2.20-or-2.21>
ibcmd-rs infobase config load --db-path=<copy> --data=<unique> <native-facet.cf>
ibcmd-rs infobase config export --db-path=<copy> --platform=<profile> --data=<unique> <out>
ibcmd-rs cf export <native-combined.cf> <out> --source-version <2.20-or-2.21>
ibcmd-rs cf bootstrap --base-free --platform=<profile> <isolated-xml> <generated.cf>
ibcmd-rs cf export <generated.cf> <out> --source-version <2.20-or-2.21>
ibcmd-rs infobase config import --db-path=<new-copy> --platform=<profile> --data=<unique> <isolated-xml>
ibcmd-rs infobase config export --db-path=<new-copy> --platform=<profile> --data=<unique> <out>
ibcmd-rs infobase config save --db-path=<new-copy> <saved.cf>
```

These five directions, two templates and two profiles produced **20 complete
owned file maps / 40 files byte-equal**, with no extra owned files. Each map
includes the descriptor and `Ext/Template.xml`, compared as unnormalized
bytes. The file-database load route measures CONFIGSAVE, without claiming
native apply. The default XML import verification was enabled.

All commands outside combined export exited 0. Both combined exports exited
2 with typed `entry_export_failed` diagnostics for other objects, while both
font maps matched completely. The 8.3 combined run reported four other
refused entries (two graphical schemas and two business-process flowcharts).
The 8.5 run reported those four plus a period-field property bag, composition
standard-picture value and table-picture TitleFont. Whole combined-config
parity is therefore **not** claimed; these refusals are not asserted to be
pre-existing without a separate merge-base reproduction.

Eight successful exact CF-storage extraction commands inspected the
generated CF and database-saved CF. Both writers produced:

| Profile | Height-only AutoFont | OEM font (line breaks omitted) |
| --- | --- | --- |
| 8.3.27.2214 | `{7,3,2,90,1,100}` | `{7,1,3,{1},120,"Arial",1,100}` |
| 8.5.1.1150 | `{8,3,2,90,1,100}` | `{8,1,3,{1},120,"Arial",1,100}` |

This checks physical 8.5 revision emission as well as logical XML equality.
The official `tools/coverage/update_baseline.py` consumed a targeted report
derived from these completed CF measurements and appended **8 entries**:
the two variants in facet and combined baselines on both profiles. This was
a product measurement report, not purported Cargo output. All old baseline
entries were preserved.

## Fresh native 8.5.1.1529 acceptance

Installed Windows x86-64 native `ibcmd` **8.5.1.1529**, SHA-256
`4dde71c1d061cc14c619255ec524b4ca8aca1f9bc38a92d1d766eb7d57721d20`,
accepted the same isolated public target XML. Both sides used
`LANG=ru_RU.UTF-8`; native create used `--locale=ru_RU`, and every command
had its own `--data`. All databases were freshly created disposable copies.

```text
ibcmd infobase create --db-path=<reference> --create-database --locale=ru_RU --data=<unique> --import=<isolated-xml> --apply --force
ibcmd infobase config export --db-path=<reference> --data=<unique> <reference-xml>
ibcmd-rs cf export <native-facet.cf> <ours-cf> --source-version 2.21
ibcmd-rs infobase config export --db-path=<reference> --platform=8.5.1.1529 --data=<unique> <ours-db>
ibcmd-rs cf bootstrap --base-free --platform=8.5.1.1529 <isolated-xml> <ours.cf>
ibcmd infobase create --db-path=<cf-consumer> --create-database --locale=ru_RU --data=<unique> --load=<ours.cf> --apply --force
ibcmd infobase config export --db-path=<cf-consumer> --data=<unique> <cf-consumer-xml>
ibcmd infobase create --db-path=<xml-consumer> --create-database --locale=ru_RU --data=<unique>
ibcmd-rs infobase config import --db-path=<xml-consumer> --platform=8.5.1.1529 --data=<unique> <isolated-xml>
ibcmd infobase config apply --db-path=<xml-consumer> --data=<unique> --force
ibcmd infobase config export --db-path=<xml-consumer> --data=<unique> <xml-consumer-xml>
```

All **11 commands exited 0**. The four product directions (CF reader,
native-applied DB reader, XML-to-CF with native consumption, XML-to-DB with
native apply) matched the fresh native reference: **8 complete maps /
16 files**, with no extra owned files. Fresh native output also matched
the pinned 8.5.1.1150 target files: **2 maps / 4 files**. Thus the XML writer
results were consumed and applied by the installed native platform.

All temporary infobases, backups, data directories, extracted rows and
export trees were removed. Local retained artifacts are sanitized logs,
hash/count reports and the source-bound converter executable. No private
configuration or customer database was used.

Limits: fresh native **8.5.1.1529** was run; the 8.3.27.2214 and 8.5.1.1150
builds supply archival native oracles and offline product routes. Fresh
8.3 native acceptance, SQL Server and whole combined configurations were
not measured. `Refs #608` retains those acceptance limits explicitly.
