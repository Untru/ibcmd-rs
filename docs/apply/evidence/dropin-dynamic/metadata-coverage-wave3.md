# Existing module and template bodies: bounded coverage survey (#345)

This checkpoint expands classification of existing 8.3.27.2214 ObjectModule and
ManagerModule bodies and three existing template types. It does not admit new
objects, removal, descriptor/property edits, unknown collections, schema changes,
or nested managed-form families without functional native-client proof. The
released CommonModule/CommonForm routes remain subject to the same strict storage
checks. The separate initial 8.5 cohort retains its narrower guard.

## Native roles measured

The native matrix uses one existing owner per kind, with an exported marker in
each listed module. Suffixes come from the compiler's source-asset registry;
canonical owned collection classes come from the existing typed metadata model.
Neither lookup alone grants admission.

| Owner kind | ObjectModule suffix | ManagerModule suffix |
|---|---|---|
| Catalog | .0 | .3 |
| Document | .0 | .2 |
| Report | .0 | .2 |
| DataProcessor | .0 | .2 |
| ExchangePlan | .2 | .3 |
| Task | .6 | .7 |
| BusinessProcess | .6 | .8 |
| ChartOfAccounts | .14 | .15 |
| ChartOfCalculationTypes | .0 | .3 |
| ChartOfCharacteristicTypes | .15 | .16 |
| Enum | — | .0 |
| Constant | — | .1 |
| SettingsStorage | — | .8 |
| DocumentJournal | — | .1 |
| InformationRegister | — | .2 |
| AccumulationRegister | — | .2 |

The template cohort is SpreadsheetDocument under Report, HTMLDocument under
DataProcessor, and TextDocument under ExchangePlan, each an existing nested
Template descriptor with a .0 body. RecordSet, ValueManager and other template
types remain outside this cohort.

## Binding and storage checks

Root and child descriptors have bounded native parsing, canonical UUIDs, exact
recognized collection counts and unambiguous ownership. Duplicate classes,
duplicate or reparented IDs, unknown child collections, invalid counts and
malformed recognized root bindings fail closed. Child bindings publish only
after the entire descriptor validates.

Every staged and effective-active body must use its measured codec. Module
containers contain exactly one `info` and one `text` element; the measured info
record is preserved and text is UTF-8 with a BOM. Compressed bodies require a
complete DEFLATE stream with all input consumed, at most 8 MiB decoded per row.
The metadata graph and retained-alias collection each have independent row and
32 MiB byte bounds. The overall publication limit remains 128 staged rows.

The graph budget includes the root row, configuration descriptor, every read
parent/child descriptor and descriptor aliases inspected for ownership changes.
Complete header batches reserve their compressed bytes and row count before any
blob of that batch is requested. Each decode also uses the remaining aggregate
budget, capped at 8 MiB per row. Descriptor/body/help semantic comparisons and
the pending-deletion check use complete, bounded DEFLATE decoding. The historical
global Versions parser and its 64 MiB contract are unchanged.
Raw equality does not bypass descriptor/body/help decoding: even identical
compressed bytes must satisfy that complete-stream and per-row size contract.
The existing SHA-identical root/version service judgment remains unchanged;
the ownership graph independently decodes its root strictly, and changed root
semantic comparisons use the same bounded decoder before profile restamp rules.

Retaining a prior generation also requires admission. Every non-service pending
alias is checked against exactly one ordinary row, using its bound full physical
header and bytes. Pending descriptors must be semantically unchanged; pending
bodies must use their admitted owner/role and codec. Help companions may be kept
only when their decoded bytes are unchanged. Changed Help is not enabled.

The existing locked inventory/history/registration/alias/marker checks remain in
place. Additional ordinary descriptor/body preimages join the physical CAS.
Attributes, Creation and Modified are checked from judgment to image, and again
by the locked SQL CAS together with keys, sizes and SHA256. Reading an ordinary
row once for several generations does not remove the independent checks of each
alias.

