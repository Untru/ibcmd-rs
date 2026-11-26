# EDT → XML native reference on macOS (#356)

The direction-specific laboratory entry point is
`scripts/edt-lab/edt_to_xml_macos.py`. It compares a converter output with native
XML exported from a **fresh file database loaded with the installed EDT export
of the same supplied EDT project**. The original project must be authentic,
complete, and contain no `.ibcmd-provenance` directory. It never accesses an
existing database and never deletes fixtures.

The harness requires physical paths under
`/Volumes/Work/Dev/ibcmd-rs-lab` for the project, candidate, copied EDT installation,
installation/schema evidence and new run. Native SDK may remain installed under
`/opt/1cv8`. It copies the Eclipse configuration and project into the new run,
uses the custom Mac CLI `-ini-file` to pin workspace/configuration and Java,
sets Java home/temp, HOME/XDG and native
`--data/--db-path/--temp` paths. Commands use a persistent advisory macOS lab lock;
coordinate the native/heavy slot with the batch planner as well, because older
harnesses do not use this lock. It does not replace the existing Windows oracle.

The capture retains raw command arguments, UTC start/duration/exit code,
stdout/stderr, full original/after project inventories, full reference/output
inventories, exact installed versions and binary hashes. It binds #358's
`installation.json` (`schema_version: 1`): live plugin inventory and selected immutable
runtime rows must match before and after (Mac launcher, INI/bootstrap/config files
and Java executable/release). Java libraries/modules are not fully inventoried;
this is partial JVM provenance, not a full runtime pin. The separately supplied #355 model
inventory is copied verbatim, bound to its declared release, preserved legacy
baseline and installed model JAR hashes; a structural model inventory does
not establish semantic compatibility. All command children run in their own
process group; timeout or interruption terminates/reaps that group before the
slot is released. Failure leaves evidence in place.

`comparison.json` retains every missing, extra and changed path, size and hash.
It contains both the strict comparison of **all** files and the existing
source-oracle content comparison excluding only `ConfigDumpInfo.xml` (native
storage generations cannot be invented by EDT). Original excluded rows remain
in the report. No XML, whitespace, namespace, module, form or resource bytes are
normalized. The native reference and EDT export are checked again after
conversion; a terminal verdict is written only after immutability checks.
Configuration import/export is separate from database activation: no APPLY or
runtime behavior is claimed.

## Current tool and legacy model are separate

The user changed the target to current EDT on 2026-10-23. Installed
**2026.2.1+3**, expected CLI **2026.2.1.3**, is the named current-version test
candidate; it must not be described as the latest release. Historical
2025.2.3 profiles remain unchanged. `--source-profile` is explicit and normally
must match the named installed release/XML version. An intentional current-tool
exercise with a legacy model requires `--compatibility-probe`; even byte equality
then emits `COMPATIBILITY_PROBE_MATCH`, **never PASS**. A profile name or launcher
version alone does not establish full 2026 model support.

