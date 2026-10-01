# Installed EDT oracle laboratory

`scripts/edt-lab/oracle.py` is a laboratory tool, outside the production
dependency graph. It explicitly starts installed `1cedtcli.exe`. The product's
`convert` and `source-three-way-oracle` stay offline and never locate EDT/JVM.

## Evidence contract

The harness creates a new disposable directory on F:, refuses existing runs,
overlapping input/output trees, symlinks/junctions, case collisions, empty and
incomplete trees, and bounded-file/count/total-size violations. Every command
records the exact argument array, working directory, UTC start, duration,
timeout, exit code, and unmodified stdout/stderr bytes. It checks each command
separately: a multi-command EDT script can report only its last command's exit
code and cannot serve as acceptance. Timeouts kill only the process tree started
by the harness; failure evidence remains available.

Every installed EDT command and every product `convert` command, including the
unchanged return conversion, takes the existing shared FIFO `heavy` laboratory
lock and releases it in `finally`. Use a unique track name per active run. The
launcher gets a disposable workspace, explicit language and a recorded heap
(`--heap-gib`, default 8; UH preparation uses 32). No
original EDT workspace, database, global EDT preferences or XML corpus is edited.
Native inputs are hashed before and after with four bounded SHA readers and
cached directory enumeration metadata; manifests include every path, size
and SHA-256. Runs retain the exact harness source and SHA-256.
ConfigDumpInfo.xml is required for native XML; genuine EDT export
omits it, which remains explicit in every raw report. Imported projects must
have `.project`, `DT-INF/PROJECT.PMF`, a
configuration MDO and the explicitly requested `Runtime-Version`.

`prepare` runs actual EDT `version`, imports native XML with `--build true`, then
exports that authentic EDT project to XML. `PREPARED` means only that these
commands and completeness/immutability checks succeeded; it is not conversion
acceptance. Original native XML is an immutable independent input captured by
the native platform in earlier laboratory runs, not an ibcmd-rs round trip.
Acceptance additionally requires `--reference`: a fresh native export after
loading this authentic EDT project's exported XML into a disposable native
infobase. Its creation is a separate laboratory operation with fresh creation,
native import/export and database ownership evidence. Database activation
(`APPLY`) is an optional, separately recorded `--probe-apply` operation; a failed
activation probe remains visible and does not replace or invalidate successful
configuration import/export proof. This harness does not
create or modify a database. `accept` binds the adjacent `native_reference.py`
capture's successful result, every required command, exact build/version output,
executable and harness hashes, input-before/after manifests and current exported
tree. A copied XML directory without this native command chain is rejected.
Comparing only the pre-EDT original XML would mix
EDT's own materialization of defaults/order with converter errors.

`accept` checks those captured inputs again and uses the exact candidate binary
for three route-specific checks:

1. Authentic EDT, with no ibcmd-rs provenance, -> our XML must match the fresh
   post-EDT native SDK export. Only `ConfigDumpInfo.xml` is excluded, with original
   path/hash rows retained; EDT cannot invent native storage generations.
2. Native XML -> our EDT -> a disposable complete project copy with the entire
   `.ibcmd-provenance` directory removed -> installed EDT XML export must match
   the authentic installed-EDT XML baseline, using the same serializer. The
   harness does not add project scaffolding or restore cached native XML.
3. Native XML -> our EDT -> unchanged our XML must match every original file
   byte for byte, including `ConfigDumpInfo.xml`.

Both results enter existing `source-three-way-oracle`, together with native XML
and genuine EDT's export of its native import. The native branch uses the fresh
post-EDT reference, not the pre-import corpus. Reports must contain unique,
nonempty rows and complete matching summaries. Full three-way reports retain
all serializer divergences. Native-versus-EDT differences do not substitute for
the route-specific equality criteria. `PASS` requires all three criteria and
unchanged inputs/binary; no XML whitespace, namespaces, absent bodies or file
differences are normalized away. The raw report and sole direct-route CDI
exclusion retain their hashes.

Acceptance additionally binds `--validation-capture` to genuine structured
validation of this exact authentic project. Installed EDT validates the generated
project, and its exact diagnostic multiset must add no entries or multiplicities.
Only TSV timestamp and project-label columns are excluded; severity, category,
validator, object, position and message remain exact, including existing source
errors. Raw inherited diagnostics and added/removed rows remain available.

`--ambient-control` must be a completed installed-EDT empty project with zero TSV
source diagnostics, bound tool/harness/command hashes and unchanged template.
Only four evidenced record classes are candidates: the exact library descriptor
exception, absent headless workbench lifecycle event, and two severity-8 state
update records. Every plugin/severity/code/full message and stack must also match
the bound control. The report retains exact original/generated counts and their
differential. New or unmatched workspace errors, stdout ERROR/FATAL/exception
lines and additional structured diagnostics prevent PASS. There is no general
exception for a zero exit, shutdown errors or source errors.