## Evidence and current acceptance boundary

Raw evidence is preserved under `F:/ibcmd/lab/05/wave3/metadata`:

- `evidence/native-session-B3.json`, `logs/control_B3-{old,new}.jsonl`: native
  26-module old/new controls, old A/A and new B/B, no module errors.
- `evidence/native-template-session-C1.json`,
  `logs/control_template_C1-{old,new}.jsonl`: three native template controls,
  old B/B and new C/C, no template errors; the 26 modules remain B.
- `tests/fixtures/dynamic-metadata-native` contains bounded exact native raw
  descriptor/body fixtures for ownership and codec regressions. Sixteen managed
  form fixture decodes establish storage shape only. The nested Form admission
  list is empty: observer/startup timeouts and private-listener refusals are not
  functional form proof.
- `evidence/modules-templates-B-source.json`: 12,198 files copied from the
  native-established A export, exactly 26 module and three template source edits,
  no form or descriptor edits. `evidence/bsl-fixed-B.json` records completed
  ParseError/CodeBlockBeforeSub checks with zero errors.
- `logs/own_modules_templates_import_B.json`: matching candidate import stages
  48 rows, creates/removes no objects, compiles no descriptors, and verifies all
  12,197 exported source files. This is import proof, not activation acceptance.
- `evidence/own-body-companions-full-diff.json`: the same import unnecessarily
  restages 16 unchanged sibling assets. Twelve have identical decoded bytes;
  three module containers regenerate headers despite identical info/text payloads,
  and ExchangePlan .1 changes an internal UUID. Exported-source equality does not
  admit those physical changes. The classifier refuses them.
- The stage now treats an exact ObjectModule/ManagerModule-only edit separately
  when the owner descriptor is unchanged and no override, built/widened owner or
  descriptor alias applies. Unchanged sibling rows follow the existing pending-row
  preimage logic. Mixed descriptor/assets/nested-form/template edits keep the
  previous Ext grouping. No new codec is admitted by this import optimization.
- `logs/delta-module-only-RED.log` reproduces both sibling-restaging and pending
  preimage errors before the repair; `logs/delta-module-only-GREEN.log` passes all
  12 focused stage tests, including mixed and forced routes. Frozen-source
  `gates-broad-v5/summary.txt` passes all four mandatory quick gates with
  3,704 root tests passed, zero failed and ten ignored. The earlier v4 quick
  attempt failed at shell startup and is not counted as a gate pass.
- Independent review found two budget gaps in that initial checkpoint. The
  repaired graph-header probe refuses before requesting either mocked 16 MiB
  parent blob; the initial code requested one despite the root/configuration
  bytes exceeding the aggregate budget. Semantic comparison also now refuses a
  full decoded payload without a DEFLATE stream end and an 8 MiB + 1-byte row.
  `logs/graph-budget-RED-v3.log` and `logs/semantic-bound-RED-v2.log` retain those
  failures. `logs/budget-repair-GREEN.log` passes 44 focused tests, including
  exact/over-limit compressed, decoded and row-count budgets, valid compression
  variants and the measured native 8.5 root restamp.
- `gates-budget-repair/summary.txt` passes all four mandatory gates with 3,709
  tests passed, zero failed and ten ignored. The earlier import and executable
  remain historical pre-repair evidence and are not activation authority.
- `logs/semantic-identical-RED.log` reproduces the remaining equality-shortcut
  gap in both the comparison helper and actual descriptor/help judgment. The
  repaired comparison refuses identical unfinished streams and identical rows
  expanding to 8 MiB + 1 byte; valid identical rows still compare equal.
  `logs/semantic-identical-GREEN.log` passes all 46 focused dynamic tests.
  `gates-semantic-identical/summary.txt` passes all four mandatory quick gates:
  3,711 tests passed, zero failed and ten ignored. Both earlier clean binaries
  remain historical pre-repair candidates. The clean final candidate used below
  is source `bb86ee58ca228db583712157eb82956beae88d29`, binary SHA256
  `B402DC94A55ED6AB316C446D2DD344BFD3EBC75DAA4E5072646EA0D419C5CEC0`.