Example (replace paths with genuine #358/#355 captures and a built candidate):

```sh
source /Volumes/Work/Dev/ibcmd-rs-lab/lab-env.sh
PYTHONDONTWRITEBYTECODE=1 python3 scripts/edt-lab/edt_to_xml_macos.py \
  --project "$LAB_AUTHENTIC_BSP_PROJECT" \
  --corpus-label BSP-current \
  --run /Volumes/Work/Dev/ibcmd-rs-lab/fixtures/issue356-bsp-current-001 \
  --edt "$LAB_EXTERNAL_EDT_CLI" \
  --installation "$LAB_EDT_INSTALLATION_JSON" \
  --schema-evidence "$LAB_EDT_MODEL_INVENTORY_JSON" \
  --edt-build 2026.2.1.3 \
  --source-profile edt-2025.2.3-xml-2.21 --compatibility-probe \
  --native /opt/1cv8/8.5.1.1592/ibcmd --native-build 8.5.1.1592 \
  --ours "$LAB_CANDIDATE" --runtime 8.5.1 --xml-version 2.21
```

## Evidence and outstanding acceptance

The implementation starts at `f9ebb8dd6a4b84d57c72a72d5941931e48fae73b`
without production codec changes or donor commits. #355 owns shared codec/model
changes; #358 owns installed EDT preparation. Real discrepancies must be sent
with exact path/hash rows and minimal synthetic reproductions to #355 before
codec edits.

Thirty-three synthetic controls passed on 2026-10-23. They exercise correct native
input routing, complete missing/extra/changed rows, CDI retention, source/tool/
plugin/Java/schema/reference mutation, incomplete runtime manifests, failed native import, preserved failed command bytes,
interrupted process cleanup, path/provenance guards, and compatibility verdicts.
Fixtures are retained under the external TMPDIR. Local log:
`/Volumes/Work/Dev/ibcmd-rs-lab/logs/issue356-harness-controls-r17.log`.

```sh
source /Volumes/Work/Dev/ibcmd-rs-lab/lab-env.sh
IBCMD_TEST_KEEP_FIXTURES=1 PYTHONDONTWRITEBYTECODE=1 \
  python3 tests/scripts/edt_to_xml_macos_test.py -v
```

Independent review identified and verified fixes for interrupted process cleanup,
external Eclipse/JVM configuration and rehashing the native reference. These
controls are not installed EDT or BSP acceptance. At the initial harness checkpoint no genuine BSP/current EDT/native capture
was available. The later empty smoke below does not provide a BSP match;
#356 remains open. A real empty-configuration smoke,
when available from #358, also cannot substitute for the required full BSP
comparison. Private corpus files and full private path inventories remain local;
only the reusable harness, synthetic controls and non-private conclusions belong
in this repository.

## Real current-version empty smoke (2026-10-23)

[Non-private summary](evidence/356-current-empty221-20261023.json) records the
actual run at `fixtures/issue356-current-empty221-001`. #357 supplied the genuine
EDT-native import of `empty-8.5.1.1592-en-001/native-xml`, not a generated project.
The source project has four files (2,696 bytes). We independently exported that
same project with **EDT 2026.2.1.3**, loaded the resulting XML into a new native
**8.5.1.1592** file database, then exported native XML and converted the original
project with explicit `edt-2025.2.3-xml-2.21` compatibility mode.

All eight version/export/create/import/conversion commands exited 0. Both
configuration files match byte for byte: ours has 2 files/12,369 bytes; native
has 3 files/13,009 bytes. The sole strict difference is native-only
`ConfigDumpInfo.xml`, 640 bytes, SHA-256
`b0b1c4a34b1c22ef1b080e257ce253b71be717186b797cd51668c493b9131e61`.
The result is **COMPATIBILITY_PROBE_MATCH**. No configuration-content codec
regression was observed in this empty control; #355 received the exact findings.

The candidate was borrowed read-only from #357's locked, offline Rust 1.95
`--no-default-features` build of the unchanged `f9ebb8dd` Rust tree, SHA-256
`e4ac476fad5b93723ecf2c335921efa19e61572f49956cdeddd611a53cd9ebbd`.
This PR changes Python/docs only. Project, selected installation files, plugins,
schema evidence and native reference passed after-run immutability checks.

The final combined EDT workspace log contains **three unresolved severity-4
entries**. `post-capture-diagnostics.json` records them without modifying any
original capture/verdict. Per-command workspace-log capture and explicit
validation-scope fields were then added to the reusable harness and covered by
a negative diagnostic control. The SDK errors are not suppressed or classified
as harmless. Content equality does not establish clean installed-EDT validation.

Full inventories, exact argv/exit/timing records, raw stdout/stderr and workspace
logs remain in the external run directory. No native/EDT corpus bodies are
committed. The native slot was released after the run. Full BSP comparison,
current-model semantic support and clean installed SDK acceptance remain open;
PR #506 remains Draft and #356 is not closed.

The original authentic-project `edt-export.stdout` also contains one mixed-case
`[Fatal Error] :1:1: Premature end of file.` line. It is preserved in a separate
`post-capture-stdout-diagnostics.json` supplement. The local scanner now detects
case-insensitive error/fatal markers; its regression control retains this exact
line. The same direct-project/fresh-workspace route therefore produces this
error on a genuine native-import project too; it alone does not prove a codec
regression. It is not waived as harmless.

Retained CLI and shutdown-hook logs are scanned too; fatal-only records in either
file prevent a clean diagnostic verdict even when stdout/stderr and `.log` are
empty. The negative control exercises both files.


## Nonempty public DCS/forms probe with Server JVM selection

The parent authorized a bounded nonempty probe over the public DcsFilterEvidence
native tree: 19 files, 86,112 bytes, tree
`efc5814c4b31e7c8892511003e591fe0f0567a9f0d1d69e3a2594ada60b51a72`.
#358 copied its provenance; #357 performed the single genuine installed-EDT
import. We reused that exact authentic `NativeControl` project after #357
released the shared slot, avoiding a second native-to-EDT import.

The optional `--jvm-library` pins the bound Java home's
`lib/server/libjvm.dylib`. It must appear in the installation runtime inventory,
is hashed in the private launch receipt, and is rechecked after capture. The
Java-executable selection remains available unchanged. Three additional controls
cover explicit selection, omitted inventory and library mutation. This remains
selected-runtime provenance, not a complete JVM modules/libraries inventory.

[Non-private failure summary](evidence/356-current-server-dcs-20261023.json)
records `fixtures/issue356-current-server-dcs-001`. Installed EDT export and fresh
native create/import/export all returned 0. The authentic project contains
19 files/79,140 bytes; the installed EDT export has 18 files/82,953 bytes;
the fresh native reference has 19 files/86,112 bytes. Full inventories and hashes
remain local. The candidate is the same immutable master Rust-tree binary used
in the empty control.

Our conversion decoded successfully but failed closed during encode preflight:
`conversion.edt-encode-preflight-failed`. The canonical config-interface guard
expects CRLF after the XML declaration/root of `ClientApplicationInterface.cai`;
the genuine current serializer produces LF. No output was published, so full
output equality was **not reached**. The 639-byte public sidecar reproducer is
retained in `fixtures/issue356-sidecar-repro-001`, SHA-256
`f1659827aed39cb1240e6dc9e4252c8efa66559a3c5448e7e75e96d29e52d62b`.
The exact report and reproducer were sent to #357 (agreed sidecar-fix owner) and
#355; this PR does not edit their shared codec files.

The genuine direct-project export also emitted the same mixed-case Fatal Error
and one severity-4 workspace entry. They remain in the per-command raw logs and
`diagnostic-supplement.json`, unsuppressed. All original project, installation,
schema and candidate hashes remained unchanged. The native slot was released.
An overlapping #381 compilation was reported by the coordinator, so these
command timings are not performance measurements. No further heavy phase or
retest starts until the parent grants it. Full BSP/UH acceptance and clean SDK
validation remain open; PR #506 remains Draft.


## PR review: whole-application overlap and configuration custody

The two actual Codex findings in PR #506 were confirmed against `9c601e78`.
Before any output directory, snapshot, copy or process, the harness now validates
`.app/Contents/Eclipse/1cedtcli` layout and rejects either-direction overlap
between the run and the entire physical application bundle. Existing-ancestor
`samefile` checks cover case aliases on the external case-insensitive volume.
The regression controls check configuration/Contents/MacOS/Resources descendants,
an ancestor and the real case alias without executing a recursive copy.

The entire source configuration tree is recorded in
`configuration-source-before.json`. A complete `configuration-copy-before.json`
must match before any launch. Both initial tree hashes enter the terminal
result, separately from the supplied installation-manifest hash; unlisted extra
configuration files therefore cannot silently share the same effective-input
identity. Full source/copy after-inventories and `configuration-custody.json`
retain private runtime cache changes without normalizing them away.

Configuration custody is captured before later manifest revalidation, including
when a listed configuration file changes. Missing, renamed or unreadable copied
configuration fails custody and cannot publish a verdict; a retained rename
control covers this without deletion. The source configuration must remain
unchanged. Thirty-three bounded retained-fixture controls pass; no native run,
Rust build, private-input mutation or cleanup was needed for these review fixes.
Older smoke evidence is preserved unchanged and is not retroactively described
as having this stronger custody contract.


## Corrected candidate: fast direct route and next DCS boundary

The immutable candidate built from combined production head `5e5b5bcd`, SHA-256
`6da274e498d6b21ef017c00bc8698cf6c14596d996604acce40a77bfe8431016`,
passed #357's generated-project forward/return checks. We separately tested the
**genuine installed-EDT project**, which is a different source route. The
[non-private summary](evidence/356-current-dcs-fixed-direct-20261023.json) binds
`fixtures/issue356-current-server-dcs-fixed-direct-002`.

No SDK/JVM was restarted. Before running the converter, the local capture
checked complete current/terminal genuine project inventories, complete current
installed-export inventories against the exact original native-import input,
the live native reference against its original full inventory, exact original
native create/import/export command routes and exit codes, SDK binary identity,
and the corrected candidate/build receipt. All selected raw receipts and input
trees were checked again afterward. The original native reference was reused
only because the entire installed-export input is identical, not because a
summary or file count matches. Prior configuration-custody limitations remain
visible and are not retroactively changed by this direct run.

The candidate gets past the earlier client-interface refusal but still fails
closed at encode preflight on
`Catalogs/FilterProbe/Forms/ListForm/Attributes/Список/ExtInfo/ListSettings.dcss`:
`dcss: must be CRLF, found Lf`. Decode completes, exit code is 2, and output is
not published. Full native equality and a direct three-way report are therefore
not reached. All input, reference, candidate and selected evidence hashes remain
unchanged. Minimal authentic public sidecars are retained under
`fixtures/issue356-dcss-repro-001`; exact reports and hashes were sent to #355
(shared codec owner) and #357. This PR makes no independent shared codec edit.

The retained `fixed-direct-001` attempt stopped on a capture receipt assertion
before invoking the converter; the checker was corrected to bind #357's actual
explicit-project export route and run in a fresh directory. Neither attempt
rewrote old failures. Full BSP acceptance and clean SDK validation remain open.


## Post-DCSS candidate: the one granted authentic reverse conversion

After all nine combined tests and a successful clean build, the parent granted
exactly one direct reverse conversion using source `484372e1` (tree
`7cc71ac761c3c1968dd31fc6eb4b3bda00796e3a`), candidate SHA-256
`5542dce707bbd7629d5a344a1c5aa13e7a1b64d6d704f9744a336498b14d5113`.
The [non-private result](evidence/356-post-dcss-direct-20261023.json) binds
`fixtures/issue356-current-server-dcs-fixed-direct-003`.

The capture renewed complete genuine project, current/old installed-export and
native-reference inventories, actual original SDK command routes/exit codes,
SDK executable identity, and the new source/test/build receipts and raw log
hashes. All before/after inputs, selected evidence and candidate hashes remain
unchanged. No SDK, JVM or compiler was started, and the converter was invoked
once for this grant.

The previous DCSS boundary is passed, but encode preflight now refuses the next
actual sidecar: `ConditionalAppearance.dcssca: must be CRLF, found Lf` in
`FilterProbe.ListForm`. Decode completes, exit code is 2, and output is not
published. Native equality and the direct three-way branch therefore remain
unavailable. Exact reports and minimal genuine public source bytes at
`fixtures/issue356-dcssca-repro-001` were sent to #355 and #357. Shared codec
ownership remains unchanged. Full release was reported immediately for #381's
next integration phase. No retry occurs without another grant; all BSP/current
schema/strict SDK acceptance limitations remain open.


## Post-DCSSCA candidate: conversion completes, exact native comparison fails

The single granted direct conversion with clean source `c0cd767b` and candidate
SHA-256 `6340fd5458a19aa7e40a1208dd35bb29138d79313ce8d2534ad5aa458d2bf471`
completed with exit code 0 after the combined twelve tests passed. The
[full public inventories and comparison](evidence/356-post-dcssca-direct-20261023.json)
bind `fixtures/issue356-current-server-dcs-fixed-direct-004`. Complete authentic
project, installed-export, reused native-reference, candidate and evidence
bindings were renewed and remained unchanged afterward. No SDK, JVM or build
was launched; the slot was released immediately after this one conversion.

The output has 18 files; the native reference has 19. Exact comparison fails on
four paths: both managed form `Module.bsl` files (474 versus 492 bytes), report
`Template.xml` (3189 versus 3264 bytes), and absent `ConfigDumpInfo.xml` (3084
native bytes). The three shared-file differences consist exactly of LF versus
CRLF bytes in this capture. That diagnostic does not normalize the acceptance
comparison: both strict equality and configuration-content equality remain
false. The report preserves every file inventory and differing row, including
the excluded CDI row. This public smoke supplies no BSP/UH acceptance, current
model schema certification, or waiver of retained SDK/style errors. Earlier
failed captures remain intact.


## Post-scaffold/DCS candidate: all configuration bytes match

The one granted direct run in `fixtures/issue356-current-server-dcs-fixed-direct-005`
uses clean production source `e12f77a1042e3ced9c620db90983bd730d87dfa5`, tree
`56aa3dcb6d1b75a057ba98277d8ab927b7ba111b`, and executable SHA-256
`ba118d90b8e0736f08eb6d3d4e1a6f36fee3a57d04f91c68bbec24c92484b12a`.
The successful build and 21 focused tests are bound through all raw phase logs.
[Full inventories and exact result](evidence/356-post-scaffold-dcs-direct-20261023.json)
retain the renewed authentic project/export/native reference and final guards.

Conversion exits 0. Every one of the 18 configuration files is byte-exact against
the real native export; no normalization is used. Strict all-file equality is
still false: the native reference additionally contains `ConfigDumpInfo.xml`,
3084 bytes, SHA-256 `7301e7124e132fc5119c4a4c809b84187f475d8f89ec7a89a6085ea4e4447b7d`.
The explicit configuration-content view excludes only that row and matches;
the strict view retains it. All inputs, candidate and selected evidence remain
unchanged. The slot was released immediately; no SDK/JVM/native/build was run.
This public smoke does not satisfy BSP/UH coverage, current-model semantic
support or clean SDK/style diagnostics. Earlier failures remain preserved.


## Remaining CDI contract and SDK diagnostics investigation

The issue's original criterion says the result matches a native export of the
loaded BSP project. It does not explicitly waive any file. The existing
[oracle contract](oracle.md) documents a CDI-only exclusion for authentic
source-only EDT conversion, while unchanged provenance return includes CDI.
Those are different claims: this PR retains strict failure alongside the
successful configuration-content comparison and does not amend issue acceptance.

[Retained public evidence](evidence/356-cdi-diagnostics-investigation-20261023.json)
compares the original public native source CDI with the fresh database CDI.
All 18 metadata name/ID rows agree; 16 versioned rows have different
`configVersion` values. The authentic project contains 19 files, no storage
`versions` record, and no literal fresh-reference version tokens. This absence
alone is not proof that no algorithm could exist, but there is no evidenced
source-only derivation of those exact database generations.

The ordinary exporter reads `Config`/CF `versions` UUIDs in
`src/mssql_dump/config_dump_info.rs` (`parse_versions_blob`) and writes each UUID
as 16 little-endian bytes plus four zero bytes (`config_version`), not a source
XML content hash. Extensions have a distinct `configinfo` SHA-1 route and cannot
be used as an ordinary-configuration substitute. `src/compiler/versions.rs`
creates deterministic bootstrap generation UUIDs under its own domain/profile;
that is an independent storage image, not proof of matching an already loaded
native database. `docs/COMMANDS.md` also uses CDI versions for incremental export
and stale-base checks. Inventing or copying these tokens could give false
incremental identities. The adapter boundary in `morph1c-adoption.md` explicitly
states it does not manufacture native storage generations from EDT.

The concrete external input missing for exact whole-export equality is the
reference database's version-generation state (or a verified native algorithm
that reproduces it from fully specified inputs). No CDI implementation is
justified from current source-only evidence. #356 owns this investigation;
#357/#358 were asked not to duplicate or fabricate storage values. If a product
route is later specified to accept an authoritative storage image, CDI should
be generated from its validated CURRENT inventory and versions using the
existing exporter, with source/storage binding and version-specific controls.
That would be a separately declared input contract; the source-only converter
and this acceptance gate must remain truthful in the meantime. No further JVM
run is needed to establish this retained-data boundary.

