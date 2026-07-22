# ibcmd-rs

Research-first Rust tool for building a replacement path for slow 1C configuration
loads from XML sources.

The first milestone started read-only for the target infobase:

- locate 1C command-line tools;
- scan source trees into deterministic manifests;
- compare manifests and produce a load plan;
- profile external `ibcmd` or `1cv8` runs;
- generate SQL Server and 1C technical log trace templates.
- group exported SQL Server Extended Events XML by normalized SQL text.

Guarded direct database writers are now available for researched storage rows.
They are still intended only for throwaway test databases.

## Standalone Conversion Direction

The planned XML/CF converter is a standalone Rust path with no production
runtime dependency on `ibcmd`, `1cv8`/Designer, EDT, or a JVM. Its independent
version axes, capability levels, opaque-data rules, and fail-closed loss policy
are defined in the
[standalone conversion contract](docs/architecture/standalone-core.md).

### Standalone Converter Roadmap Progress

<!-- offline-converter-progress: completed=35 total=56 updated=2026-07-22 -->

As of 2026-07-22, 35 of 56 accepted leaf issues in the
[standalone converter epic #178](https://github.com/Untru/ibcmd-rs/issues/178)
are complete (62.5%). Live workflow statuses are tracked in
[GitHub Project #5](https://github.com/users/Untru/projects/5). This is
issue-count roadmap progress, not codec or compatibility coverage, and it is
separate from the export parity metrics below.

CF fixture work is paused until project-owned, independently validated
Format15/Format16 artifacts are available; platform-independent migration and
compiler work continues on separate roadmap lanes.

CF-005 is complete independently of those unavailable golden containers. The
CF payload layer now distinguishes `Stored` from a single complete raw-DEFLATE
stream, requires `StreamEnd` and full encoded-input consumption, and rejects
truncated streams, trailing bytes, aggregate depth/count/byte overrun, and
excessive expansion ratios through typed errors. Decoding uses fixed-size codec
buffers and commits its shared resource budget only after complete validation;
synthetic corruption and decompression-bomb regressions run without 1C, EDT,
or a JVM. See [bounded CF payload evidence](docs/evidence/cf-payload-limits.md).

BOOT-002 is paused only on the native `Configuration` body: the bounded
identity/inventory graph and deterministic `root`, `version`, and `versions`
codecs are implemented, while the unproven seven-section body remains
fail-closed. BOOT-003 is complete. Its six vertical slices, `Constant`,
`Language`, `SessionParameter`, `DefinedType`, `FunctionalOption`, and
`FunctionalOptionsParameter`, support XML 2.20/2.21 -> typed IR ->
deterministic base-free native row -> strict native IR/XML under independently
selected 8.3.27 family layouts. Readable `Location`, `Content`, and `Use`
references are resolved only through the validated canonical graph;
type-pattern `cfg:*` values resolve only through canonical generated TypeIds.
The core now also retains generated `ValueId` as a typed optional identity
(legacy canonical JSON remains readable; semantic digest domain v2). The
`Constant` codec preserves all three generated TypeId/ValueId pairs, its single
type pattern and `UseStandardCommands`; unknown native shell fields and
unevidenced qualifier mappings fail closed.
All evidenced textual BOOT-003/BOOT-004 native rows include the required UTF-8
BOM followed by deterministic CRLF text; serializers and strict decoders share
that profile-level byte contract.

BOOT-004 is complete. All seven service-family slices, `ScheduledJob`,
`EventSubscription`, `HTTPService`, `WebService`, `IntegrationService`,
`WSReference`, and `XDTOPackage`, now
support XML 2.20/2.21 -> typed IR -> deterministic base-free native row ->
strict native IR/XML. `HTTPService` includes its nested URL templates and methods, with
explicit reversible mappings for every retained session-reuse and HTTP-method
code. `WebService` adds deterministic operation/parameter trees, XDTO qualified
types, all three transfer directions, and graph-resolved `XDTOPackage` links.
`IntegrationService` retains Manager TypeId/ValueId identities for the service
and every channel, and strictly maps `Send`/`Receive`, handlers, and transaction
flags through the evidenced channel collection shell.
`WSReference` retains its Manager TypeId/ValueId pair and LocationURL while its
WSDL definition remains an independent source asset.
CommonModule handlers, event-source TypeIds, and package UUIDs are resolved only
through the validated canonical graph; independently evidenced 8.3.27 layouts
reject unknown or reordered native fields. Service `Ext/Module.bsl` files and
`XDTOPackage/Ext/Package.bin` remain separate source assets rather than fields
of these metadata rows.

BOOT-005 is complete for the strict evidenced Catalog/Document slice. Both
families now support typed XCF roots, generated types, direct attributes,
tabular sections with nested attributes, commands, and graph-resolved
form/template references. Their independently selected 8.3.27 layouts compile
deterministic UTF-8-BOM/CRLF raw-DEFLATE rows without reading a base artifact.
Minimal and child-rich fixtures verify exact embedded versus separately routed
identity inventories; unknown markers, unresolved references, unsupported
complex properties, and undeclared future layouts fail closed. The retained
layout evidence and current boundary are documented in
[Catalog and Document native layout evidence](docs/evidence/business-objects-8.3.27.md).

BOOT-006 is complete for `Report`, `DataProcessor`, `Enum`, and
`SettingsStorage`. Their strict XCF 2.20/2.21 codecs retain roots, generated
TypeId/ValueId pairs, ordered form/template references, attributes, tabular
sections, commands, and enum values. The independently selected 8.3.27 layouts
compile deterministic base-free rows and validate them through a bounded native
parser; root/collection shape, standard-attribute descriptors, identity
uniqueness, and child order fail closed. All four emitted semantic native trees
were also matched against independently inflated laboratory rows. See
[utility-object native layout evidence](docs/evidence/utility-objects-8.3.27.md).

BOOT-007 is complete for `Subsystem`, `ExchangePlan`, `BusinessProcess`, and
`Task`. Their strict XCF 2.20/2.21 codecs retain subsystem content and nested
subsystems, exchange-plan children, workflow tables and routes, generated
TypeId/ValueId pairs, and task addressing dimensions. The independently
selected 8.3.27 layouts compile deterministic base-free rows and validate them
through a bounded native parser. Ownership, reference targets, child order,
duplicate identities, standard-field inventories, and unsupported native tails
all fail closed. See
[hierarchical/workflow native layout evidence](docs/evidence/hierarchical-workflow-8.3.27.md).

BOOT-008 is complete for all four register families, `Recalculation`, and the
three chart families. Strict XCF 2.20/2.21 codecs feed independently selected
8.3.27 native layouts with exact owner sizes, collection markers, standard
fields, and generated identities. `Recalculation` keeps semantic ownership
while declaring its independently stored native row; its Dimension targets are
validated against the owning CalculationRegister. Compact source files that
omit required defaults and unevidenced embedded register fields remain
fail-closed. See
[register/chart native layout evidence](docs/evidence/registers-charts-8.3.27.md).

BOOT-009 is complete for `CommonModule`, `CommonCommand`, `CommandGroup`,
`CommonPicture`, module bodies, pictures, Help and raw binary assets. Strict XCF
2.20/2.21 projections feed profile-selected 8.3.27 metadata layouts; module and
asset codecs preserve exact text/content bytes through bounded compression and
strict decoding. A single source-asset registry now owns family, semantic role,
native suffix, relative path and codec mappings for both the standalone
compiler and the legacy MSSQL bridge. Unknown properties, unresolved graph
references, unsafe asset names, malformed base64 and future layouts fail
closed. See [module/command/asset native layout evidence](docs/evidence/modules-commands-assets-8.3.27.md).

BOOT-010 is complete as a bounded, fail-closed body-codec slice. Role Rights
now has a profile-selected base-free encoder plus a strict decoder shared with
the MSSQL export bridge. Catalog Predefined data compiles from typed nested
items without a base row; the three chart layouts are preserved only as exact
same-profile opaque bytes until their family-specific schemas are evidenced.
Parent-configuration, standalone-content and mobile-signature artifacts are
classified as opaque support data: they require observed bytes, provenance and
a matching SHA-256, never cross profiles implicitly, and can only be dropped
through an explicit policy that emits a loss report. Source paths and native
suffixes for Rights/Predefined/support data are centralized in the same asset
registry. See [Rights/Predefined/support evidence](docs/evidence/rights-predefined-support-8.3.27.md).

BOOT-011 is complete for the evidenced 8.3.27 managed Form and
CommandInterface layouts. `Form.xml` plus optional `Module.bsl` now compile
without a base row into a deterministic UTF-8-BOM marker-50 body with typed
root properties, nested controls, attributes, parameters, commands and local
command references. The strict decoder requires the four native trailing
sections; the representative element matrix is exported back to XML and
checked semantically. CommandInterface owns all five ordered native sections
and emits raw-reference XML through the same bounded typed codec. Unsupported
Form facets, embedded item assets, incomplete readable command tuples, legacy
markers and future profiles remain explicit blockers. The legacy MSSQL bridge
uses the shared decoders and now bypasses a base fetch for supported Form
bodies. See [Managed Form/CommandInterface evidence](docs/evidence/forms-command-interface-8.3.27.md).

BOOT-012 is complete for all eight recognized Template kinds. A typed,
profile-gated dispatcher now owns MXL, DCS, HTML, binary and raw XML/text body
framing. Spreadsheet staging emits the strict MOXCEL marker-8 layout without
reading a base row; DCS emits the evidenced 24-byte header and three BOM-XML
documents, including semantic extraction/reinsertion of non-empty Settings.
HTML nested files and AddIn/BinaryData payloads preserve exact bytes. Unknown
template kinds, native tails, unsafe nested names, unsupported DCS area
templates and future profiles fail closed. The legacy export bridge delegates
MXL/DCS framing to the same bounded readers. See
[Template body evidence](docs/evidence/template-bodies-8.3.27.md).

| Phase | Completed | Progress |
|---|---:|---:|
| Phase 0 baseline/boundaries | 4/4 | 100% |
| Phase 1 version profiles/core models | 10/10 | 100% |
| Phase 2 XCF | 6/6 | 100% |
| Phase 3 CF | 2/15 | 13.3% |
| Phase 4 bootstrap | 11/13 | 84.6% |
| Phase 5a migrations | 2/4 | 50% |
| Phase 5b app/release | 0/4 | 0% |
| **Overall** | **35/56** | **62.5%** |

## Export Compatibility Status

Current parity tracking is maintained in
[docs/export-parity-status.md](docs/export-parity-status.md). The table below is
the compact top-level view from the latest full `ut_ibcmd` export comparison
against native `ibcmd`.
Recommended parallel-agent ownership by object family is tracked in
[docs/metadata-agent-slicing.md](docs/metadata-agent-slicing.md).
The Form.xml mapping workflow is tracked in
[docs/form-diff-matrix.md](docs/form-diff-matrix.md).

EDT XML exporter/importer plugin findings are summarized in
[docs/edt-xml-layer-analysis.md](docs/edt-xml-layer-analysis.md). The short
version: use the EDT XML layer as a reference for ordering, file layout and
import hierarchy, not as a production runtime dependency.

| Object / group | Status | Ready | Remaining |
|---|---|---:|---:|
| CommandGroups, CommonCommands, CommonModules, CommonPictures, Constants, DefinedTypes, DocumentNumerators | done / byte-identical | 100.0% | 0 |
| EventSubscriptions | partial | 0.0% | 312 |
| Ext | partial | 0.0% | 15 |
| FunctionalOptions | partial | 0.0% | 567 |
| FunctionalOptionsParameters | partial | 0.0% | 9 |
| IntegrationServices | partial | 0.0% | 2 |
| Languages | partial | 0.0% | 1 |
| ScheduledJobs | partial | 0.0% | 402 |
| SessionParameters | partial | 0.0% | 98 |
| StyleItems | partial | 0.0% | 400 |
| WSReferences | partial | 0.0% | 2 |
| XDTOPackages | partial | 0.0% | 814 |
| Enums | partial | 97.3% | 32 |
| CommonTemplates | partial | 83.6% | 81 |
| Reports | partial | 0.0% | 2362 |
| AccumulationRegisters | partial | 59.9% | 180 |
| Roles | partial | 0.0% | 2220 |
| ExchangePlans | partial | 0.0% | 366 |
| HTTPServices | partial | 0.0% | 10 |
| WebServices | partial | 0.0% | 36 |
| DocumentJournals | partial | 59.5% | 49 |
| DataProcessors | partial | 77.7% | 1572 |
| Tasks | partial | 0.0% | 49 |
| Documents | partial | 52.0% | 2987 |
| Catalogs | partial | 52.0% | 3217 |
| ChartsOfCharacteristicTypes | partial | 48.5% | 86 |
| InformationRegisters | partial | 0.0% | 3978 |
| CommonAttributes | partial | 42.9% | 4 |
| BusinessProcesses | partial | 43.4% | 86 |
| SettingsStorages | partial | 0.0% | 82 |
| CommonForms | partial | 60.7% | 439 |
| Subsystems | partial | 0.0% | 766 |
| FilterCriteria | partial | 0.0% | 7 |
| Configuration.xml | partial | 0.0% | 1 |
| **Overall full snapshot** | **partial** | **56.9%** | **21369** |

Current live `ut_ibcmd` scoped roundtrip queue status, verified alongside the
latest full snapshot and tracked separately from the full-export parity table above:

| Scoped sweep slice | Result |
|---|---|
| Narrowed representative queue (`CommonAttributes`, `FilterCriteria`, `Subsystems`, `CommonForms`, `Reports`, `DataProcessors`) | 100.0% green (`6 / 6` prefixes) |
| Default-family representative pass `#1` | 100.0% green (`9 / 9` generated prefixes) |
| Default-family representative pass `#2` | 100.0% green (`9 / 9` generated prefixes) |

Note: these percentages describe the current direct MSSQL scoped roundtrip
queue on the reference database `Srvr="localhost";Ref="ut_ibcmd"`. They are
not a replacement for the full-snapshot export parity percentages above, which
must only change after a fresh full diff is regenerated.

Latest full-run timing on July 3, 2026 (`full_diff_20260703_102243_zeroemptyrow1`):
`40,576` rows / `927,826,268` bytes (`~884.8 MiB`) exported in `124.419 s`
on the dump critical path (`process_rows_wall`), which is `326.12 rows/s` and
`7.11 MiB/s` overall. Raw BCP fetch took `198.186 s` (`4.46 MiB/s`).

Latest full `DataProcessors` scoped verification on July 6, 2026
(`selected_dp_full_20260706_0026_command_interface_release`): `5,737` rows /
`39,889,431` bytes (`~38.0 MiB`) exported in `12.863 s` on the dump critical
path, which is `446.01 rows/s` and `2.96 MiB/s` overall. Raw BCP fetch took
`1.089 s` (`34.93 MiB/s`). The resulting `source-diff` summary still stands at
`1572 different / 5486 unchanged`, with `0 left-only / 0 right-only`
(`77.7274%`, `5486 / 7058`). The same retained run reports
`fetch_headers_ms=7976`, `prepare_indexes_ms=40783`,
`prepare_metadata_fetch_ms=20589`, `fetch_rows_ms=1089`,
`process_rows_wall_ms=12863`, and `source_asset_cpu_ms=58406`; the hottest
source-asset CPU buckets are now `form_xml=34033`, `child_items=13928`, and
`help=10325`. Latest signature mining on the same retained run keeps
`document/rowsItem/row/c/c/f=464` concentrated in `1` file
(`РаботаСНоменклатурой/Templates/ПФ_MXL_КарточкаНоменклатуры`), while the
largest remaining color tails are `document/format/borderColor=368` and
`document/format/backColor=324`; the leading form tails are now
`Form/Commands/Command/Picture=234`, `Form/ChildItems/Table/CommandSet/ExcludedCommand=211`,
and `Form/CommandInterface/CommandBar/Item=140`. This rerun keeps the same
headline parity but lowers the `Form/CommandInterface/CommandBar/Item` tail
from `195` across `25` files to `140` across `18` files by falling back from a
fixed `body.trailing[3]` lookup to the first real form `CommandInterface`
container.

Latest verified slices:

| Round | Area | Verified progress |
|---|---|---|
| 110 | DataProcessors scoped verification / timing | fresh selected export `0026_command_interface_release` keeps `DataProcessors` at `1572` different / `5486` unchanged with `0` left-only / `0` right-only after falling back from a fixed `body.trailing[3]` slot to the first real form `CommandInterface` container; this does not close additional files yet, but it reduces the `Form/CommandInterface/CommandBar/Item` tail from `195` across `25` files to `140` across `18` files and lands at `12.863 s` critical path (`446.01 rows/s`, `2.96 MiB/s`) |
| 109 | DataProcessors scoped verification / timing | fresh selected export `1947_filter_picture_release` keeps `DataProcessors` at `1572` different / `5486` unchanged with `0` left-only / `0` right-only after remapping filter-command standard picture UUIDs back to `StdPicture.FilterCriterion`, `StdPicture.FilterByCurrentValue`, and `StdPicture.ClearFilter`; this does not close additional files yet, but it reduces the `Form/Commands/Command/Picture` tail from `248` across `97` files to `234` across `95` files and lands at `22.574 s` critical path (`254.15 rows/s`, `1.69 MiB/s`) |
| 108 | DataProcessors scoped verification / timing | fresh selected export `1735_root_order_release` improves `DataProcessors` to `1572` different / `5486` unchanged with `0` left-only / `0` right-only after restoring native root-form `CommandBarLocation` / `CommandSet` order and moving `CustomizeForm` back into the standard dialog command order; this closes `6` more files, shifts the leading form tail to `Form/ChildItems/Table/CommandSet/ExcludedCommand=268`, and lands at `16.920 s` critical path (`339.07 rows/s`, `2.25 MiB/s`) |
| 107 | DataProcessors scoped verification / timing | fresh selected export `144331_event_uuid_release` improves `DataProcessors` to `1578` different / `5480` unchanged with `0` left-only / `0` right-only after remapping confirmed form-event UUID aliases back to native event names, keeps the dominant template `document/rowsItem/row/c/c/f` signature at `464` in `1` file, preserves the leading form tail at nested `UsualGroup/.../Width=307`, and lands at `28.909 s` critical path (`198.45 rows/s`, `1.32 MiB/s`) |
| 106 | DataProcessors scoped verification / timing | fresh selected export `141710_readme_refresh_release` reconfirms `DataProcessors` at `1580` different / `5478` unchanged with `0` left-only / `0` right-only after a full current-release rerun, keeps the dominant template `document/rowsItem/row/c/c/f` signature at `464` in `1` file, shifts the leading form tail to nested `UsualGroup/.../Width=307`, and lands at `28.129 s` critical path (`203.95 rows/s`, `1.35 MiB/s`) |
| 105 | DataProcessors scoped verification / timing | fresh selected export `122017_purchase_line_release` improves `DataProcessors` to `1580` different / `5478` unchanged with `0` left-only / `0` right-only after remapping leading `f527` palette-offset MXL style refs and expanding the two-entry `{None, Solid}` line table into the native three-width solid set, which closes `ОбменСБанками/Templates/ПоручениеНаПокупкуВалюты/Ext/Template.xml`, `ОбменСБанками/Templates/ПоручениеНаПереводВалюты/Ext/Template.xml`, and `ОбменСБанками/Templates/РаспоряжениеНаОбязательнуюПродажуВалюты/Ext/Template.xml`, keeps `document/rowsItem/row/c/c/f` at `464` in `1` file, and lands at `29.071 s` critical path (`197.34 rows/s`, `1.31 MiB/s`) |
| 104 | DataProcessors scoped verification / timing | fresh selected export `110226_slotfix_release` improves `DataProcessors` to `1583` different / `5475` unchanged with `0` left-only / `0` right-only after resolving MXL style-ref slots `{-1}` / `{-3}` to `FormBackColor` / `FormTextColor` instead of dropping them to compact-color fallback, which removes the broad `ToolTipBackColor` misclassification tail, keeps `document/rowsItem/row/c/c/f` at `464` in `1` file, and lands at `12.943 s` critical path (`443.25 rows/s`, `2.94 MiB/s`) |
| 103 | DataProcessors scoped verification / timing | fresh selected export `101148_eisheader_release` improves `DataProcessors` to `1584` different / `5474` unchanged with `0` left-only / `0` right-only after making single-set sparse MXL header/footer refs prefer the explicit external source slot ahead of the `font=0,width=72` sparse-default heuristic, which closes `ЭлектронноеАктированиеЕИС/Templates/ЭД_ПриложениеЕИС_ru`, drops the dominant template `document/rowsItem/row/c/c/f` cluster from `872` across `2` files to `464` in `1` file, and lands at `11.219 s` critical path (`511.36 rows/s`, `3.39 MiB/s`) |
| 102 | DataProcessors scoped verification / timing | fresh selected export `093104_contracttypical_release` improves `DataProcessors` to `1585` different / `5473` unchanged with `0` left-only / `0` right-only after restoring single-set shared-empty MXL default handling without reopening the `UKD` / `СоответствияПараметровИМеток` cases, adding a new baseline regression for `ФорматДоговорнойДокумент101XML/Templates/ПереченьТиповыхНаименованийЭлементовДоговоров`, and closing `ОбменСКонтрагентами/Templates/УКД_ИнформацияПокупателя_2020` plus `ФорматДоговорнойДокумент101XML/Templates/СоответствияПараметровИМеток`; latest signature mining now concentrates `document/rowsItem/row/c/c/f` at `872` across `2` files and lands at `19.508 s` critical path (`294.08 rows/s`, `2.05 MiB/s`) |
| 101 | DataProcessors scoped verification / timing | fresh selected export `080819_true_scope_release` improves `DataProcessors` to `1587` different / `5471` unchanged with `0` left-only / `0` right-only after batching `sqlcmd` row-header fetches to avoid SQL Server `8623`, resolving sparse shared-header/default MXL slots through explicit source refs only when no native sparse-default format exists, and suppressing the shifted row-level `formatIndex=2` tails that appear after the leading shared default reorder; this closes `ПечатьОтчетовПоКомиссии/Templates/ПФ_MXL_ОтчетПоКомиссии`, `ПечатьОтчетовПоКомиссии/Templates/ПФ_MXL_ОтчетПоКомиссииОСписании`, and `ФорматДоговорнойДокумент101XML/Templates/ПереченьТиповыхНаименованийЭлементовДоговоров`, drops the dominant template `document/rowsItem/row/c/c/f` cluster from `1041` (`4` files) to `594` (`3` files), and lands at `11.498 s` critical path (`498.96 rows/s`, `3.31 MiB/s`) |
| 100 | DataProcessors scoped verification / timing | fresh selected export `055344_compactstyle_release` improves `DataProcessors` to `1592` different / `5466` unchanged with `0` left-only / `0` right-only after teaching compact MOXEL style-ref indices to resolve directly to tooltip/form/field colors when the style-ref table omits those slots; this closes the remaining invoice template tails in `ПФ_MXL_СчетФактура451_ru`, `ПФ_MXL_СчетФактура981_ru`, `ПФ_MXL_СчетФактура1137_ru`, and `ПФ_MXL_СчетФактура1137_625_ru`, and lands at `12.925 s` critical path (`443.87 rows/s`, `2.94 MiB/s`) |
| 99 | DataProcessors scoped verification / timing | fresh selected export `052811_exactslot_release` improves `DataProcessors` to `1603` different / `5455` unchanged with `0` left-only / `0` right-only after reading declared sparse-sheet height from the live MXL column-set block, restoring leading `{129,0,width}` default-width detection for sparse source-slot templates without reclassifying the whole width table, and preferring exact existing `font=0 + width` default-format matches before normalized width-only fallbacks; this closes two more files on top of the previous retained run, fixes the `СчетФактура451` `defaultFormatIndex=102` regression while preserving the `1096` and `request_offer` native baselines, and lands at `11.417 s` critical path (`502.50 rows/s`, `3.33 MiB/s`) |
| 98 | DataProcessors scoped verification / timing | fresh selected export `023103_zero_offset_sparse_release` improves `DataProcessors` to `1609` different / `5449` unchanged with `0` left-only / `0` right-only after detecting zero-offset sparse MXL source-format refs, splitting the format table by real source refs, remapping row/cell refs on that path, and recomputing the fallback default-format slot after remap; this closes `ОбменСКонтрагентами/Templates/ПользовательскоеПредставлениеОбязательныхПолей/Ext/Template.xml`, drops the dominant `document/rowsItem/row/c/c/f` signature from `2029` to `1679` (`9 -> 7` files), and lands at `16.033 s` critical path (`357.82 rows/s`, `2.37 MiB/s`) |
| 97 | DataProcessors scoped verification / timing | fresh selected export `225500_zero_colfmt_release` improves `DataProcessors` to `1610` different / `5448` unchanged with `0` left-only / `0` right-only after reusing matching MXL default formats instead of always appending a synthetic tail format and preserving zero-valued MXL column `formatIndex`; this closes `5` more files on top of the previous retained run, drops the dominant `document/rowsItem/row/c/c/f` signature from `3001` to `2857` (`17 -> 9` files), and collapses `ЗаявлениеНаВыпускНовогоКвалифицированногоСертификата/Templates/ЮридическоеЛицо/Ext/Template.xml` from `143` differences to `25`; latest release timing is `15.075 s` critical path (`380.56 rows/s`, `2.52 MiB/s`) |
| 96 | DataProcessors scoped verification / timing | fresh selected export `011557_command_no_action_release` reconfirms `DataProcessors` at `1615` different / `5443` unchanged with `0` left-only / `0` right-only after keeping form `Command` entries with empty `Action` and omitting empty `<Action>` tags; this does not close additional files yet, but narrows `Form/Commands/Command/Picture` from `269` to `252` and trims representative form diffs such as `БизнесСеть/ПрофильУчастника` (`24 -> 18`) and `БизнесСеть/ОтправкаПриглашенийКонтрагентам` (`116 -> 49`); latest release timing is `12.438 s` critical path (`461.25 rows/s`, `3.06 MiB/s`) |
| 95 | DataProcessors scoped verification / timing | fresh selected export `010452_change_row2_release` improves `DataProcessors` to `1615` different / `5443` unchanged with `0` left-only / `0` right-only after refining wrapper-55 ordinary-table `ChangeRowOrder=false` heuristics; this closes `2` more files on top of the radio-button work and lands at `11.079 s` critical path (`517.83 rows/s`, `3.43 MiB/s`) |
| 94 | DataProcessors scoped verification / timing | fresh selected export `004400_radio_release` improves `DataProcessors` to `1617` different / `5441` unchanged with `0` left-only / `0` right-only after adding `RadioButtonField` extraction plus `ChoiceList`, `RadioButtonType`, and `ColumnsCount` support for managed forms; latest release timing is `15.515 s` critical path (`369.76 rows/s`, `2.45 MiB/s`) |
| 93 | DataProcessors scoped verification / timing | fresh selected export `000112_dp_child_order_release` improves `DataProcessors` to `1619` different / `5439` unchanged with `0` left-only / `0` right-only after restoring wrapped-child `EditFormat`, preserving boolean `{"U"}` fill values as `xsi:nil`, separating `TabularSection.FillChecking` from the `LineNumber` standard attribute, and ordering top-level `DataProcessor` attributes before tabular sections; this closes `18` more files and lands at `19.862 s` critical path (`288.84 rows/s`, `1.92 MiB/s`) |
| 92 | DataProcessors scoped verification / timing | fresh selected export `220341_dp_childobjects_release` improves `DataProcessors` to `1637` different / `5421` unchanged with `0` left-only / `0` right-only after rebuilding release and emitting explicit empty `DataProcessor/ChildObjects` and empty child `Attribute/Type` nodes, which closes `29` files on top of the earlier template palette and dynamic-list fixes; latest release timing is `25.825 s` critical path (`222.15 rows/s`, `1.47 MiB/s`) |
| 91 | DataProcessors scoped verification / timing | fresh selected export `214500_listsettings_rework` improves `DataProcessors` to `1666` different / `5392` unchanged with `0` left-only / `0` right-only after reworking dynamic-list `ListSettings` normalization, restoring implicit defaults only where the native body omits them, and reading `Appearance` / `GroupSelectedSettingId`-backed settings without reintroducing wrong default IDs; latest release timing is `14.799 s` critical path (`387.66 rows/s`, `2.57 MiB/s`), and the old `Form/.../ListSettings/...` signature family drops out of the current top block entirely |
| 90 | DataProcessors scoped verification / timing | fresh selected export `2110_regfix` improves `DataProcessors` to `1668` different / `5390` unchanged with `0` left-only / `0` right-only after narrowing the `LabelDecoration.AutoMaxWidth` and table `UseAlternationRowColor` fallbacks, fixing the three-form regression cluster while preserving `РедактированиеБланка`; latest release timing is `22.234 s` critical path (`258.03 rows/s`, `1.71 MiB/s`) |
| 89 | DataProcessors scoped verification / timing | fresh selected export `220025_valuetreefont` improves `DataProcessors` to `1670` different / `5388` unchanged with `0` left-only / `0` right-only after restoring built-in form attribute types `v8ui:Font` and `v8:ValueTree` on top of the latest `SpreadsheetDocument` / bare-number / `v8ui:Color` fixes; latest signature mining keeps `document/rowsItem/row/c/c/f=3001`, shifts the next template color tails to `document/format/backColor=337` and `document/format/textColor=327`, and pushes the old `Form/Attributes/Attribute/Type` cluster out of the current top signature block; latest release timing is `16.624 s` critical path (`345.10 rows/s`, `2.29 MiB/s`) |
| 88 | DataProcessors scoped verification / timing | fresh selected export `195904_btnchoice` improves `DataProcessors` to `1671` different / `5387` unchanged with `0` left-only / `0` right-only after restoring button `AutoMaxWidth` / `MaxWidth` and input-field `ChoiceFoldersAndItems`; latest signature mining keeps `document/rowsItem/row/c/c/f=3001`, `document/format/textColor=1188`, and reduces the remaining form-level table `ExcludedCommand` gap to `876` combined occurrences (`82` files); latest release timing is `13.647 s` critical path (`420.39 rows/s`, `2.79 MiB/s`) |
| 87 | DataProcessors scoped verification / timing | fresh selected export `192500_wrapper55` reconfirms `DataProcessors` at `1673` different / `5385` unchanged with `0` left-only / `0` right-only, but narrows the remaining form-level `Form/ChildItems/Table/CommandSet/ExcludedCommand` signature cluster from `964` to `948` occurrences (`94 -> 92` files) after broadening ordinary wrapper-55 table detection, restoring 8-command table `CommandSet` variants, and emitting table row-picture metadata; latest release timing is `13.809 s` critical path (`415.45 rows/s`, `2.75 MiB/s`) |
| 86 | DataProcessors scoped verification / timing | fresh selected export `185000_checkall` improves `DataProcessors` to `1673` different / `5385` unchanged with `0` left-only / `0` right-only after restoring more form/root `CommandSet` patterns, wrapper-55 table `ExcludedCommand` variants, and `UncheckAll` standard-command binding; latest release timing is `12.632 s` critical path (`454.16 rows/s`, `3.01 MiB/s`) |
| 85 | DataProcessors scoped verification / timing | fresh selected export `003500_f527prop` reconfirms `DataProcessors` at `1680` different / `5378` unchanged with `0` left-only / `0` right-only after property-aware `f527...` tooltip/form/field color resolution and the latest retained MXL color checks; latest release timing is `15.548 s` critical path (`368.99 rows/s`, `2.45 MiB/s`) |
| 84 | DataProcessors scoped verification / timing | fresh selected export `233500_mxlpictext` improves `DataProcessors` to `1680` different / `5378` unchanged with `0` left-only / `0` right-only after preserving embedded MXL picture payloads, `textPosition`, `widthWeightFactor`, negative format heights, and row-level `columnsID` ordering; latest release timing is `15.584 s` critical path (`368.13 rows/s`, `2.44 MiB/s`) |
| 83 | DataProcessors scoped verification / timing | fresh selected export `190500_clientbank_settlement` improves `DataProcessors` to `1683` different / `5375` unchanged with `0` left-only / `0` right-only after restoring standalone MXL default-format tails, `detailsUse=WithoutProcessing`, non-integer font heights, indexed style overrides / `auto`, and aggregating multiple merge-region lists so both `КлиентБанк/Templates/ОтчетОЗагрузке/Ext/Template.xml` and `ЗаполнениеРегистровВзаиморасчетов/Templates/Макет/Ext/Template.xml` become byte-identical; latest release timing is `13.401 s` critical path (`428.10 rows/s`, `2.84 MiB/s`) |
| 82 | DataProcessors scoped verification / timing | fresh selected export `140030_hiddenmerge` improves `DataProcessors` to `1700` different / `5358` unchanged with `0` left-only / `0` right-only after preserving explicit empty `format` / `editFormat`, `hidden=false`, legacy `Bottom` alignment, wrapped MXL style refs, and narrowing the sparse default-format heuristic so `request_offer` becomes byte-identical; latest release timing is `16.896 s` critical path (`339.55 rows/s`, `2.25 MiB/s`) |
| 81 | DataProcessors scoped verification / timing | fresh selected export `113500_emptycols` reconfirms `DataProcessors` at `1703` different / `5355` unchanged with `0` left-only / `0` right-only after restoring sparse-source MXL absolute font flags, `{-11}` line-style handling, and removing the extra sparse row/cell `formatIndex` `+1` remap; signature mining collapses template `document/rowsItem/row/c/c/f` from `65299` to `3001`; latest release timing is `13.170 s` critical path (`435.61 rows/s`, `2.89 MiB/s`) |
| 80 | DataProcessors scoped verification / timing | fresh selected export `113500_emptycols` improves `DataProcessors` to `1703` different / `5355` unchanged with `0` left-only / `0` right-only after restoring native sparse source-slot MXL column format selection, `request_offer`/`load_goods` default-format placement, and `ReportHeaderBackColor` color normalization; latest release timing is `11.978 s` critical path (`478.96 rows/s`, `3.18 MiB/s`) |
| 79 | DataProcessors scoped verification / timing | fresh selected export `113500_emptycols` improves `DataProcessors` to `1706` different / `5352` unchanged with `0` left-only / `0` right-only after normalizing `LockOwnerWindow`, `BeforeRowChange`, and additional table `ExcludedCommand` patterns; latest release timing is `14.977 s` critical path (`383.05 rows/s`, `2.54 MiB/s`) |
| 78 | Full snapshot / timing | fresh snapshot `102243_zeroemptyrow1` captures the current tree at `12499` left-only / `8871` different / `28253` unchanged, with `DataProcessors` improved to `1709` different / `5349` unchanged; latest release timing is `124.419 s` critical path (`326.12 rows/s`, `7.11 MiB/s`) |
| 77 | DataProcessors scoped verification / timing | fresh selected export `113500_emptycols` improves `DataProcessors` to `1709` different / `5349` unchanged with `0` left-only / `0` right-only after emitting zero-column-slot row `formatIndex=1` and preserving native empty default/picture slot handling; latest release timing is `22.464 s` critical path (`255.39 rows/s`, `1.69 MiB/s`) |
| 76 | DataProcessors scoped verification / timing | fresh selected export `092028_mxlrowmap2` improves `DataProcessors` to `1710` different / `5348` unchanged with `0` left-only / `0` right-only after restoring sparse source-derived MXL row/cell remap, `BorderColor` slot 0, `WindowsFont`, and native spreadsheet XML ordering; latest release timing is `12.042 s` critical path (`476.42 rows/s`, `3.16 MiB/s`) |
| 75 | DataProcessors scoped verification / timing | fresh selected export `085640_mxlslots` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only after preserving source-derived MXL style-slot ordering and native default format tails; latest release timing is `14.397 s` critical path (`398.49 rows/s`, `2.64 MiB/s`) |
| 74 | Full snapshot / timing | fresh snapshot `081405_sourcefix` reconfirms overall parity at `12226` different / `37396` unchanged, reconfirms `Reports` at `437` remaining, and reconfirms `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `83.685 s` critical path (`484.87 rows/s`, `10.57 MiB/s`) |
| 73 | DataProcessors scoped verification / timing | fresh selected export `081017_17rc17lo5` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only after remapping source-derived MXL column format refs; latest release timing is `13.050 s` critical path (`439.62 rows/s`, `2.92 MiB/s`) |
| 72 | DataProcessors scoped verification / timing | fresh selected export `013406_zerosize` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only after preserving zero-sized default MXL column sets; latest release timing is `14.493 s` critical path (`395.85 rows/s`, `2.62 MiB/s`) |
| 71 | DataProcessors scoped verification / timing | fresh selected export `012824_negcol` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only after accepting negative MXL column indexes; latest release timing is `14.045 s` critical path (`408.47 rows/s`, `2.71 MiB/s`) |
| 70 | Full snapshot / timing | fresh snapshot `010516_vg` reconfirms overall parity at `12226` different / `37396` unchanged, reconfirms `Reports` at `437` remaining, and reconfirms `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `86.186 s` critical path (`470.80 rows/s`, `10.27 MiB/s`) |
| 69 | DataProcessors scoped verification / timing | fresh selected export `005936_vg` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only after the `vg` MXL roundtrip fix; latest release timing is `13.730 s` critical path (`417.84 rows/s`, `2.77 MiB/s`) |
| 68 | DataProcessors scoped verification / timing | fresh selected export `003832_editformat` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only after the `editFormat` MXL roundtrip fix; latest release timing is `12.405 s` critical path (`462.47 rows/s`, `3.07 MiB/s`) |
| 67 | Full snapshot / timing | fresh snapshot `001609` improves overall parity to `12226` different / `37396` unchanged, improves `Reports` to `437` remaining, and reconfirms `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `86.359 s` critical path (`469.85 rows/s`, `10.25 MiB/s`) |
| 66 | DataProcessors scoped verification / timing | fresh selected export `001157` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only; latest release timing is `13.193 s` critical path (`434.85 rows/s`, `2.88 MiB/s`) |
| 65 | Full snapshot / timing | fresh snapshot `235827` reconfirms overall parity at `12227` different / `37395` unchanged, `Reports` at `438` remaining, and `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `86.750 s` critical path (`467.73 rows/s`, `10.20 MiB/s`) |
| 64 | DataProcessors scoped verification / timing | fresh selected export `235010` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only; latest release timing is `13.324 s` critical path (`430.58 rows/s`, `2.86 MiB/s`) |
| 63 | Full snapshot / timing | fresh snapshot `232306` reconfirms overall parity at `12227` different / `37395` unchanged, `Reports` at `438` remaining, and `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `86.922 s` critical path (`466.81 rows/s`, `10.18 MiB/s`) |
| 62 | Full snapshot / timing | fresh snapshot `230839` reconfirms overall parity at `12227` different / `37395` unchanged, `Reports` at `438` remaining, and `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `86.967 s` critical path (`466.57 rows/s`, `10.17 MiB/s`) |
| 61 | DataProcessors scoped verification / timing | fresh selected export `230510` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only; latest release timing is `13.293 s` critical path (`431.58 rows/s`, `2.86 MiB/s`) |
| 60 | Full snapshot / timing | fresh snapshot `2252236` reconfirms overall parity at `12227` different / `37395` unchanged, `Reports` at `438` remaining, and `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `89.698 s` critical path (`452.36 rows/s`, `9.86 MiB/s`) |
| 59 | DataProcessors scoped verification / timing | fresh selected export `222918` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only; latest release timing is `13.722 s` critical path (`418.09 rows/s`, `2.77 MiB/s`) |
| 58 | Full snapshot / timing | fresh snapshot `214826` updates overall parity to `12227` different / `37395` unchanged, reconfirms `Reports` at `438` remaining, and captures `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `89.419 s` critical path (`453.77 rows/s`, `9.90 MiB/s`) |
| 57 | DataProcessors scoped verification / timing | fresh selected export `214301` reduces `DataProcessors` to `1714` different / `5344` unchanged with `0` left-only / `0` right-only; latest release timing is `15.957 s` critical path (`359.53 rows/s`, `2.38 MiB/s`) |
| 56 | Full snapshot / timing | fresh snapshot `212531` updates overall parity to `12238` different / `37384` unchanged, reconfirms `Reports` at `438` remaining, and reconfirms `DataProcessors` at `1725` different / `5333` unchanged; latest release timing is `90.265 s` critical path (`449.52 rows/s`, `9.80 MiB/s`) |
| 55 | DataProcessors scoped verification / timing | fresh selected export `212150` reduces `DataProcessors` to `1725` different / `5333` unchanged with `0` left-only / `0` right-only; latest release timing is `14.474 s` critical path (`396.37 rows/s`, `2.63 MiB/s`) |
| 54 | DataProcessors scoped verification / timing | fresh selected export `211339` reduces `DataProcessors` to `1745` different / `5313` unchanged with `0` left-only / `0` right-only; latest release timing is `12.541 s` critical path (`457.46 rows/s`, `3.03 MiB/s`) |
| 53 | Full snapshot / timing | fresh snapshot `205558` updates overall parity to `12264` different / `37358` unchanged, reconfirms `Reports` at `438` remaining, and reconfirms `DataProcessors` at `1751` different / `5307` unchanged; latest release timing is `89.677 s` critical path (`452.47 rows/s`, `9.87 MiB/s`) |
| 52 | DataProcessors scoped verification / timing | fresh selected export `2015` updates `DataProcessors` to `1751` different / `5307` unchanged with `0` left-only / `0` right-only; latest release timing is `13.309 s` critical path (`431.06 rows/s`, `2.86 MiB/s`) |
| 51 | Full snapshot / timing | fresh snapshot `185626` reconfirms overall parity at `12307` different / `37315` unchanged, `Reports` at `438` remaining, and `DataProcessors` at `1789` different / `5269` unchanged; latest release timing is `82.381 s` critical path (`492.54 rows/s`, `10.74 MiB/s`) |
| 50 | Full snapshot / timing | fresh snapshot `0022` updates overall parity to `12307` different / `37315` unchanged, improves `Reports` to `438` remaining, and improves `DataProcessors` to `1789` different / `5269` unchanged; latest release timing is `91.909 s` critical path (`441.48 rows/s`, `9.63 MiB/s`) |
| 49 | Full snapshot / timing | fresh snapshot `0021` reconfirms overall parity at `10276` different / `39346` unchanged, `Reports` at `505` remaining, and `DataProcessors` at `3042` different / `4016` unchanged; latest release timing is `133.197 s` critical path (`304.63 rows/s`, `6.64 MiB/s`) |
| 48 | Full snapshot / timing | fresh snapshot `0020` reconfirms overall parity at `10276` different / `39346` unchanged, `Reports` at `505` remaining, and `DataProcessors` at `3042` different / `4016` unchanged; latest release timing is `128.107 s` critical path (`316.74 rows/s`, `6.91 MiB/s`) |
| 47 | Full snapshot / timing | fresh snapshot `0018` reconfirms overall parity at `10276` different / `39346` unchanged, `Reports` at `505` remaining, and `DataProcessors` at `3042` different / `4016` unchanged; latest release timing is `122.111 s` critical path (`332.29 rows/s`, `7.25 MiB/s`) |
| 46 | Full snapshot / timing | fresh snapshot `0017` reconfirms overall parity at `10276` different / `39346` unchanged, `Reports` at `505` remaining, and `DataProcessors` at `3042` different / `4016` unchanged; latest release timing is `133.466 s` critical path (`304.02 rows/s`, `6.63 MiB/s`) |
| 45 | Full snapshot / timing | fresh snapshot `0016` updates overall parity to `10276` different / `39346` unchanged, `Reports` to `505` remaining, and `DataProcessors` to `3042` different / `4016` unchanged; latest release timing is `246.829 s` critical path (`164.39 rows/s`, `3.58 MiB/s`) |
| 44 | Full snapshot / timing | fresh snapshot `0015` reconfirms overall parity at `8956` different / `40666` unchanged and DataProcessors at `1789` different / `5269` unchanged; latest release timing is `127.389 s` critical path (`318.52 rows/s`, `6.95 MiB/s`) |
| 43 | Form.xml | property-bag `UsualGroup` items now emit native `Behavior=Usual`, `Representation`, and non-default `Group`; default root `Form/Group=Vertical` is suppressed |
| 43 | Form.xml commands | command `Representation` / `CurrentRowUse` now derive from native command flags, picture presence, and action/name shape instead of over-emitting defaults |
| 42 | Form.xml tooling | `form-diff-candidates` compares controlled Form.xml/blob pairs and suggests `XML path -> layout path` mappings for matrix-driven pack/unpack tests |
| 42 | Form.xml | newly synthesized `Table` child items now preserve XML `<DataPath>` in the native layout |
| 42 | MXL templates | MOXCEL vertical alignment code `48` now exports and packs as `Bottom` |
| 42 | Catalog metadata XML | root create/history tail properties now come from native fields instead of fixed defaults |
| 42 | Source staging readiness | metadata XML base dependency audit reports direct property counts and child-object kind breakdowns |
| 42 | Refactor | selected-export planning helpers moved from `src/mssql_dump/mod.rs` to `src/mssql_dump/selected.rs` |
| 41 | Form.xml | new `CheckBoxField` items now pack explicit `ShowInHeader` through the extended layout shape |
| 41 | MXL templates | MOXCEL number-format string tables now export and pack `document/format/format/v8:item` references |
| 41 | Workflow metadata XML | `BusinessProcess` and `Task` now emit `UseStandardCommands` from the native owner field |
| 41 | Source staging readiness | `Predefined.xml` base dependency audit reports exact row, nesting, editable-field and native-shape blockers |
| 41 | Refactor | SQL/BCP fetch helpers moved from `src/mssql_dump/mod.rs` to `src/mssql_dump/fetch.rs` with focused fetch/timing checks |
| 40 | Configuration.xml | root child object tags now reuse the shared metadata classifier for Languages, XDTOPackages, SettingsStorages, ScheduledJobs, CommandGroups, Styles and DocumentNumerators in source XML 2.20/2.21 |
| 40 | Catalog metadata XML | `QuickChoice` and `ChoiceMode` are extracted from native root fields with backward-compatible defaults |
| 40 | Form.xml | `TextDocumentField` now exports explicit `ReadOnly=true` through the shared child-item path |
| 40 | Source staging readiness | sectionless `Form.xml` bodies keep the existing-base requirement, but now report the exact native container skeleton blocker |
| 40 | Native dump performance | timing summary output now exposes sorted source-asset and Form CPU breakdowns; latest saved full-run evidence still points at Form XML reconstruction CPU, not BCP/direct fetch |
| 39 | Form.xml | default `Table` child item `SkipOnInput=false` is suppressed while explicit `true` remains exported |
| 39 | MXL templates | MOXCEL system style refs `-25`, `-26`, `-27`, `-34`, `-35`, `-36`, `-37`, and `-38` now map to native report/table/button style color refs |
| 39 | InformationRegister metadata XML | extended owner tuple field emits `DataLockControlMode` for `InformationRegister` as well as `AccumulationRegister` |
| 39 | Role Rights.xml | role rights object ordering can follow metadata tree order while preserving serialized order within one owner |
| 39 | Source staging readiness | Form body base-dependency audit now reports precise counts for root/layout scalars, child items, trailing sections and `Ext/Form/Items/**` assets |
| 38 | Form.xml | default `WindowOpeningMode=DontBlock` is suppressed in export while explicit XML still packs into the form layout |
| 38 | MXL templates | native empty format slots `{0}` are preserved, so later width-bearing formats are no longer dropped |
| 38 | MXL templates | mixed `NamedItemCells` / `NamedItemDrawing` lists now preserve valid named areas without packing bogus drawing areas |
| 38 | Object metadata XML | Document and Report child attributes now reuse shared property-tail extraction, including `DataHistory` after `ChoiceForm` |
| 38 | Subsystems | metadata scalar tail now emits `IncludeHelpInContents`, `IncludeInCommandInterface`, and native `UseOneCommand` |
| 38 | Source staging readiness | `XDTOPackages/*/Ext/Package.bin` is covered as base-free raw-deflated staging and source-load coverage |
| 37 | Form.xml | dynamic-list `Settings/Field` entries now export `dataPath`/`field` and pack back into the serialized settings bag |
| 37 | MXL templates | `textPlacement=Cut` now extracts from MOXCEL code `1` and packs back to the same format bit |
| 37 | MXL templates | non-1-based column `formatIndex` values are preserved separately from the normalized internal indexes used for row/cell decoding |
| 37 | Configuration.xml | root `CommonPicture` child objects are classified by native field shape and emitted in `Configuration.xml` for source XML 2.20/2.21 |
| 37 | Object metadata XML | child attribute `ChoiceForm` tails now preserve empty values and resolved non-empty design-time refs |
| 37 | Source staging readiness | root `Ext/MainSectionPicture.xml` is covered as base-free staging and source-load coverage |
| 36 | Diff mining | reusable `source-diff-signatures` CLI added; bounded scan of the last full diff sampled 2,101 XML pairs and shows the largest remaining clusters are still MXL `document/format/*` and `formatIndex` signatures |
| 36 | Form.xml | default `Page/ScrollOnCompress=false` is suppressed while explicit `true` remains exported and packable |
| 36 | Flowchart.xml | BusinessProcess/GraphicalSchema `Font` now decodes serialized style font tuples for connection lines |
| 36 | MXL templates | spreadsheet `textOrientation` format bit 13 extracts to XML and packs back into MOXCEL bodies |
| 36 | CommonAttributes | native property-tail `FillChecking` now emits `ShowError` / `DontCheck` after `FillValue` in source XML order |
| 36 | Source staging readiness | `CommonCommand/Ext/CommandInterface.xml` raw command-interface rows are covered as base-free staging |
| 35 | Diff mining | sampled XML-path mining report added to drive agents by repeated signatures instead of object families |
| 35 | Form.xml | default `ShowCommandBar=true` is suppressed while explicit `false` and explicit packer values remain supported |
| 35 | Object metadata XML | Catalog child attributes now reuse generic property-tail parsing with resolved `ChoiceParameters` refs |
| 35 | Flowchart.xml | BusinessProcess/GraphicalSchema item `ZOrder` exports from serialized item order instead of hardcoded zero |
| 35 | MXL templates | unknown MOXCEL format bits no longer drop the whole format table; known neighboring format properties and indexes are preserved |
| 35 | Source staging readiness | root configuration application modules under `Ext/*.bsl` have explicit base-free readiness and row-generation coverage |
| 35 | Role Rights.xml | form refs resolve and pack in role rights, including owned refs such as `Catalog.<name>.Form.<form>` |
| 34 | Source staging readiness | `Ext/ParentConfigurations.bin` now stages base-free by re-deflating inflated raw-deflated source bytes into the `ConfigSave` row |
| 34 | Object metadata XML | Catalog owner refs are emitted as ordered `xr:MDObjectRef` items in `<Owners>` |
| 34 | DCS templates | calculated-field `TypeId` normalization uses the native `d4p1` current-config namespace context |
| 34 | Form.xml | wrapper `55` table `RowFilter xsi:nil="true"` extracts and packs through property-bag key `10` |
| 34 | ExchangePlan Content.xml | content items are ordered by configuration metadata tree order with blob order as fallback |
| 34 | CommonAttributes | property-tail `FillValue` emits string, nil, decimal and boolean XML values between fill-from and fill-checking fields |
| 33 | Role Rights.xml | HTTPService URL template method refs resolve as `HTTPService.<service>.URLTemplate.<template>.Method.<method>` and pack back from source XML |
| 33 | DCS templates | unqualified DCS core `xsi:type` values normalize to `dcscor:*`; data-core `StandardPeriod` / `StandardPeriodVariant` normalize to `v8:*` |
| 33 | Form.xml | wrapper `55` table `Period` extracts and packs through property-bag key `7` as `v8:StandardPeriodVariant=Custom` |
| 33 | Object metadata XML | DataProcessor child metadata emits non-empty `ChoiceParameters`, resolving design-time UUIDs to stable refs and fixed arrays |
| 33 | ExchangePlan metadata XML | optional `xr:ThisNode` is emitted in `InternalInfo`, and `UseStandardCommands` is read relative to the detected header slot |
| 33 | Source staging readiness | module `.bin` source assets containing inflated V8 containers can stage base-free for common/object/nested module bodies |
| 32 | Native dump performance / Form.xml CPU | consolidated form child-item support index traversal; full source-layout after-run completed without `ConfigDumpInfo.xml`, with `source_asset_form_child_items_cpu_ms` reduced from 68,886 to 40,598 and `source_asset_form_xml_cpu_ms` from 118,673 to 102,296 |
| 32 | Form.xml | wrapper `55` table `RestoreCurrentRow` extracts and packs through property-bag key `12` |
| 32 | Object metadata XML | Document owner `<IncludeHelpInContents>` is emitted after default/auxiliary form properties |
| 32 | ExchangePlan metadata XML | code-4/code-27 child attributes are emitted with value types and property tails |
| 32 | Configuration.xml | localized root information fields are emitted (`BriefInformation`, `DetailedInformation`, copyright and information addresses) |
| 32 | Source staging readiness | unsupported `AdditionalIndexes.xml` families now report a precise base-blob blocker instead of being silently omitted |
| 31 | Native dump performance / Form.xml CPU | full source-layout after-run completed without `ConfigDumpInfo.xml`; `InputField` option-bag parsing is cached per form item, reducing `source_asset_form_child_items_cpu_ms` from 70,750 to 68,886 and `source_asset_form_cpu_ms` from 124,212 to 123,507 |
| 31 | Role Rights.xml | Task child refs now map and pack as `Task.<name>.AddressingAttribute.<child>` instead of ordinary attributes |
| 31 | Form.xml | wrapper `55` table `ChoiceFoldersAndItems` extracts and packs through existing typed table slot |
| 31 | Object metadata XML | tabular-section property tails are emitted, and child property tails now include empty `<LinkByType/>` between `ChoiceForm` and `ChoiceHistoryOnInput` |
| 31 | Register metadata XML | register dimensions/resources/attributes reuse generic child object parsing and emit decoded types plus common property tails |
| 31 | Configuration.xml | root settings-storage refs are emitted from UUID-backed metadata fields |
| 31 | DCS templates | generated type id lookup is case-insensitive for DataCompositionSchema template bodies |
| 31 | Source staging readiness | WebService `Ext/Module.bsl` is covered as a base-free source staging body |
| 30 | Native dump performance / ExchangePlan Content.xml | selected ExchangePlan content now requests `object_refs`; selected and full source-layout exports completed without `ConfigDumpInfo.xml`, with full timing captured under `E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-full-source-report.json` |
| 30 | Role Rights.xml | import packing maps rights objects by source-resolved refs instead of XML order, including commands, child dimensions/resources/attributes, standard attributes, and direct metadata refs |
| 30 | Form.xml | wrapper `55` table `RowPictureDataPath` extracts and packs through string property-bag key `19` |
| 30 | Object metadata XML | child `Attribute` tail properties now include choice/use/indexing/full-text/data-history scalar details |
| 30 | Object metadata XML | Report/DataProcessor child commands emit command property tails from owner metadata blobs |
| 30 | Register metadata XML | InformationRegister and AccumulationRegister standard attributes are emitted; AccumulationRegister type value `1` now maps to `Turnovers` |
| 30 | CommonAttributes | native separation tail properties and UUID-backed separation refs are emitted |
| 30 | DCS templates | current-config `v8:Type` prefixes normalize by DCS context (`d4p1`, `d5p1`, `d6p1`) |
| 30 | Source staging readiness | FilterCriterion manager modules can stage base-free from `FilterCriteria/<Name>/Ext/ManagerModule.bsl` |
| 29 | Native dump performance / ExchangePlan Content.xml | source asset extraction now builds source-layout refs without metadata XML; ExchangePlan content failures report the exact unsupported metadata id, with the next blocker isolated to a missing Constant ref index |
| 29 | Role Rights.xml | object rights preserve serialized object order in export and pack paths |
| 29 | Form.xml | wrapper `55` table `DefaultItem=true` extracts and packs through property-bag key `11` |
| 29 | Object metadata XML | child `Attribute` property tails emit format/edit/fill/history details from native metadata blobs |
| 29 | Register metadata XML | AccumulationRegister `DataLockControlMode` and `FullTextSearch` owner properties are emitted |
| 29 | CommonAttributes | native property detail defaults are emitted before `Content` for source XML 2.20/2.21 |
| 29 | MXL templates | default-only spreadsheet `printSettings` blocks are suppressed to match native source output |
| 29 | Source staging readiness | `versions` rows now report precise base-free blockers instead of a generic bootstrap note |
| 28 | Native dump performance | standalone-content refs resolve without `--extract-metadata-xml`; full timing now advances to an ExchangePlan `Content.xml` blocker |
| 28 | Role Rights.xml | string-backed restriction fields export/import through `<restrictionByCondition><field>` while preserving base layout |
| 28 | Form.xml | wrapper `55` table `UserSettingsGroup` extracts and packs existing numeric property-bag slots |
| 28 | Object metadata XML | Catalog/DataProcessor child attribute type refs and Report auxiliary-variant placeholder behavior improved |
| 28 | Register/ExchangePlan metadata/assets | AccumulationRegister `RegisterType` and ExchangePlan `Content.xml` BOM/final-newline framing covered |
| 28 | CommonAttributes | content item `xr:ConditionalSeparation` refs are resolved from native settings records |
| 28 | DCS templates | selected template owners build `type_index` so DCS TypeIds can normalize in selected/source-only flows |
| 28 | Source staging readiness | metadata XML rows now report precise base-dependent blockers for incomplete native metadata compilation |
| 27 | Role Rights.xml | nested subsystem and integration service channel refs resolve to fuller native-style child object names |
| 27 | Form.xml | wrapper `55` table items extract and pack existing `AllowGettingCurrentRowURL` boolean slots |
| 27 | Object metadata XML | Report command child headers and Catalog/DataProcessor Attribute/TabularSection child headers are emitted |
| 27 | Workflow/Register metadata XML | BusinessProcess/Task generated types and AccumulationRegister `IncludeHelpInContents=false` are emitted |
| 27 | Configuration.xml | root `<DefaultRoles>` is emitted from resolved role refs in metadata field `39` |
| 27 | DCS templates | TypeId normalization can use `xr:GeneratedType` entries from XML-shaped metadata text |
| 27 | Source staging readiness | configuration `Ext/HomePageWorkArea.xml` has explicit base-free readiness classification |
| 27 | Native dump performance | `mssql-dump-timing-summary` added; selected 664 MB blob timing run confirmed 3 BCP batches and `fetch_rows_ms=3532` |
| 26 | Native dump performance | native `bcp` row batch cap reduced to 256 MiB and timing JSON reports batch count/max size diagnostics |
| 26 | Role Rights.xml | role object refs use UUID plus serialized tail fields, preserving repeated owner refs and standard-attribute refs |
| 26 | Form.xml | wrapper `55` table items extract and pack observed `UpdateOnDataChange=Auto` through property bag key `14` |
| 26 | Object metadata XML | Catalog standard attribute labels and DataProcessor object/presentation scalars are parsed from metadata blobs |
| 26 | Register/ExchangePlan metadata XML | AccumulationRegister and ExchangePlan generated type `InternalInfo` coverage expanded |
| 26 | Configuration.xml | root `DefaultStyle` and `DefaultLanguage` refs are emitted from UUID-backed metadata fields |
| 26 | DCS templates | observed DCS body `AnyIBRef`/`TypeSet` and settings `xsi:type` values are canonicalized |
| 26 | Source staging readiness | readable `CommandInterface.xml` refs now report precise base-free blockers while raw refs remain base-free |
| 25 | Role Rights.xml | explicit disabled `false` rights without restrictions are preserved in export and pack paths |
| 25 | Form.xml | wrapper `55` table items extract and pack `UseAlternationRowColor` through the existing property bag |
| 25 | Object metadata XML | `Document` metadata emits numbering settings, standard command flag and default form refs |
| 25 | InformationRegister metadata XML | `DefaultRecordForm` and `DefaultListForm` refs are resolved from owner metadata form indexes |
| 25 | Configuration.xml | root scalar application/compatibility properties are emitted in native-shaped metadata XML |
| 25 | MXL templates | SpreadsheetDocument text nodes keep literal quotes while still escaping XML structural characters |
| 25 | Source staging readiness | `Predefined.xml` readiness reports precise base-blob blockers for row order, parent/type slots and trailing fields |
| 24 | Role Rights.xml | role rights objects are exported and packed against base slots in object UUID order |
| 24 | Form.xml | table child items extract and pack `SkipOnInput`, including wrapper `55` layouts |
| 24 | Object metadata XML | `Document` metadata emits `StandardAttributes` with number-type-aware `Number` fill value |
| 24 | CommonAttributes metadata XML | native `Content` metadata/use pairs are parsed, including `use_flag=2` as `DontUse` |
| 24 | DCS templates | DataCompositionSchema template bodies rewrite known `v8:TypeId` values to source-style `v8:Type` references through the metadata type index |
| 24 | ExchangePlan Content.xml | `AutoRecord` mapping is corrected to `0=Deny`, `1=Allow`, `2=Auto` |
| 24 | Source staging readiness | `BusinessProcess/Ext/Flowchart.xml` readiness reports precise base-blob blockers for item order, type slots, events and nested payload shape |
| 23 | Form.xml | wrapper `55` table items extract and pack `AutoRefresh` / `AutoRefreshPeriod` through existing property bag keys |
| 23 | Object metadata XML | `Document` metadata emits owned `Form` and `Template` child refs from generic indexes |
| 23 | Partial metadata XML | `InformationRegister` reads `UseStandardCommands` from the extended native owner tuple |
| 23 | MXL templates | SpreadsheetDocument MOXCEL merge regions preserve `verticalUnmerge` instead of converting it to ordinary `merge` |
| 23 | Source staging readiness | `Role/Ext/Rights.xml` readiness reports precise base-blob blockers instead of a generic reason |
| 23 | Native dump performance | `mssql-dump-config` routes blob-bearing metadata/direct prepare reads through the existing native `bcp` parser and reports `sqlcmd`/`bcp` timing split |
| 22 | Form.xml | `Command/ModifiesSavedData=true` is extracted and packed for form commands |
| 22 | Root/CommonAttributes metadata XML | root `Configuration.xml` child families expanded; `CommonAttribute` emits native-shaped `Content` items and `AutoUse` enum values |
| 22 | Source staging readiness | form body readiness audit now reports precise base-blob blockers for layout, trailing sections, modules and item assets |
| 22 | V8 container layer | module blob V8 container parser/builder moved into a shared tested internal module |
| 21 | Form.xml | nested `AutoCommandBar` items preserve `HorizontalAlign` and `Autofill` settings |
| 21 | Object metadata XML | `DataProcessor` emits `UseStandardCommands` from the metadata blob |
| 21 | Source staging readiness | configuration `Ext/MobileClientSignature.bin` rows are covered as base-free raw-deflated bodies |
| 20 | Form.xml | new `UsualGroup` items preserve extended `Behavior`, `Representation`, `ShowTitle=false`, and nested children |
| 20 | Partial metadata XML | register-family metadata emits `UseStandardCommands` from the metadata blob |
| 20 | Source staging readiness | configuration `Ext/StandaloneConfigurationContent.bin` rows are covered as base-free raw-deflated bodies |
| 19 | Form.xml | new `InputField` items preserve extended options (`Width`, `HorizontalStretch`, `AutoMaxWidth`, `MaxWidth`) |
| 19 | Partial metadata XML | `Subsystem` emits `UseStandardCommands` from the metadata blob |
| 19 | Source staging readiness | `WSReference` definition rows are covered as base-free raw-deflated bodies |
| 18 | Form.xml | new top-level `Button` items preserve explicit `DefaultButton` values |
| 18 | Partial metadata XML | `ExchangePlan` emits `UseStandardCommands` from the metadata blob |
| 18 | Configuration.xml | root `Catalog` child object headers are emitted |
| 18 | Source staging readiness | root configuration raw-id `MainSectionCommandInterface.xml` rows are covered as base-free |
| 17 | Form.xml | new `InputField` items preserve explicit `SkipOnInput` values |
| 17 | Partial metadata XML | `Task` generated type `InternalInfo` entries are emitted |
| 17 | MXL templates | SpreadsheetDocument `style:FieldSelectionBackColor` is supported in pack path |
| 17 | Source staging readiness | root configuration raw-id `CommandInterface.xml` rows are covered as base-free |
| 16 | Form.xml | new `LabelField` items preserve explicit `ShowInHeader` values |
| 16 | Partial metadata XML | `AccountingRegister` and `CalculationRegister` generated type `InternalInfo` entries are emitted |
| 16 | Configuration.xml | root `Constant` child object headers are emitted |
| 16 | Source staging readiness | raw-id `CommandInterface.xml` rows can be prepared without active `Config` blobs |
| 15 | Form.xml | `InputField` `DataPath` is packed for existing and new layout entries |
| 15 | Partial metadata XML | `ExchangePlan` and related partial families emit owned `Form`/`Template` child refs |
| 15 | Simple metadata XML | `DocumentJournal` emits owned `Form`/`Template` child refs |
| 15 | Source staging readiness | HTMLDocument template rows are covered as base-free help-style blobs |
| 14 | Form.xml | new `InputField` items preserve explicit `ReadOnly` values |
| 14 | BusinessProcess metadata XML | generated type `InternalInfo` entries are emitted |
| 14 | Task metadata XML | owned `Form` and `Template` child object headers are emitted |
| 14 | Source staging readiness | `Style/Ext/Style.xml` rows are prepared without active `Config` blobs |
| 13 | Form.xml | new `PictureDecoration` child items can be compiled from XML |
| 13 | Catalog metadata XML | owned `Form` and `Template` child object headers are emitted |
| 13 | MXL templates | SpreadsheetDocument `style:ButtonTextColor` is supported in pack and extract paths |
| 13 | Source staging readiness | SpreadsheetDocument `Ext/Template.xml` rows are prepared without active `Config` blobs |

Scope exclusion: `ConfigDumpInfo.xml` is intentionally not generated and is not
counted as parity debt.

`Configuration.xml` support currently covers the root metadata header
(name/synonym/comment/uuid), source XML version selection, selected root child
object headers (`CommonAttribute`, `CommonModule`, `Constant`, `Catalog`) and
selected root refs such as default roles/style/language/settings storages;
localized root information fields are also covered;
deeper root properties remain tracked under Issue #22.

## Commands

```powershell
cargo run -- probe --deep
cargo run -- scan C:\path\to\xml-sources -o current-manifest.json
cargo run -- plan current-manifest.json -b baseline-manifest.json -o load-plan.json
cargo run -- source-diff C:\ibcmd-export C:\ibcmd-rs-dump --path-prefix Catalogs\Валюты -o source-diff.json
cargo run -- profile-run --capture-output -- ibcmd infobase config load ...
cargo run -- dump-sources --settings C:\repo\autumn-properties.json --extension EmergingTravelGroup -o C:\repo\src\cfe\EmergingTravelGroup --overwrite
cargo run -- mssql-dump-config --database MyInfobase -o C:\repo\db-dump --include-config-save --inflate --extract-module-text
cargo run -- trace-template .\trace
cargo run -- trace-analyze .\trace\events.xml -o trace-analysis.json
cargo run -- storage-map .\trace\events.xml -o storage-map.json
cargo run -- compatibility
cargo run -- mssql-compare --left ref_db --right target_db -o compare.json
cargo run -- mssql-clone --source target_db --target ours_db --overwrite --allow-non-lab
cargo run -- mssql-storage-export --database import_only_db -o storage-bundle --overwrite
cargo run -- mssql-storage-import --database empty_target_db -i storage-bundle --replace --allow-non-lab
cargo run -- mssql-delta-export --database staged_db -o delta-bundle --overwrite
cargo run -- mssql-delta-import --database target_db -i delta-bundle --allow-non-lab
cargo run -- module-blob-pack --text CommonModules\...\Ext\Module.bsl --base-blob module.bin -o module.blob
cargo run -- versions-blob-patch -i versions.bin -o versions-new.bin --change <metadata-id> --change <metadata-id>.0
cargo run -- mssql-stage-common-module --database target_db --module-id <metadata-id> --text CommonModules\...\Ext\Module.bsl --replace-config-save --allow-non-lab
cargo run -- mssql-stage-common-modules --database target_db --module <metadata-id>=CommonModules\...\Ext\Module.bsl --module <metadata-id>=CommonModules\...\Ext\Module.bsl --replace-config-save --allow-non-lab
cargo run -- mssql-stage-common-module-metadata --database target_db --module-id <metadata-id> --xml CommonModules\...\Module.xml --replace-config-save --allow-non-lab
cargo run -- mssql-stage-common-module-object --database target_db --xml CommonModules\Module.xml --replace-config-save --allow-non-lab
cargo run -- mssql-stage-common-module-objects --database target_db --xml CommonModules\Module1.xml --xml CommonModules\Module2.xml --replace-config-save --allow-non-lab
cargo run -- mssql-stage-metadata-objects --database target_db --source-root C:\full\xml-sources --xml Constants\SomeConstant.xml --xml SessionParameters\SomeParameter.xml --replace-config-save --allow-non-lab
cargo run -- mssql-stage-source-metadata-objects --database target_db --source-root C:\full\xml-sources --replace-config-save --allow-non-lab
cargo run -- mssql-stage-source-common-module-objects --database target_db --source-root C:\full\xml-sources --replace-config-save --allow-non-lab
cargo run -- mssql-stage-source-objects --database target_db --source-root C:\full\xml-sources --source-version 2.21 --replace-config-save --allow-non-lab
```

### bcp client compatibility

The `mssql-storage-*` and `mssql-delta-*` commands shell out to `bcp`. Native
`mssql-dump-config` also uses the existing `bcp` native parser for blob-bearing
row fetches in streamed export paths, while lightweight headers/control queries
still use `sqlcmd`. By default these paths invoke `bcp` with native format,
which works across `bcp` versions including `bcp 13` (Microsoft ODBC Driver 13
for SQL Server).

`bcp 18+` (ODBC Driver 18) defaults to encrypted connections and may need the
server certificate to be trusted. Pass `--bcp-trust-cert` to add `bcp -u` (trust
server certificate) in that case. Do **not** set it with `bcp 13` or earlier:
those builds do not recognize `-u` and will fail with an "unknown argument"
usage error.

```powershell
# bcp 18+ over an encrypted connection to a self-signed server:
cargo run -- mssql-storage-export --database import_only_db -o storage-bundle --overwrite --bcp-trust-cert
```

## First ERP Experiment

1. Prepare a disposable ERP infobase on SQL Server.
2. Export or prepare ERP XML sources.
3. Build a baseline manifest:

   ```powershell
   cargo run -- scan C:\erp-src -o baseline-manifest.json
   ```

4. Generate trace templates:

   ```powershell
   cargo run -- trace-template .\trace
   ```

5. Start SQL Server Extended Events from `trace\sqlserver-xevents.sql`.
6. Run the slow load through `profile-run`:

   ```powershell
   cargo run -- profile-run --capture-output -- ibcmd ...
   ```

7. Stop the SQL trace and keep the `.xel`, 1C technical log, manifest and
   profile JSON together.
8. Export event XML from SQL Server and group it:

   ```powershell
   cargo run -- trace-analyze C:\temp\events.xml -o trace-analysis.json
   ```
9. Map the grouped SQL into storage-mutation families:

   ```powershell
   cargo run -- storage-map C:\temp\events.xml -o storage-map.json
   ```

## Roadmap

1. Source model: parse object identity, UUIDs, module/form/template ownership.
2. Storage bundle bridge: export/import `ConfigSave` and `Params` using native
   SQL Server BCP so a prepared import state can be applied in an empty infobase.
3. Delta bundle bridge: export/import staged `ConfigSave` rows for a prepared
   partial change in an existing infobase.
4. Common module body compiler: build a valid module `.0` blob from BSL source
   as `deflate(V8File(info,text))`, using a base blob for element headers.
5. Versions patcher: build a staged `versions` blob by replacing generation,
   optional `root`/`version`/`versions` entries when present, and changed file UUIDs;
   source/metadata staging can append missing changed entries for newly staged body rows.
6. SQL common-module stager: read active `Config`, generate the changed `.0`
   and `versions` blobs, and write the five-row `ConfigSave` staging set.
7. Multi-module stager: stage several common module body changes in one
   `ConfigSave` set with a single patched `versions` blob.
8. Common module metadata stager: stage XML changes for `Name`, `Synonym`,
   `Comment`, execution-context flags, `Privileged`, and `ReturnValuesReuse`
   with a four-row `ConfigSave` set.
9. Common module object stager: stage a complete common module from XML and
   sibling `Ext\Module.bsl` in one five-row `ConfigSave` set.
10. Batch common module object stager: stage several complete common modules
   from XML plus sibling `Ext\Module.bsl` files with one shared `versions` blob.
11. Generic simple metadata stager: stage metadata-only XML changes for
   `Name`, `Synonym`, and `Comment` while preserving the rest of each metadata
   blob; verified on `Constant` and `SessionParameter`. For `Constant`, it also
   stages supported `Type` patterns (`boolean`, `string`, `decimal`,
   `dateTime`) and `UseStandardCommands`. For `DefinedType`, it stages builtin
   `Type` patterns with one or more `boolean`, `string`, `decimal`, and
   `dateTime` entries, plus `cfg:*` reference types resolved from
   generated `TypeId` values under `--source-root`. For `CommonCommand`, it stages
   `Representation`, `ToolTip`, `IncludeHelpInContents`, `ParameterUseMode`,
   `ModifiesData`, `Picture` for empty or `CommonPicture.<name>` refs,
   `CommandParameterType` for empty or a single `cfg:DefinedType.<name>`, and
   the currently observed `OnMainServerUnavalableBehavior` value `Auto`.
   Reference resolution requires `--source-root`; `StdPicture.User` is mapped
   to the platform-owned user picture UUID, while other `StdPicture.*` values
   and arbitrary multi-type command parameter sets are still rejected until
   mapped.
12. SQL verifier: compare table shape, row counts and later row checksums.
13. Trace analyzer: expand `.xel` export support and add more robust SQL
   normalization.
14. Storage mapper: map 1C metadata operations to observed SQL mutations per
   platform version.
15. Writer hardening: keep destructive staging behind explicit confirmation
   flags and add more safety gates before non-lab use.
16. Compatibility matrix: platform build, DBMS, compatibility mode, configuration
   type and supported operation set.
