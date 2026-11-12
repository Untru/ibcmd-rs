# SandyBrown spreadsheet palette (#610)

## Native rule and inputs

The public `Untru/1c-fixtures` corpus is pinned at
`8d138b896f462c3053d16076cbabf0ffe5030c1c`. In
`coverage/<platform>/corpus-spreadsheet-facets`, the native
`CovSheetSandyBrown` template has exactly one web-colour palette slot and
publishes `d3p1:SandyBrown` with the web namespace. Both platforms store
ordinal **124**:

| Platform | Native slot | CF SHA-256 | Unpacked template-row SHA-256 |
| --- | --- | --- | --- |
| 8.3.27.2214 | `{3,2,{124}}` | `0a10aa69f798ac50b5f0f70cee61f9dd91763df5117503f1418ff0d471bf1a28` | `3874b98ab59f9d488561a9056834a81acaec56466fdea5385ba5dacd440c0bd8` |
| 8.5.1.1150 | `{4,2,{124},2}` | `3490af9c79c00d0a67677378c44b61bdfc3e946f34b6828500eb13a5b4127eac` | `f26e4d7dee4b15c63475339cf85e9a809d8e3f7915270f30b72f6e8c78084872` |

The existing 8.5 export adapter normalizes primitive tuple revisions before
the spreadsheet decoder. The change adds the measured ordinal and its
writer inverse. Complete base palettes and indexed palette overrides with
unsupported web ordinals refuse before compact defaults can substitute a
different colour.

## Failing-first checks

- `coverage_corpus::spreadsheet_sandy_brown_matches_native` first failed
  against the original decoder on the native 8.3 template's complete bytes.
- The compiler regression first refused `SandyBrown` as having no stored code.
- An unknown base-palette ordinal first exported `style:ToolTipBackColor`
  successfully. An independent critic then found the corresponding indexed
  override case; its regression first exported the original base colour
  successfully. Both now refuse. Known overrides preserve the complete
  native XML, and unsupported XML web-colour names also refuse.

The fallible spreadsheet parser retains a typed palette diagnostic for
embedded form settings. The form IR keeps that diagnostic on the attribute;
both the ordinary XML writer and its opaque-ChoiceList preflight reject it
before publishing XML. The two rejection regressions first passed through
successfully with the settings omitted. Seven focused embedded-settings
checks now pass, including the known-colour control, and all three compiler
regressions remain green.

The first remote merge gate also exposed a pre-existing native colour that
the stricter refusal made fatal: the 8.5.1.1529 modern-spreadsheet set's sole
kind-2 web slot `{4,2,{129},2}` is paired with native `SkyBlue` backColor.
Its CF SHA-256 is
`de37f8c326144b7155d7e3305464165b3ebe5596e7dca62a7235c547e5db8871`;
the unpacked template-row SHA-256 is
`905c8ab63fddb6174209e85bb197d7084663cf7312710b60149d116e6c643069`.
The reader also recognizes this measured ordinal. A whole-variant regression
first failed with `entry_export_failed`; it requires a successful export and
every native file byte-equal. Unknown ordinal 4294967295 remains a refusal.
No stored-row baseline is removed or relaxed.

```text
cargo test --locked -p ibcmd-compiler sandy_brown -- --nocapture
3 passed, 0 failed

cargo test --locked -p ibcmd-rs --test integration coverage_corpus::spreadsheet_sandy_brown_matches_native --no-default-features -- --nocapture
1 passed, 0 failed
```

The latter test checks both facets and combined sets on both platforms:
four native CF exports, every file owned by the variant byte-equal. It checks
the input CF and native file hashes against their manifests. Four baseline
entries hold these results; no existing baseline entry is removed.

The body-reader unit test compares both XML input dialects to the unchanged
native 8.3 XML. The production export adapter adds the unused 8.5 palette
namespace; the CLI test compares the complete native 8.5 bytes too.

## Offline import and export routes

Measured source tree: commit `3fb85dc3ba02885a4194d9a7e7fbbf448e7e330d`
(equal to the locally compiled tree). Pinned Rust 1.95.0, Windows x86-64 MSVC,
debug profile with line tables, one Cargo job. Tested executable SHA-256:
`b80159b0cb8f310a7b3c949a4c1bd1ee3c382036d4557309fd5d871a3979d15c`.