SDK diagnostics are classified by evidence, without suppressing them:

- `Skipping the lifecycle event because workbench is not running` is emitted by
  vendor bundle `com.e1c.g5.dt.applications.infobases.ui` on the genuine native
  import/export/validate routes before our generated route. It is consistent
  with the headless environment; exact root cause is not independently proven.
- `[Fatal Error] :1:1: Premature end of file.` appears in genuine explicit-project
  export/validate stdout and in the same generated routes. The matched same-phase
  error multisets are equal, with nothing added or removed. It is demonstrably
  pre-existing on the genuine route; stdout supplies no failing XML path or
  stack, so a vendor-parser root cause is still unproven.
- The genuine and generated validation TSVs each contain the same 24 Minor rows
  (14 Code style, 10 UI style), zero configuration-error rows and no added or
  removed diagnostics. These are source/control diagnostics, not evidence of a
  converter regression. UI style remains unclassified by the strict gate and
  is not waived.

These matched diagnostics come from the earlier `6da274e4` installed capture,
not a new installed run of candidate `ba118d90`. They cannot establish clean SDK
acceptance for the latest candidate. A future justified vendor investigation
should isolate the genuine explicit-project route under the same external
installation/INI with vendor-supported diagnostics to identify the parser
input; it must retain the current errors and use a separately granted native
slot. No SDK/JVM/build/converter rerun was made for this investigation.


The existing explicit-project export stdout contains only the fatal line;
stderr is empty, and neither retained workspace/CLI/shutdown log identifies
its input or stack. The CLI log reports SLF4J falling back to a no-operation
logger, plus JDK warnings; these do not prove the parser error's cause. The
minimal next diagnostic step is offline inspection of the installed vendor
logging/trace options and the already bound startup configuration to identify
an option capable of naming the failing parser input. Only if that yields a
concrete supported trace should one fresh genuine-control run be proposed,
with identical version/configuration bindings and a separately granted slot.
No speculative JVM option changes or repeated conversions are needed now.