## Commands

Exact CLI help was obtained from the installed tool, including status codes:
`import --version ... --configuration-files ... --project-name ... --build true`
and `export --project ... --configuration-files ...`. The latter imports an
existing EDT project automatically. `version` reports the exact installed build;
the profile names use the declared release series (for example `2025.2.3`), while
evidence records the complete build (`2025.2.3.30`). Launcher `-help` emits
UTF-16LE and actual EDT command output emits UTF-8; raw logs preserve both.
The installed `help validate` positively documents `validate --file TSV
--project-list PROJECT...`, which imports absent projects before checking them.
The separate `validate` mode copies a prepared authentic project, captures this
raw TSV and workspace logs, and checks that the original project remains intact.
Its `CAPTURED` status is an evidence capture, never a validation or acceptance PASS.
The TSV summary preserves its raw hash and every configuration-error row.
Unknown categories and malformed rows remain unresolved source diagnostics;
the standalone summary cannot silently call them clean. Acceptance compares the
complete baseline and generated multiset even for unfamiliar categories; malformed
rows are rejected.
The `control` mode uses the same CLI options and prepared template to create a
synthetic empty EDT project, retaining only root platform properties, contained
object identifiers and the inline language. It validates and exports that project
with the installed tool. This diagnostic control can establish precise recurring
environment log classes; it never substitutes for BSP/UH data or automatically
waives an error. Every original template byte remains unchanged.

```powershell
$edt = 'C:\Program Files\1C\1CE\components\1c-edt-2025.2.3+30-x86_64\1cedtcli.exe'
$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$native = 'F:\ibcmd\lab\v85\ibcmd_rs_bsp_85_src_20260922_20260922_bsp85_r2\native'

python scripts/edt-lab/oracle.py prepare `
  --native $native --source-version 2.21 --runtime 8.5.1 `
  --native-tool-version 8.5.1.1150 --edt-exe $edt --edt-version 2025.2.3 --edt-build 2025.2.3.30 `
  --run F:\ibcmd\lab\07\oracle-bsp85-example --lock-script $lock

python scripts/edt-lab/native_reference.py `
  --input F:\ibcmd\lab\07\oracle-bsp85-example\edt-native-xml `
  --run F:\ibcmd\lab\07\native-reference-bsp85-example `
  --database ibcmd_rs_04_edt07_bsp85_example `
  --ibcmd 'C:\Program Files\1cv8\8.5.1.1150\bin\ibcmd.exe' `
  --native-build 8.5.1.1150 `
  --restore-script F:\ibcmd\lab\04\tools\restore-clone.ps1 --lock-script $lock

python scripts/edt-lab/oracle.py validate `
  --native $native --source-version 2.21 --runtime 8.5.1 `
  --native-tool-version 8.5.1.1150 --edt-exe $edt --edt-version 2025.2.3 --edt-build 2025.2.3.30 `
  --prepared F:\ibcmd\lab\07\oracle-bsp85-example `
  --run F:\ibcmd\lab\07\validate-bsp85-example --lock-script $lock --heap-gib 32

python scripts/edt-lab/oracle.py control `
  --native $native --source-version 2.21 --runtime 8.5.1 `
  --native-tool-version 8.5.1.1150 --edt-exe $edt --edt-version 2025.2.3 --edt-build 2025.2.3.30 `
  --prepared F:\ibcmd\lab\07\oracle-bsp85-example `
  --run F:\ibcmd\lab\07\empty-control-example --lock-script $lock --heap-gib 16

python scripts/edt-lab/oracle.py accept `
  --native $native --source-version 2.21 --runtime 8.5.1 `
  --native-tool-version 8.5.1.1150 --edt-exe $edt --edt-version 2025.2.3 --edt-build 2025.2.3.30 `
  --prepared F:\ibcmd\lab\07\oracle-bsp85-example `
  --reference F:\ibcmd\lab\07\native-reference-bsp85-example\native-xml `
  --validation-capture F:\ibcmd\lab\07\validate-bsp85-example `
  --ambient-control F:\ibcmd\lab\07\empty-control-example `
  --ours-exe F:\ibcmd\lab\07\target-root\debug\ibcmd-rs.exe `
  --run F:\ibcmd\lab\07\accept-bsp85-example --lock-script $lock