For each native facets source tree, retain `CovSheetSandyBrown.xml` and its
body directory, omit other CommonTemplates and ConfigDumpInfo.xml, and remove
only the omitted `<CommonTemplate>` entries from Configuration.xml. Other
source files retain their original bytes. This isolates the target from
other spreadsheet writer gaps. The expected template descriptor and body
remain the original manifest-checked native files.

Run each route in a fresh temporary directory, substituting the platform
and its XML version (2.20 or 2.21):

```text
<bin> cf bootstrap --base-free --platform <platform> <source> <round-trip.cf>
<bin> cf export <round-trip.cf> <cf-export> --source-version <version>

<bin> infobase config import --db-path=<db> --platform <platform> <source>
<bin> infobase config export --db-path=<db> --platform <platform> <file-export>
```

Before the file route, decompress the repository's
`tests/fixtures/file-infobase/empty.1CD.gz` into `<db>/1Cv8.1CD`. Its
decompressed SHA-256 is
`c79a7752a6afe4120316cd5fca7941be18ac840b6a836118f7beab4601d1f891`.
Default import verification remains enabled.

| Profile | XML → CF → XML | XML → file infobase → XML |
| --- | --- | --- |
| 8.3.27.2214 / 2.20 | Both commands exit 0; both target files byte-equal | Both commands exit 0; both target files byte-equal |
| 8.5.1.1150 / 2.21 | Both commands exit 0; both target files byte-equal | Both commands exit 0; both target files byte-equal |

All disposable databases, source copies and exports were removed after each
profile. These initial measurements use the pinned native corpus. Their
counts cover the isolated template, not every object in its original
configuration. Full merge-gate results remain recorded in PR #713.

## Fresh native 8.5.1.1529 acceptance — 2026-11-12

The installed Windows x86-64 native `ibcmd` reports version **8.5.1.1529**;
its executable SHA-256 is
`4dde71c1d061cc14c619255ec524b4ca8aca1f9bc38a92d1d766eb7d57721d20`.
The converter is the preserved executable from source `3fb85dc3` with the
SHA-256 recorded above. Subsequent changes through `43b571cb` affect only
this evidence document.

The input is the same manifest-checked 8.5.1.1150 facets source and CF listed
above. Retain the isolated SandyBrown template as described for the offline
routes. Every target descriptor and body byte remains unchanged. Both tools
run with `LANG=ru_RU.UTF-8`; each native invocation uses a distinct `--data`
directory, and each created file infobase uses `--locale=ru_RU`.

First create and apply a native reference directly from that XML:

```text
<native> infobase create --db-path=<reference-db> --create-database --locale=ru_RU --import=<source> --apply --force --data=<data-1>
<native> infobase config export --db-path=<reference-db> --data=<data-2> <reference-xml>
```

Then measure four product routes against that fresh native reference:

| Route | Result |
| --- | --- |
| Original native CF → converter XML | Complete two-file target map byte-equal |
| Native-created, applied file infobase → converter XML | Complete two-file target map byte-equal |
| Converter XML → CF → native create/load/apply → native XML | Complete two-file target map byte-equal |
| Converter XML → native-created empty file infobase, default import verification → native apply → native XML | Complete two-file target map byte-equal |

The CF writer uses `cf bootstrap --base-free --platform 8.5.1.1529`.
The XML writer uses `infobase config import --db-path=<db> --platform
8.5.1.1529`; default verification is enabled. The native consumer of the
generated CF uses `infobase create --create-database --locale=ru_RU
--load=<cf> --apply --force`. The other native consumer uses `infobase config
apply --force` after the converter import. Each native export uses a new
output directory.

All **11 commands exited 0**. All four complete product maps, **8 file
comparisons**, match the fresh native output with no extra target files.
The fresh reference also matches the pinned native target map completely
(two additional file comparisons). Temporary infobases, backups, source
copies, data directories and exports were removed on exit.

This proves the isolated SandyBrown read/write routes on native 8.5.1.1529.
8.3 evidence remains the pinned native corpus and the offline routes above;
no fresh 8.3 executable or SQL route was run. No whole-configuration parity
claim is made for the other facets in the original CF.

## Physical-adapter policy

The compiler's existing `web_color_code` match gains one enum arm. The scanner
includes the entire match in each name fingerprint, so this changes the
fingerprints of its 58 existing arms and adds the 59th. Only those 59
`name-special-case` records are appended for this file. All 424 existing
records, every other file and stale entries are preserved; UUID and XML-policy
inventories are unchanged. Independent review checked every added fingerprint
against the isolated enum table and ran the policy guard successfully.
