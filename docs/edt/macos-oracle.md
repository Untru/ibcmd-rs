# Current EDT oracle on macOS

Refs #358; shared laboratory prerequisite for #355–357. This is capture tooling,
not a completed BSP/UH oracle or a claim that the converter supports a new model.
The user's 2026-10-23 instruction changed new testing to current EDT. Existing
`edt-2025.2.3-xml-2.20` and `edt-2025.2.3-xml-2.21` profiles remain unchanged.
A test using one of those writer models and a 2026 installed tool must record
both versions separately and describe it as a compatibility probe.

## Verified baseline

On 2026-10-23, the [official download page](https://edt.1c.ru/docs/new/download.php)
advertised **2026.2.2**. The installed Mac components were
`1c-edt-2026.2.0+289-x86_64` and `1c-edt-2026.2.1+3-x86_64`. The tested local
build is **2026.2.1.3**, not the newest advertised release. No 2025 acquisition or
installation is needed for this changed target. No browser access is required.

The official installed 2026.2.1 component was copied without changing the
original installation into:

```
/Volumes/Work/Dev/ibcmd-rs-lab/tools/edt-2026.2.1+3-x86_64/
  1cedt (2026.2.1+3).app/Contents/Eclipse/1cedtcli
```

The Mac CLI is a custom launcher: it accepts `-ini-file`, `-data`, `-timeout`,
`-vmargs`, and `-command`. Generic Eclipse `-vm`, `-configuration`, `-user`,
`-nl` belong in the retained custom INI, not the CLI argument list. The harness
makes relative `../Eclipse/` bootstrap paths absolute when moving that INI. It
omits the optional original `-Dosgi.debug=` property to avoid bootstrap tracing
in the exact version output; raw command/workspace diagnostics remain intact.
The original INI is immutable and hashed. The complete copied app, including
its configuration/p2 caches, is external. A shared nonblocking advisory lock
serializes cooperating Mac EDT/native captures through final inventory checks.

The explicitly selected Java is the already installed read-only
`/Applications/1C/1CE/components/axiom-jdk-full-25.0.4+3-x86_64/bin/java`.
Its executable, `release` file and explicitly selected Server VM library are hashed.
This is **partial Java provenance**,
not a complete JDK inventory. Java `user.home`/`java.io.tmpdir`, environment
HOME/TMP/TEMP/XDG paths, workspace, user area, command logs and evidence all point
at a fresh external run. The harness removes inherited Java argument-injection
environment variables before launching. It never launches the default workspace,
opens the original ERP database, deletes fixtures, or changes security settings.

## Reproduce discovery and preparation

Source `lab-env.sh` and use a new run name; existing evidence cannot be resumed
or overwritten. The run must be disjoint from the entire physical `.app`
bundle, including its root, Contents, MacOS and Resources, and must not contain
the bundle. Existing directory identities also reject case aliases on macOS.
An invalid launcher layout or overlap is rejected before any capture output or
process is created. No Rust build is required for this capture step.

```sh
python3 -B scripts/edt-lab/prepare_macos.py discover \
  --edt '/Volumes/Work/Dev/ibcmd-rs-lab/tools/edt-2026.2.1+3-x86_64/1cedt (2026.2.1+3).app/Contents/Eclipse/1cedtcli' \
  --java /Applications/1C/1CE/components/axiom-jdk-full-25.0.4+3-x86_64/bin/java \
  --edt-build 2026.2.1.3 \
  --jvm-library /Applications/1C/1CE/components/axiom-jdk-full-25.0.4+3-x86_64/lib/server/libjvm.dylib \
  --run /Volumes/Work/Dev/ibcmd-rs-lab/logs/issue358-current-discovery-NEW \
  --timeout 240
```

`prepare` additionally takes `--native PATH --corpus ACTUAL-LABEL
--xml-version 2.20|2.21`; it checks that Configuration.xml declares that version,
imports with the corresponding runtime 8.3.27/8.5.1 and `--build true`, then exports
using the installed serializer. Inputs must be physical external lab trees.
`PREPARED_NOT_ACCEPTED` only means commands, structural checks and immutable
input/tool checks completed. Raw error diagnostics are unresolved evidence,
never automatically waived. Preparation does not run a converter or provide any
of the three route acceptance results described in [oracle.md](oracle.md).

The schema_version 1 `installation.json` contains the complete immutable plugin
snapshot (`root`, `files[{path,bytes,sha256}]`, count, total bytes and tree SHA-256),
CLI path/hash and absolute `runtime_files` rows for the real Mac launcher,
original/custom INIs, CLI startup script, Info.plist, configuration/config.ini,
bundles.info, `.options`, Java executable and Java release. Mutable installation
configuration caches are deliberately outside the immutable payload set.
`installation-after.json` must match before any terminal capture receipt.
Command receipts retain exact argument arrays, working directory, isolated
environment overrides, start time, duration, timeout and native exit code, plus
unmodified stdout/stderr. SIGTERM/interrupt/timeout kill and reap the owned process
group. Failure evidence and all control fixtures are retained; cleanup is to
Trash by a separate user-authorized action.

## Actual evidence and remaining gates

`logs/issue358-current-discovery-003` contains actual installed-tool commands:
`version`, `help`, `help import`, `help export`, `help validate`, each exit 0.
Version stdout is exactly `2026.2.1.3`. The complete plugin inventory contains
1,870 files / 1,639,204,208 bytes with tree SHA-256
`b409df68cb99a34d81e3994cc212d8517dcc158f88488dd321f7f17f9657d09d`.
CLI SHA-256 is
`d44b5890360f382280f66035ea627b47e6ef24c2169d5fef1783578f807eab1f`.
Before/after immutable inventories are equal. Each run preserves the exact
executed harness and oracle source; later safety refinements are verified by
retained controls, not retroactively attributed to that executed source.

Earlier discovery-001 records unsupported CLI `-vm` failing before JVM startup.
Discovery-002 records a successful real version command followed by strict version
rejection because optional bootstrap tracing preceded the version. Neither has a
positive terminal discovery result. Both are retained.

The separate #355 model inventory binds installed JARs and compares legacy class
and resource inventories; it does not establish semantic schema equivalence or
justify silently relabeling the 2025 converter profiles.

The retained private native XML identifies its actual configuration as
`УправлениеПредприятием` 2.5.27.70, compatibility 8.3.27, XML 2.21. It is an ERP
corpus; it is not established as BSP or ERP UH. No private configuration bodies,
CFs, exports, full private inventories, or database files belong in GitHub.
Full current-version BSP/UH native→EDT→installed export preparation, fresh native
reference, structured validation/control and all three exact route comparisons
remain required. Empty/public fixture smoke tests can demonstrate the laboratory
pipeline but cannot close that corpus acceptance gap.

Controls: `python3 -B tests/scripts/edt_prepare_macos_test.py` with external
TMPDIR from lab-env.sh. The synthetic controls prove guards/receipts only and
never count as installed EDT compatibility or BSP/UH acceptance.

## Actual empty 2.21 compatibility smoke and JVM diagnosis

The subsequent real captures use the genuine native empty XML fixture from
`fixtures/empty-8.5.1.1592-en-001/native-xml`, not XML generated by ibcmd-rs.
#357 imported it with installed EDT to create NativeControl and exported it with
that installed serializer. Its generated provenance-free project then exported
byte equal with the same serializer; its unchanged reverse conversion matched
all original native XML files including ConfigDumpInfo.xml. The capture remains
strict **FAIL** because validation emitted `[Fatal Error] :1:1: Premature end of
file.` and workspace errors. Zero process exits and an empty TSV do not waive it.

#356 used that same authentic NativeControl project, ran the installed export,
created a fresh native 8.5.1.1592 file database, imported and re-exported it.
Its legacy-profile EDT reader output matched every configuration-content byte
of that independent native export. The sole strict inventory difference was
native ConfigDumpInfo.xml, retained in the report. This is
`COMPATIBILITY_PROBE_MATCH`, not new-model acceptance. Original projects, tool,
schema and input inventories remained unchanged.

The raw [direct reader](../evidence/edt-current-empty221/direct-edt-to-xml.json),
[installed forward serializer](../evidence/edt-current-empty221/installed-forward-serializer.json)
and [unchanged return](../evidence/edt-current-empty221/unchanged-return.json)
reports retain **all three path rows** in each route. Native tool is 8.5.1.1592,
EDT 2026.2.1.3, candidate ibcmd-rs 0.4.1 using the separately declared legacy
2025.2.3 XML-2.21 model. Exact raw classification counts are:

| Route | All equal | Native=ours only | EDT=ours only | Other disagreements |
|---|---:|---:|---:|---:|
| Direct EDT reader | 1 | 1 | 1 | 0 |
| Installed forward serializer | 1 | 0 | 2 | 0 |
| Unchanged return | 1 | 2 | 0 | 0 |

These raw serializer differences are deliberately not normalized. Route equality
criteria differ as explained in oracle.md: raw `all_equal` counts alone are not
acceptance. Reproduction uses the existing `source-three-way-oracle` command with
explicit `--native`, `--edt`, `--ours`, `--source-version 2.21`, and exact tool
versions; all three complete commands/stdout/stderr, driver source and before/after
input inventories are retained at `logs/issue358-three-way-current-empty221-001`.
Captured input trees are unchanged. Full local route evidence is at
`fixtures/issue357-current-empty221-001` and
`fixtures/issue356-current-empty221-001`. None of these are BSP/UH acceptance.

A concrete setup defect was then isolated: Eclipse selected **Client VM** even
with the supplied Axiom bin/java and `-server`. The supported INI `-vm` entry
pointing directly at the actual `lib/server/libjvm.dylib` made the real installed
EDT report **Server VM**, without changing/spoofing VM properties. The additional
library hash is in runtime_files. Version-only evidence is retained at
`logs/issue358-current-server-discovery-001`. On a copy of the authentic native
project, matched validation with that actual Server VM completed with exit 0,
empty TSV and **empty stderr**, but stdout contains
`[Fatal Error] :1:1: Premature end of file.`, with unchanged source/installation inventories
(`logs/issue358-server-native-validation-001`). The headless workbench lifecycle
severity-4 record still appears; it is retained and has not been waived. This
control does not retroactively turn the earlier generated validation into PASS;
the generated route must be rerun with the same corrected VM before comparing
phase-specific diagnostics. The subsequent #357 Server VM empty run does reproduce
the fatal line on both authentic and generated matching phases, with identical
error multisets and all content/immutability gates true; its strict status remains
FAIL. The Server VM selection fixed VM identity, not the XML fatal or headless
workbench errors.

Read-only inspection also identified the remaining workbench diagnostic's exact
origin. The actual control INI and argument array contain no
`java.awt.headless` flag. The installed
`com.e1c.g5.dt.applications.infobases.ui_1.0.1003.v202609171902.jar` (SHA-256
`5d34e50d650ea38bab84d23f8d269987463c330d8987226b8469d525f96bd785`)
contains `LifecycleAwareApplicationsViewOpener.onProjectStart`: it checks
`PlatformUI.isWorkbenchRunning`, calls the plugin's `logError` and returns when
there is no workbench. The supported command-line workflow has no UI workbench;
this evidence does not implicate a forced headless flag in our command. The
read-only javap receipt, complete raw output and exact JAR hash are retained at
`logs/issue358-headless-vendor-inspection-001`; vendor class bodies are not
published. This identifies the error source, not a waiver or evidence that all
future headless errors are benign. Export and validation command results,
complete XML equality and this error are reported separately.

## Bounded nonempty current-version capture

The next shared capture uses an existing genuine native 8.5.1.1592 export of the
public synthetic `DcsFilterEvidence` configuration. The original `owned-create`
and `owned-full` commands completed successfully in the retained #362 laboratory;
#358 copied their evidence and all input bytes into
`fixtures/issue358-nonempty-source-001`. Native input is XML 2.21, 19 files /
86,112 bytes, tree SHA-256
`efc5814c4b31e7c8892511003e591fe0f0567a9f0d1d69e3a2594ada60b51a72`.
It has two nontrivial managed form XML bodies (19,444 / 8,283 bytes), catalogs and
a DCS report/template. Its 492-byte form BSL modules contain region stubs, so
this does not establish executable event-handler coverage.

To avoid duplicate preparation, #357 ran one genuine current EDT native import,
export and validation with actual Server VM. The installed-created project has
19 files / 79,140 bytes; its native XML export has 18 files / 82,953 bytes. These
are genuine installed EDT outputs, not a project synthesized by our converter.
The source validation TSV retains 24 Minor style diagnostics (UI/Code style),
zero Configuration-error rows and unclassified categories; no source-clean claim.
Raw fatal/headless diagnostics remain unresolved.

The same project then went through #356's fresh native file database creation,
import and export. That native reference has 19 files / 86,112 bytes, tree hash
`8628cfab808f7414be1e6e8912ff7912fe7c027b95ab4c303cf451d17141cc67`.
All native commands completed successfully. Source project, tool, installation
and schema evidence were unchanged in that reverse capture. Our independent
post-failure inventories also confirm unchanged native input, authentic project,
authentic export, plugin inventory and selected runtime payloads
(`logs/issue358-current-nonempty-postcheck-001`). The failed #357 capture lacked
its own final guards; this independent check is explicitly separate evidence,
not a claim that those missing terminal checks ran.

Both converter routes fail **closed before atomic output publication**:

- XML→EDT rejects the native non-default ClientApplicationInterface.xml
  `<top><panel ...>` layout where it only witnesses a default `<panelDef .../>`.
- EDT→XML rejects the actual installed ClientApplicationInterface.cai LF header
  where the pinned reader witnesses CRLF/default byte shape.

These concrete reports were handed to the codec owners for a model-preserving
fix. No sidecar was removed, no whitespace normalized, and no safety guard was
relaxed. The complete [nonempty accounting report](../evidence/edt-current-dcs-accounting.json)
retains every input/project/export hash row and both exact converter failure
reports. The independent native-versus-EDT pair contains 19 union rows: 16 byte
equal and three different/missing. That pair is not a three-way success: no
candidate output exists, so all candidate-dependent route results are blocked.
An empty or unrelated output is never substituted. Fresh captures remain at
`fixtures/issue357-current-server-dcs-001` and
`fixtures/issue356-current-server-dcs-001`; no BSP/UH acceptance is implied.

## Fixed nonempty candidate: forward and unchanged return

The subsequent `fixtures/issue357-current-server-dcs-002` capture uses the fixed
candidate built from source head `5e5b5bcdb2a2e56c685ea25d2893ac34734c1434`,
SHA-256 `6da274e498d6b21ef017c00bc8698cf6c14596d996604acce40a77bfe8431016`.
The immutable build receipt records Rust 1.95.0, two jobs, locked/offline build
and the exact executable hash. These are candidate-code evidence, separate from
the #358 capture-tool branch; this PR does not add those production codec fixes.
Historical failed capture reports above remain unchanged.

The installed serializer comparison now matches all 18 files exactly. Unchanged
return matches all 19 original native files, including ConfigDumpInfo.xml, with
original tree hash `efc5814c4b31e7c8892511003e591fe0f0567a9f0d1d69e3a2594ada60b51a72`.
All terminal tree/tool/installation/INI guards pass with empty audit errors.
Structured diagnostics match 24 versus 24 with no additions/removals (14 Code
style / 10 UI style rows); raw export/validation phase error multisets also match.
The strict result remains **FAIL**: the fatal/headless SDK records and
unclassified style categories have not been waived.

The new raw [installed forward](../evidence/edt-current-dcs-fixed/installed-forward-serializer.json)
and [unchanged return](../evidence/edt-current-dcs-fixed/unchanged-return.json)
reports each retain all 19 union paths. Forward has 16 all-equal and three
EDT=candidate-only rows; return has 16 all-equal and three native=candidate-only
rows. These are serializer/storage distinctions retained in the raw reports,
not normalized equivalence. Local command receipts, driver source, candidate
hash and input before/after inventories are retained at
`logs/issue358-three-way-current-dcs-fixed-001`.

The existing fresh native reference is reused only after full file/size/hash
inventory equality between its actual installed-EDT export input and this new
authentic export, unchanged native reference inventory, and successful prior
native create/import/export command receipts. That explicit binding is retained
in `native-reference-reuse.json`. No additional native database or JVM is needed
merely to repeat those identical bytes. This remains the legacy model exercised
against installed EDT 2026.2.1.3 on one public fixture, not unrestricted current
schema support or BSP/UH acceptance.

The independent fixed direct reader attempt is now captured at
`fixtures/issue356-current-server-dcs-fixed-direct-002`. Native-reference reuse
binding succeeded against complete project/export/native inventories, command
receipts and code/build hashes. The same fixed candidate then exited 2 before
atomic output because genuine current
`FilterProbe.ListForm/Attributes/Список/ExtInfo/ListSettings.dcss` uses LF while
the pinned DCSS reader requires CRLF. Every direct source/candidate/receipt guard
passed, and `ours-xml` does not exist. The earlier `fixed-direct-001` stopped at a
receipt-route check before conversion and is retained separately.

The [fixed accounting report](../evidence/edt-current-dcs-fixed/accounting.json)
records that exact next codec blocker, successful immutable native-reference
binding, the two complete raw reports, all terminal audits and the unchanged
strict diagnostic verdict. It explicitly sets `all_three_routes_completed`
and every unrestricted/current-schema/BSP/UH acceptance claim to false. No raw
direct report is produced from a missing, empty or older candidate. The next
DCSS lexical fix belongs to the shared codec owners; this capture-tool branch
preserves the reproduction and does not relax the CRLF guard itself.

## Published direct output after DCSS/DCSSCA fixes

The next actual direct run, `fixtures/issue356-current-server-dcs-fixed-direct-004`,
uses source candidate `c0cd767b`, executable SHA-256
`6340fd5458a19aa7e40a1208dd35bb29138d79313ce8d2534ad5aa458d2bf471`.
Its clean source/build receipt, focused tests, genuine-project/native-reference
reuse binding and all after guards pass. Conversion now exits 0 and publishes
18 XML files. Exact comparison still **fails**: both form Module.bsl bodies are
474 bytes versus the native 492, and report Template.xml is 3,189 versus 3,264.
The native-only 3,084-byte ConfigDumpInfo.xml remains the fourth strict row;
removing only that storage-inventory row still leaves three content differences.

The new [raw direct three-way report](../evidence/edt-current-dcs-direct004/direct-edt-to-xml.json)
retains all 19 union paths: 14 all-equal, two native=EDT-only, one native=candidate-only
and two EDT=candidate-only. The [direct accounting](../evidence/edt-current-dcs-direct004/accounting.json)
binds every full inventory, exact failed comparison, native reuse evidence,
command/build receipts and candidate identity. All three input trees, candidate
and bound receipts were rechecked unchanged. The offline oracle report alone
was run; no conversion, SDK, JVM or build was repeated. Raw local command/driver
evidence is `logs/issue358-current-dcs-direct004-oracle-001`.

This supplies a real direct report, but not an equality or acceptance result.
Earlier successful forward/return comparisons use candidate `6da274e4…`, not
this `6340fd54…` candidate; they are not promoted into a single-head three-route
success. All prior failures and diagnostic/style gates remain intact. No content
bytes were normalized, no older output substituted, and full BSP/UH acceptance
remains open.

## Direct005: exact native configuration-content match

The subsequent authorized direct-only capture,
`fixtures/issue356-current-server-dcs-fixed-direct-005`, uses source candidate
`e12f77a1`, executable SHA-256
`ba118d90b8e0736f08eb6d3d4e1a6f36fee3a57d04f91c68bbec24c92484b12a`.
Its renewed source/build/test/command/reference bindings and all immutable after
guards pass. Conversion exits 0 and all **18 configuration-content files match
native byte for byte**, including both form modules and the report template.
The candidate tree is
`bacc61842157fe2fc3ebd7a4f0ca78e2442ce1e5bf9e95472dd9e41004edad1b`.

The full file inventory still differs: the sole row is native-only
ConfigDumpInfo.xml, 3,084 bytes, SHA-256
`7301e7124e132fc5119c4a4c809b84187f475d8f89ec7a89a6085ea4e4447b7d`.
The established configuration-content comparison excludes only that native
storage inventory and reports zero content differences; the strict report
retains the original CDI row and remains unequal. No bytes are normalized.

The [direct005 raw report](../evidence/edt-current-dcs-direct005/direct-edt-to-xml.json)
retains all 19 union paths: 16 all equal, two native=candidate-only serializer
rows and one EDT=candidate-only native-CDI absence row. The
[accounting](../evidence/edt-current-dcs-direct005/accounting.json) binds the exact
successful comparison and complete source/native/EDT/candidate evidence.
Input trees, executable and bound receipts are unchanged; the only additional
operation was offline raw reporting at
`logs/issue358-current-dcs-direct005-oracle-001`. No conversion, SDK, JVM, native
database or build was repeated.

This is a direct content match on the public DcsFilterEvidence fixture. Earlier
forward/return reports use another candidate and cannot establish a same-head
three-route result for `ba118d90…`. Existing SDK/style errors remain unsuppressed;
strict SDK validation, unrestricted current model/schema support, both XML
profiles and full BSP/UH acceptance remain open. All earlier failed captures and
reports remain retained.