```

For ERP UH use the independently captured native reference
`F:\ibcmd\lab\v85\native\uha_20260923\native` and distinct run names. For
8.3.27 explicitly choose XML `2.20` and runtime `8.3.27`; the harness rejects a
profile/runtime mismatch. Native tool version is caller-supplied provenance of
the historical native capture, not a claim that this harness launched ibcmd.

Negative evidence controls run with
`python tests/scripts/edt_oracle_test.py`. These synthetic tests verify failure
handling and evidence integrity only; they never count as BSP/UH acceptance.

## Actual captures

On 2026-10-01 the installed tool positively reported `2025.2.3.30`. Exact help
and launcher output are preserved under
`F:\ibcmd\lab\07\edt-discovery`; the native BSP85 import with `--build true`
completed with exit 0 under `F:\ibcmd\lab\07\oracle-bsp85-r1`.
The separate genuine validation capture under
`F:\ibcmd\lab\07\validate-bsp85-r1` completed with exit 0 but reported five
`Major / Configuration error` rows in the ordinary application module, concerning
execution environments and compilation. The raw TSV and derived summary are
retained. The UH85 import under `F:\ibcmd\lab\07\oracle-uha85-r2` also logged
`Unsupported version of file MobileDigiSign.bin` in the installed EDT workspace.
The source TSV errors remain preserved baseline diagnostics; generated projects
must not add to them. The unsupported MobileDigiSign runtime error is not an
approved ambient class. The actual empty EDT control under
`F:\ibcmd\lab\07\empty-edt-control83-r1` emitted zero TSV rows while reproducing
the four exact environment record classes described above. Its source inventory
then showed EDT had added `.settings/org.eclipse.core.resources.prefs`, so that
capture fails the strict control immutability binding. The replacement
`F:\ibcmd\lab\07\empty-edt-control83-r2` copies the authentic template's encoding
settings before taking the source snapshot. Its genuine validate/export completed,
TSV is empty and all source/template/export inventories pass binding. The earlier
evidence remains intact.

The genuine UH83 capture under `F:\ibcmd\lab\07\oracle-uha83-r1` completed
build-enabled import and export, with unchanged native input. Its native input
is a fresh SHA-256 verified mirror of the historical E: native SDK export;
`F:\ibcmd\lab\07\uha83-native-mirror-evidence` preserves every file hash and
the original export command. The final workspace logs also contain the
unsupported mobile-signature diagnostic and a BSL index transaction error.
Choosing the 8.3.27 profile alone therefore does not make this corpus clean.
These records remain outside the ambient control classes and block strict
acceptance; the full UH83 structured validation is captured separately.

A separate read-only investigation under
`F:\ibcmd\lab\07\mobile-signature-reader-probe` reproduces the mobile-signature
tail guard with the installed `TextListInStream` class in a 64 MiB Java process.
The actual 44-byte source is a version-2 text list with four empty digest groups.
The installed `readDigestData` zero-count branch consumes two list terminators;
on the fourth group it also consumes the enclosing list's terminator. The later
version-2 guard then sees the comma before the converted flag and reports the
unsupported-version error. Source before/after hashes, plugin hashes, bytecode,
probe source and exact command/trace are preserved. An old basic version-0
literal is only a grammar control: rewriting the genuine version-2 artifact to
that schema would discard typed/converted fields. This investigation neither
changes the source nor relaxes the acceptance gate.

The follow-up bounded sandbox examined 3,736 short synthetic version-2 inputs
using the genuine installed text-list reader. It preserved empty key/digest,
four zero-count leaf groups and the false converted flag while varying zero
spelling, whitespace and balanced containers. All 2,429 inputs that reached the
tail check failed it; the other inputs were rejected earlier. No supported
variant was found. Actual installed model-factory inspection confirms that the
version-0 basic object and version-2 converted object have different EClasses;
the synthetic version-0 project was prepared but never submitted to EDT. Raw
serialization of a constructed full version-2 model succeeds but differs from
the native file's framing, so it is not an exact native acceptance result.
`bounded-variant-search-result.json` preserves the candidate set, command,
model/source/plugin hashes and unchanged-source proof. The sandbox stopped
without an adaptation or broader diagnostic waiver.

Acceptance binds the empty control's current TSV to its captured summary and
SHA-256, its template before/after/current inventories, its source before/current
inventory and its installed-export inventory. New captures additionally record
the source after inventory. Command evidence must name the exact disposable
workspace, project and export destination. Malformed or unknown workspace error
records remain unmatched errors instead of disappearing during parsing.

The source issue remains subject to the completed BSP and ERP UH acceptance
reports, review of diagnostics and complete differences. Neither an import
success, upstream's corpus assertions nor unit tests close that requirement.
