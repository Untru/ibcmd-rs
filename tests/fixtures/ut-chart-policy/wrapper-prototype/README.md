# Source-owned chart wrapper prototype

**Source input, not native expected output.** Candidate bootstrap and one native
import/export completed for this exact tree. The observed name/count facts are
recorded in [the zero-series evidence](../explicit-english-zero-series/evidence.json).
Historical data accounting remains `FINAL_AUDIT_INCOMPLETE`; no full parity is
claimed. Any additional operator run still requires review and allocation.
No default export selector or causal language control is established here.

`tree/` contains five source files: the pinned hand-authored XML 2.20 bootstrap
configuration/common-module pair plus a base CommonTemplate descriptor and a
spreadsheet document with one chart. This preserves the three-file starting
skeleton rather than removing an already-established module. It is the smallest
assembly made here, not a proof of global minimality. Configuration/template names
and UUIDs are synthetic; the starting module's synthetic identity is preserved.
The spreadsheet's cell, parameter and named-area strings are replaced by seed
literals. The chart's unrelated localized title becomes `ChartSeedTitle`.
The target retains the approved public `Сводная` token.

Only `tree/` is an input directory. README and manifest are evidence metadata;
never pass their parent as the configuration tree. No ConfigDumpInfo is fabricated.
Existing native reference harnesses admit source XML without that file, but
that was the preparation boundary. The later retained run used native
8.3.27.2342; its bounded results are recorded in the linked evidence.

## Rules supporting the assembly

| Component | Existing rule and chosen representation |
| --- | --- |
| Starting configuration | `tests/fixtures/bootstrap/manifest.json`: hand-authored clean-room three-file 2.20 fixture; retain module and register CommonTemplate in ChildObjects |
| Root registration | `src/compiler/root.rs`: CommonTemplate family; `src/metadata_model/root.rs`: matching metadata family |
| Base descriptor | `src/metadata_model/common_export.rs` `common_template`, `header_properties`, `object`: CommonTemplate UUID, Properties Name/Synonym/Comment/TemplateType; no extension-only fields |
| Template enum | `crates/ibcmd-schema/src/external_named.rs` TEMPLATE_TYPES: SpreadsheetDocument; the public 2.20 nested Template fixture independently uses this spelling |
| Body location/root | Existing common-template fixture uses CommonTemplates/<name>/Ext/Template.xml; SpreadsheetDocument body rule in `src/compiler/bodies/template.rs` and schema external_named root is document in spreadsheet namespace |
| Spreadsheet carrier | Pinned public `external/test_processor/expected/.../Template.xml`, XML 2.20; two rows, three columns, four formats, no existing drawings |
| Drawing placement | `src/mssql_dump/moxel.rs` `render_moxel_spreadsheet_xml`: drawings after rows and before templateMode |
| Required drawing fields | `push_moxel_drawing_xml` and `src/compiler/bodies/mxl_native.rs` `drawing`: Chart; synthetic id 1; formatIndex 4 within existing pool; row 0..1, column 0..2, offsets 0; autoSize false, pictureSize Stretch, zOrder 1; no pictureIndex |
| Object payload | Pinned public empty-no-extended-scales native fragment, baseline unchanged flag / no extended XML scale profile / realSeriesCount 0; one mandatory realExSeriesData |
| Namespace binding | object inherits spreadsheet default namespace; its explicit d3p1 binding and xsi:type d3p1:Chart retained; v8/v8ui/xsi bindings inherited from document |
| Style references | Existing compiler `platform_style_color` names include FormTextColor and ToolTipBackColor; no custom configuration style item or private UUID is added |

All source hashes and prototype file hashes are pinned in `manifest.json`.
Geometry/id values are synthetic choices inside the existing typed writer's
admitted fields; they are not measurements of native layout or business values.
No extra packaging flag or guessed metadata property is introduced.

## Important serializer boundary

The published generator constructs well-formed one-line XML fragments. The
candidate compiler's `mxl_native.rs` chart path compares significant source lines
with the exporter renderer. A semantically equal one-line fragment can therefore
be rejected by that candidate roundtrip guard. This prototype preserves the
pinned native multiline object layout, changing localized title text and using
consistent LF endings for this source input;
it does not weaken the guard or change the published generator. Candidate
compilation was unmeasured at preparation; its later successful bootstrap is
recorded in the linked zero-series evidence. Do not normalize output or convert a guard
refusal into native acceptance. Future case packaging must retain an admitted
source rendering or explicitly report compiler refusal.

## Historical static preparation checks

- Pinned source hashes match; five-file inventory and output hashes recorded.
- Configuration name/UUID and CommonTemplate registration/descriptor agree.
- XML parses; drawing and chart object namespace/xsi:type resolve correctly.
- Exactly one drawing; its formatIndex refers to an existing format.
- Zero real-series nodes/count; one unchanged extra-series approved-name node.
- No generated expected XML, native process, build, database or private input used.

## Gates still open

Source review, candidate compilation and one disposable native import/export
have completed. The candidate produced a 7,505-byte Format15 CF with SHA256
`b2fa2d388face5e7281711d3714815cc304e12bd220f7495be15abfa0a613b8b`.
The native name observation is `Pivot`/`ru`, zero real series and unchanged name;
selected input levels and identities were preserved. The original input manifest
retains its historical preparation state and hash; later evidence is separate.
Native import can normalize storage, so this does not prove historical binary
layout preservation. Final data accounting, full byte/inventory parity, controlled
paired matrix results and the normal export selector remain pending.

ScriptVariant English is retained from the bootstrap skeleton and fixed across
cases; it is not a supported UI/export language discriminator. Create-only
`--locale` does not establish one either. The historic `--system=...cfg` attempt
was rejected by native argument parsing, so it establishes no language setting.
No new environment switch or experiment is proposed by this wrapper.

The first prepared source used mixed LF/CRLF from fragment insertion. The
retained external v001 bundle records that preparation. This prototype now uses
consistent LF as an input construction choice; no historical native expectation
or byte/inventory comparison is altered or normalized.

## Reviewed InternalInfo prerequisite

The legacy three-file bootstrap skeleton omits InternalInfo. A retained legacy
bootstrap attempt refused CommonTemplate; the separate retained base-free
attempt then refused Configuration with `Configuration has no <InternalInfo>`.
Neither attempt published a CF or ran inspect/export. Their receipts and original
fixture bundles remain unchanged outside the repository.

The current Configuration adds only the readable `xr` namespace declaration and
seven ContainedObject ClassId/ObjectId pairs. `metadata_model/root.rs`
`contained_objects` requires InternalInfo; `configuration` requires one pair for
each fixed SECTIONS class. `compiler/root.rs` CONFIGURATION_SECTIONS has the same
mapping; `root_export.rs` emits those pairs. The public clean-room
`tests/cf_bootstrap_package_intent.rs::configuration()` supplies exactly this
structure. The public native `file-infobase/empty-xml-8.3.27/Configuration.xml`
independently contains the same seven classes.

Class IDs are copied from those established rules, not generated. Object IDs are
independent synthetic UUIDs, following the existing clean-room test convention,
in a disjoint 400-prefix because the test's 200-prefix would collide with the
retained Portable module. They are section identities, not guessed GeneratedType
or application type IDs. Independent static review approved this exact change.
The manifest pins rule/test lineage and both Configuration hashes. The other four
tree files are byte-identical to the previous prototype.

This addition established the source prerequisite before the later candidate
and native run. Those later results do not close full parity or selector gates.
No refusal/roundtrip guard, default or prior receipt was changed to admit the
input. The manifest and historical refusals are preserved.