## Fresh module activation twins

The native and OWN D1 controls each restore the native-established A backup into
a new owned database. Four samples per control cover all 26 module roles: the
retained old COM session reads A before and after publication; the new session
reads B before and after its second sample. Each sample also reads the three
unchanged templates as A, with zero module or template errors. Full session UUID,
infobase, application, start time and helper process bindings are retained; this
is functional proof for the measured existing module cohort.

The OWN lifetime uses the clean final candidate, with all 551 tracked source
files, clean HEAD and binary digest checked before launch and after completion.
Native import/apply/export and OWN import/apply/native export return success;
acceptance additionally uses the actual samples and full source comparisons.

Both exported inventories contain 12,198 files. Against the expected
modules-B/templates-A tree, 12,197 files match byte for byte. ConfigDumpInfo
matches after changing only `configVersion` attribute values, with all 9,835
attributes and remaining bytes preserved. Native changes exactly 42 values
(26 modules and 16 owners); OWN changes exactly 26 module values and no owners.
The native and OWN module `info` and `text` element payloads match in all 26 rows.
Neither raw compressed bytes nor complete inflated containers match: native
container headers differ, and those differences remain in the report.

The storage evidence preserves full headers, SHA256 and 493 selected native raw
blobs / 445 selected OWN raw blobs. Native stages 45 rows, including 16 unchanged
descriptor companions; OWN stages 29 rows, without those companions. Native
creates descriptor aliases and changes 16 Params SI rows plus siVersions; OWN
preserves these rows. Both publish their actual generation histories and change
MobileVersions.dat. Distinct generation values and dates are retained.

Native replaces all 20,685 registration IDs and 21,357 linked physical rows while
preserving every owner/node file list. It resets 60 touched messages from zero
to NULL and changes 65 other NULL messages to zero. OWN preserves IDs, linked
rows and file lists, and resets only the same 60 touched messages. Native also
changes three opaque `.ui` Params rows and `extd_props_cached/gc.mrk`; OWN
preserves them. The native after-payloads for these four opaque rows were not
selected: only their complete headers and SHA256 are captured. All 15 help/search
index rows remain byte/header exact in both controls. These are explicit storage
differences and capture limits, not a complete physical parity claim.

Both controls finish unregistered with their owned clients closed, the private
cluster stopped without purge and worker/native leases released. Native cleanup
first refused a register/unregister result-label collision; its raw refusal is
preserved and a separately reviewed cleanup-only controller completed cleanup.
V7 uses distinct action labels, with actual production-helper RED/GREEN evidence,
and OWN cleanup succeeds directly. Databases remain for the standard 60-minute
idle cleanup guard; no old shared registration is modified by these controls.

The compact retained evidence map is
`metadata-modules-D1-summary.json` beside this document. Full originals and
reproducible offline comparisons remain on F:

- `evidence/body2-native-modules-D1-checkpoint.json`: 557 pinned native proof files.
- `evidence/body2-own-modules-D1-checkpoint.json`: 511 pinned OWN proof files.
- `evidence/body2-modules-D1-twins.json`: exact 26 element comparisons and gaps.
- `evidence/body2-{native,own}-modules-D1-{source,storage,registration}.json`:
  complete source, full-header/selected-raw and registration comparisons.
- `evidence/body2_own_modules_D1-source-freeze.json`: actual-write provenance.

OWN template activation remains pending. Its next control must start from a
captured modules-B/templates-A baseline and change only the three templates;
the earlier combined A-to-B fixture is not fresh template-only activation proof.
Nested managed-form admission remains empty. No broad all-object or high-load
acceptance follows from these module controls.

The prepared >128-row control remains a native-fallback case. No larger dynamic
budget is enabled by this checkpoint, and no all-object or high-load acceptance
is inferred from this survey.
