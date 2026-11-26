# Review of the physical-adapter guard findings brought by PR #387

Issue #396. Track exdg, 2026-09-30. Branch `feat/0.4-guard-review`.

- Checkpoint 1 (review only, base `feat/0.4` at `7ad219d9`): sections 1-5 and the analysis of section 6.
- Checkpoint 2 (the coordinator's decisions, `feat/0.4` `2cfe46a9` merged first): **G1 and G3 are done** (section 6),
  **G2 is not in 0.4 and is filed as #413**, the text patching of `form_extension.rs` is filed as #414
  (both in milestone 0.8). Section 10 has the final counts.

The guard is `tools/validate-physical-adapter-policy.ps1`; its baseline is
`tools/physical-adapter-policy-baseline.json`. Every line number below is a
line of `7ad219d9`. The decision for every single finding is in
[`physical-adapter-387-findings.csv`](physical-adapter-387-findings.csv)
(579 rows: the 578 findings and the one merge entry).

## 1. Result in brief

- PR #387 brought **578 findings**: 578 baseline entries (a category plus a
  fingerprint, per file), 605 occurrences in the source, because 3 entries
  repeat a statement that was already baselined. 347 are `uuid-literal`,
  156 `name-special-case`, 75 `xml-policy`.
- **None of the 578 names a configuration object.** The 59 distinct
  `name-special-case` strings outside the tests and the folder table are
  storage tags (`"0"`, `"22"`, `"{3,"`), platform kind or element names
  (`"CommonModule"`, `"UsualGroup"`) and a few type spellings (`"cfg:AnyRef"`).
- Verdict: **401 of the 578 go away through three general fixes** (G1, G2, G3
  in section 6) and **177 stay in the baseline** with a stated reason
  (sections 4 and 5). Decided for 0.4: **G1 (150) and G3 (46) are done and
  their entries are out of the baseline; G2 (205) is deferred to #413** because
  it changes export behaviour (six pictures are named where the export fails
  now) and `mod.rs` is a hot merge zone. So 196 fixed, 205 deferred, 177 kept
  (13 of them also filed as #414).
- **The one entry that came from the merge resolution** (2429d691) is the
  `{"Pattern"}` comparison of the defined-type condition (section 3). It is
  not a new literal: master already had it; the merge only changed its
  fingerprint.
- **Master is not patched separately.** The decision stands: `master` turns
  green when 0.4 merges into it (section 8).

## 2. How the set was derived

The finding set is the difference of two baselines, by key. A key is
`file`, `category` and `fingerprint`; the guard fails when a key is missing in
the baseline or its count is above the baseline count.

| Baseline | Files | Entries | Occurrences |
|---|---|---|---|
| `2429d691^1` = `4d5af6be` (feat/0.4 before the master merge) | 37 | 8 376 | 9 628 |
| `2429d691` (the merge, regenerated baseline) | 42 | 8 712 | 9 987 |
| `7ad219d9` (feat/0.4 now) | 42 | 8 755 | 10 035 |

`2429d691` has a single parent (`4d5af6be`); the master side was brought in as
a tree, so the "first parent" of the task is its only parent.

- Entries the merge baseline has and `4d5af6be` has not (or has with a lower
  count): **579 entries, 606 occurrences** (576 new entries and 3 whose count
  went up). 578 of them are #387; 1 is the merge entry.
- All 578 are still in the baseline of `7ad219d9` with the same fingerprints and
  counts: no later commit changed any of these statements. So the review of the
  current tree is the review of the merge.
- Cross-check against the master tree (`e273b925`, PR #387 merge): scanning its
  sources gives 579 entries the master baseline does not allow, plus 5 scoped
  files it does not list (`extension_types.rs`, `form_extension.rs`,
  `upgrade.rs`, `revision_mix_tests.rs`, `tests/onecdec.rs`). These 579 are the
  578 plus master's own spelling of the merge entry (section 3). This is why
  master is red.
- The guard prints only the first violation and only hashes, so it cannot
  give this set. The set was made by scanning the sources with the guard's own
  scanner (the C# source is cut out of the `.ps1`, patched only to also record
  the literal, the unit's lines and the context) and comparing keys. The patched
  scanner reproduces the baseline exactly at `4d5af6be`, `2429d691` and
  `7ad219d9` (same entries, same counts), and gives the 579 entries above at
  `e273b925`. The kit is in `F:\ibcmd\lab\04\guard-review` (not committed:
  checkpoint 1 changes no tool).
- The issue's split (mod.rs about 297, tests about 103, refs about 90,
  form_body about 41, the rest about 47) is an estimate; the total is right,
  the split is not. Measured: `mod.rs` 285, tests 150 (`tests/onecdec.rs` 103
  and `revision_mix_tests.rs` 47), `refs.rs` 40, `form_body.rs` 22, the rest 81.

## 3. The entry from the merge resolution

| Field | Value |
|---|---|
| File and item | `src/mssql_dump/mod.rs`, `parse_defined_type_properties_from_text`, condition at lines 34622-34625 |
| Category | `name-special-case` |
| Fingerprint | `8d9b9b33e523a880898dfa06a70fe9ed3cf4477325c9eb2f7d80e96d16b83fb4` |
| Literal | `r#"{"Pattern"}"#` |

The condition is `value_types.is_empty() && extension::active().is_none() &&
fields.get(4)?.trim() != r#"{"Pattern"}"#`. Master had
`value_types.is_empty() && fields.get(4)?.trim() != r#"{"Pattern"}"#`
(fingerprint `499149a5…`, one of master's 579). The merge added the
`extension::active().is_none()` term of feat/0.4, and the context of the
statement is part of the fingerprint, so the entry changed. The literal is the
storage form of an empty value-type pattern of an adopted defined type; it is a
storage-format marker (category R4 below), not an object name. Verdict: keep.

## 4. Categories and counts

Review categories (what the finding really is):

| Id | Category | Entries | Occurrences | Guard categories | Verdict |
|---|---|---|---|---|---|
| R1 | Test-only module the guard scopes by mistake | 150 | 171 | uuid 121, xml 21, name 8 | fix G1, **done** |
| R2 | A second copy of the platform standard-picture table | 205 | 205 | uuid 205 | fix G2, **deferred to #413** |
| R3 | A second copy of the kind-to-collection table | 46 | 46 | name 46 | fix G3, **done** |
| R4 | Storage-format marker (tag, version, flag, row name) | 72 | 75 | name 72 | keep |
| R5 | Platform class or type id (named constant, nil uuid) | 21 | 21 | uuid 21 | keep |
| R6 | Platform kind, element or command name | 27 | 27 | name 27 | keep |
| R7 | XML output or edit fragment | 36 | 36 | xml 33, name 3 | keep |
| R8 | Guard heuristic hit (not a decision) | 21 | 24 | xml 21 | keep |
| | **Total** | **578** | **605** | uuid 347, name 156, xml 75 | 196 fixed, 205 deferred, 177 keep |

The merge entry is one more R4 keep (not counted in the 578).

By file (entries; R-columns are entries):

| File | Entries | Occ. | R1 | R2 | R3 | R4 | R5 | R6 | R7 | R8 |
|---|---|---|---|---|---|---|---|---|---|---|
| `src/mssql_dump/mod.rs` | 285 | 286 | | 205 | 46 | 10 | 1 | 13 | 10 | |
| `src/mssql_dump/tests/onecdec.rs` | 103 | 117 | 103 | | | | | | | |
| `src/mssql_dump/revision_mix_tests.rs` | 47 | 54 | 47 | | | | | | | |
| `src/mssql_dump/refs.rs` | 40 | 43 | | | | 11 | 8 | | | 21 |
| `src/mssql_dump/upgrade.rs` | 32 | 33 | | | | 28 | 1 | 3 | | |
| `src/mssql_dump/form_body.rs` | 22 | 22 | | | | 5 | 3 | 8 | 6 | |
| `src/mssql_dump/form_extension.rs` | 16 | 16 | | | | 3 | | | 13 | |
| `src/mssql_dump/metadata.rs` | 12 | 13 | | | | 8 | 4 | | | |
| `src/mssql_dump/extension_types.rs` | 5 | 5 | | | | 1 | 1 | 1 | 2 | |
| `src/mssql_dump/source_assets.rs` | 5 | 5 | | | | 2 | 1 | | 2 | |
| `src/mssql_dump/form/xml_2_21_writer.rs` | 3 | 3 | | | | 1 | | 1 | 1 | |
| `src/mssql_dump/role_rights.rs` | 3 | 3 | | | | 3 | | | | |
| `src/module_blob.rs` | 2 | 2 | | | | | 1 | 1 | | |
| `src/mssql_dump/dcs.rs` | 2 | 2 | | | | | 1 | | 1 | |
| `src/mssql_dump/command_interface.rs` | 1 | 1 | | | | | | | 1 | |
| **Total** | **578** | **605** | 150 | 205 | 46 | 72 | 21 | 27 | 36 | 21 |

The three entries that repeat an already-baselined statement (count 1 to 2, 1
to 2, 2 to 3) are `"0" => false` and `"1" => true` in
`parse_information_register_owner_header` (mod.rs lines 15829-15830; the same
arms already exist in `parse_flowchart_scheme` and in `refs.rs`) and the
`<ChildObjects/>` push in `format_report_source_xml` (mod.rs line 40050; the
catalog and enum writers already had it). All three are R4 or R7 keeps.

## 5. Verdict per category

### R1: test-only modules scoped by mistake (150 entries) - replace by a general rule

`src/mssql_dump/tests/onecdec.rs` (103) and `src/mssql_dump/revision_mix_tests.rs`
(47) are compiled only for tests: `mod.rs:982-983` declares
`#[cfg(test)] mod revision_mix_tests;` and `mod.rs:46146-46147` declares
`#[cfg(test)] mod tests;`, whose `tests.rs:79566` declares `mod onecdec;`. The
guard already leaves out `tests.rs` and `metadata_order_tests.rs` by file name
and removes inline `#[cfg(test)]` items, so the intent is clear; it just does not
follow an out-of-line `#[cfg(test)] mod x;` to the file, and so it missed these
two. The findings are fixtures: UUIDs of made-up objects (`aaaaaaaa-0000-...`),
storage records and expected XML. Nothing here is production policy. Every next
`*_tests.rs` file would turn CI red again. Verdict: remove from scope by a
general rule (G1), not by adding two more names to the list.

### R2: second copy of the standard-picture table (205 entries) - replace by one table

`STANDARD_PICTURES` (mod.rs lines 35210-35466) is a documented table of
platform picture identifiers and the names the platform writes (212 rows, with
per-row corpus counts in comments). It is evidence-backed data, not a special
case. It is also a duplicate: `metadata_model/objects_export.rs:1631` keeps a
second table of the same kind (206 rows) for the object and register command
pictures (`objects_export.rs:1388`, `registers_export.rs:370`). The two agree on
206 rows and the `mssql_dump` copy has six more (`HierarchicalView`,
`QueryWizardCreateNestedQuery`, `QueryWizardCreateTempTableDescription`,
`QueryWizardShowChangesTables`, `Rename`, `SortList`). So today the form and
common-command exports (`standard_picture_name`, 212 rows) name a picture that
the object and register exports (206 rows) reject with "no name for picture".
Verdict: one table in the metadata model, the 212-row union (G2). This removes
the 205 entries, the drift and the second copy. If 0.4 wants no production change
now, the fallback is to keep the 205 entries; they are true data.

### R3: second copy of the kind-to-collection table (46 entries) - replace by one table

`root_family_folder` (mod.rs lines 5798-5848) maps the 46 kinds of the
configuration root families to their folder (`Role` to `Roles`, `Catalog` to
`Catalogs`, ...). `metadata_model/index.rs:55` `kind_of_collection` is the same
table in the other direction (plus five nested kinds: `Form`, `Template`,
`Recalculation`, `Interface`, `PaletteColor`). Checked mechanically: the 46
pairs are identical, none missing on either side. Two consumers besides the
dump depend on the function: `load/compiled.rs:488` and `load/external.rs:179`
(the #387 load paths). Verdict: one pair table in the model, both directions
derived from it (G3).

### R4: storage-format markers (72 entries + the merge entry) - keep

Tags, version codes, flags and physical row names that a parser compares a
field to: `"0"`/`"1"` bool arms, record versions (`"68"`, `"2.20"`, `"2.21"`,
`"{3,"`), item wrappers (`"22"`/`"7"`), the `"#"` and `{"Pattern"}` markers of a
type description, access value codes (`"-1"`, `"0"`, `"2"`), config-table row
names `"root"` and `"configinfo"` (mod.rs line 3930), the `9`/`a` file suffix
(source_assets.rs line 203), and the version-upgrade rules of `upgrade.rs`
(28 entries; that module is by design a store of evidence-backed upgrade rules
per record version, see its header). The guard sees `== "1"` as a name decision;
it is a byte of the format. They cannot be replaced by a general rule (the tag
is the rule) and naming each as a constant would only move the string. Keep, with
this reason. Where a tag decides a platform behaviour, the code comment already
carries the evidence (for example `default_button_importance`, xml_2_21_writer.rs
line 1420, revision `"34"`, #410).

### R5: platform class or type ids (21 entries) - keep

Named constants and one table of class identifiers that are the same in every
configuration: `ANY_IB_REF_TYPE_ID` (dcs.rs line 48), `TYPE_DESCRIPTION`
(extension_types.rs line 20), `WIDENED_TYPE` and the nil/configuration class
uuids of metadata.rs, `FORM_ITEM_IDENTITY_UUID` and the two 8.5-only event ids
(form_body.rs lines 863-884, evidence in the comments: fixture `form_events`),
`CONFIGURATION_CONTAINED_OBJECT_CLASSES` (refs.rs line 4972, 7 ids),
`HTML_TEMPLATE_DOCUMENT_UUID`, nil uuids, and the `cfg:AnyRef` row of
`builtin_v8_type_id` (module_blob.rs line 30524). Keep: a class id is data. Side
observation, not a #387 fix: the same ids are spelled in many places (the
`AnyIBRef` id in 9 source files, the nil uuid at 66 places in 37 files); a
repository-wide "platform ids" module would be a separate cleanup, outside 0.4.

### R6: platform kind, element or command names (27 entries) - keep

None is a configuration object. `upgrade_metadata_record` (3): the kinds that
have an upgrade rule. `extract_metadata_source_xml_from_text_row_with_owner_graph_diagnostic`
(9, mod.rs line 13084): a deliberate fail-closed list of kinds that must not
print a header-only frame when their record version is unknown; replacing it by
"kinds without a reader" would change which kinds fail closed. `format_form_child_item_xml`
(6, form_body.rs line 32482): the owners that write `TitleFont` next to their
own title (evidence in the comment: 11 native owners). Register property values
`Nonperiodical` and `RecorderSubordinate` (2, evidence: census of 4 074 registers
in the comment); the `CommonForm.` reference prefix; the foreign type category
`TypeSet`; the profile id `xml-2.20`; the standard command `SelectAll`; the
`Button` tag; the type spellings `cfg:AnyRef` and `cfg:AnyIBRef`. Keep. The one
that could be typed later is the `xml-2.20` profile compare (1 entry); not
worth a change now.

### R7: XML output or edit fragments (36 entries) - keep

Element spelling, indentation and CRLF of the platform's XML: the source writers
in mod.rs (10: `ExtensionProperty`, `DefaultSearchForm`, `ChildObjects/`,
`Task`, `NumberPeriodicity`, `Type/`), the form writer (6), `source_assets.rs`
`v8:lang` (2), `command_interface.rs` (1), `dcs.rs` (1), the `<Type>` wrappers of
`extension_types.rs` (2), and the adopted-form edits of `form_extension.rs` (13).
The guard's own documentation says it "cannot replace migration of XML policy
into schema-owned writers"; that migration is the direction, not a 0.4 task.
Keep. One group deserves a note for later: `form_extension.rs` (13 entries)
rewrites finished form XML with `find` and `replace` (`with_event_call_types`,
`with_adopted_form_parts`, `base_form_inner`). It is the only place where the
XML category points at a design smell (the call types and the base form should
be written by the form writer, not patched into its output). It belongs to a
refactor after 0.4 with the extension parity as its proof (see section 7).

### R8: guard heuristic hit (21 entries, 24 occurrences) - keep

All in one place: `parse_configuration_properties_from_text` (refs.rs lines
4027-4103), one `Some(ConfigurationProperties { .. })` literal. The guard's
`xml-policy` rule fires on an identifier that has both `xml` and `default`
(`configuration_default_run_mode_xml`, a decoder of one field) and then counts
every literal and number of the whole statement: the field indexes (2, 3, 4, ...)
and the reference kind prefixes `"Style."`, `"Language."`, `"SettingsStorage."`.
The statement decides no XML order, name or default. Keep. Narrowing the rule
would also stop counting real decisions; not proposed.

## 6. The fix list and its risk

Proposed at checkpoint 1, decided by the coordinator for checkpoint 2:

| Id | What | Entries out of the baseline | Production code | Decision |
|---|---|---|---|---|
| G1 | The guard follows `#[cfg(test)] mod x;` to the file | 150 (171 occ.), 2 file records | none | **done**, commit `8255560c` |
| G2 | One standard-picture table, in the metadata model | 205 | yes, small | **not in 0.4**, #413 (milestone 0.8) |
| G3 | One kind-to-collection table, in the metadata model | 46 | yes, small | **done**, commit `0bfc2f59` |

The analysis below is the checkpoint-1 text; section 10 has what G1 and G3
measured, and G2 stays as the plan of #413.

### G1. The guard follows `#[cfg(test)] mod x;` (done)

Change (tool only): the scanner learns the predicate it already computes for
inline items (`TryParseCfgAttribute`, "cannot be true outside tests") for an
item that is `mod name;`. `Get-ScopedFiles` then leaves out `name.rs` and
`name/mod.rs` next to the declaring file, and everything under the directory of
an excluded module (`tests/` under `tests.rs`). `cfg(any(test, ...))` and other
partly-production predicates stay guarded, as the tool's documentation says.
`$ExcludedMssqlModules` keeps only `mxl_ir.rs` and `moxel.rs` (production, out
of scope by decision). `-SelfTest` gets cases for a declared test module, a
nested one and an undeclared `x_tests.rs` (still guarded).

Parity that could move: none. No production source is touched and nothing is
compiled differently; the CF/CFE/EPF/ERF round trips, the БСП/УХ export and the
4 extensions run the same binary.

Proof, offline, no lab needed:
1. `-WriteBaseline` into a temp copy and a key diff against the current file:
   the only difference is removal of exactly the 150 entries (171 occurrences)
   and the two file records `revision_mix_tests.rs` and `tests/onecdec.rs`.
   Both files are wholly new in #387, so nothing else of theirs is in the file.
2. The guard passes on the tree and `-SelfTest` passes with the new cases (the
   Windows and Linux jobs of `offline-e2e.yml` run both).
3. The declarations exist as quoted (`mod.rs:982`, `mod.rs:46146`,
   `tests.rs:79566`): a test module cannot be reached by production code.

### G2. One standard-picture table (not in 0.4: #413)

Change: the 212-row union (with the evidence comments and counts) becomes the
only `STANDARD_PICTURES`, in `metadata_model` (outside the guard's scope, next
to the other platform type tables). `mssql_dump::standard_picture_name`,
`standard_picture_uuid` and `STANDARD_PICTURE_NAMES` read it; the copy in
`mssql_dump/mod.rs` and the 206-row copy in `objects_export.rs` go away. The
`STD_PICTURE_*` constants used by other code stay.

Parity that could move:
- `cf load`/`cf export` help pages and every loader that turns a `StdPicture.X`
  name into a uuid: unchanged, the union keeps all 212 rows and both name and uuid
  are unique in it (checked: no duplicate of either).
- The object and register command-picture exports (`objects_export.rs:1388`,
  `registers_export.rs:370`) now name the six extra pictures where they used to
  fail with "no name for picture". That can only turn an error into a name, and only
  where such a picture occurs.
- БСП/УХ default export and the 4 extensions: a picture the native export names
  must be in the native tree. Searched all `.xml` of the native reference trees
  for the six names: БСП 8.3.27 (7 760 files) 0 hits, БСП 8.5 (7 860 files) 0
  hits, the seven extension trees under `F:\ibcmd\lab\05\ext\native` (four
  for 8.3.27, three for 8.5; 989 files) 0 hits, УХ 8.3.27 (98 117 files) 0
  hits, УХ 8.5 (98 117 files) 0 hits. So no corpus in the acceptance set
  exercises the change; it is proved by the table test and by tree equality.

Proof:
1. A unit test that the table equals a snapshot of the old 212 `(uuid, name)`
   pairs (the snapshot is a test fixture), and that uuids and names are unique.
2. The existing tests of the touched functions:
   `names_the_standard_pictures_of_the_query_wizard_forms` (`tests/onecdec.rs`
   line 348, the six extra names), `resolves_common_command_standard_picture_uuids`
   (`module_blob.rs` line 44283), the help-page tests, and the full
   `cargo test -p ibcmd-rs --no-default-features`.
3. Offline corpus equality, before and after, same input rows: БСП 8.3.27 and
   8.5 (`F:\ibcmd\lab\rawrows\bsp\Config`, `F:\ibcmd\lab\v85\rawrows\bsp\Config`)
   through `mssql-dump-config --rows-dir ... --extract-metadata-xml
   --extract-module-text --no-binary-rows`, trees compared with
   `F:\ibcmd\lab\04\apply\tools\compare_trees_fast.py`: 0 differences expected;
   the 4 БСП extensions through the ext track's offline export with the same
   comparison; one УХ 8.3.27 offline run under the heavy lock to confirm.
4. The #387 fixture round trips: `cargo test --no-default-features --test
   cf_export --test cf_roundtrip --test cf_native_roundtrip --test cf_load
   --test cf_load_compiled --test external_export --test extension_export
   --test form_events`, and with the corpora present
   (`IBCMD_CF_EXTERNAL_CORPUS`, `IBCMD_ONECDEC_CORPUS`) `cf_corpus` and
   `external_corpus` against `tests/fixtures/external/corpus-baseline.txt`.
5. Guard: the baseline diff is removal of exactly the 205 entries.

### G3. One kind-to-collection table (done)

Change: a single `ROOT_COLLECTIONS` pair table in `metadata_model/index.rs`;
`kind_of_collection` (folder to kind, plus the five nested kinds) and a new
`collection_of_kind` (kind to folder, the 46 root kinds only) are both derived
from it. `mssql_dump::root_family_folder` becomes a call to `collection_of_kind`
(its three users, `mod.rs:5763`, `load/compiled.rs:488`, `load/external.rs:179`,
keep the signature).

Parity that could move: the function is total over a fixed set and the two
tables were found identical (46 of 46 pairs, none only on one side), so no
input can change its answer. The surfaces it feeds are the #387 load paths
(`cf load` compiled and external) and the root-family folder of the dump;
neither can move if the function is unchanged.

Proof:
1. An exhaustive unit test: for the 46 old kinds `collection_of_kind` returns the
   old folder (the test carries the old table as data); for `Form`, `Template`,
   `Recalculation`, `Interface`, `PaletteColor` and a non-kind it returns `None`;
   `kind_of_collection(collection_of_kind(k)) == k` for all 46.
2. `cargo test -p ibcmd-rs --no-default-features` with `cf_load`,
   `cf_load_compiled`, `cf_overlay`, `external_export`, `cf_roundtrip`.
3. Offline equality of the four corpora and the extensions as in G2 (expected
   identical), with the guard baseline diff being removal of exactly the 46
   entries.

Order and cost: G1 first (no risk, unblocks the next test file), then G2 and G3
as two small commits; each carries its proof in the commit message. Together
they are about one working day, mostly for the corpus reruns.

## 7. Considered and not proposed for 0.4

- **The 177 keeps.** They cannot be replaced by a general rule without moving
  code that has evidence attached; the risk is real (they sit in the readers
  and writers that the БСП/УХ parity depends on) and the gain is only fewer
  baseline lines. Renaming a tag to a constant only moves the string.
- **`form_extension.rs` text edits** (13 entries of #387, 15 with the later
  `with_action_call_types`). Real design smell, see R7; filed as #414 (milestone
  0.8). Proof would be the four БСП extensions and the adopted forms of the fixtures
  (`extension_adopted*`, `form_events`) byte-equal after the change.
- **A repository-wide platform-ids module** (R5 side observation). Out of scope.
- **Narrowing the guard's `xml`+`default` rule** (R8). Would hide real
  decisions.

## 8. Master

Decided already and repeated here for the record: `master` is not patched
separately. `origin/master` is `e273b925` (the PR #387 merge); its baseline lacks
the 579 entries and the 5 files above, so its Offline E2E stays red until 0.4
merges into it. The 0.4 tree passes the guard (checked on `7ad219d9`: the guard
and `-SelfTest` both exit 0), and merging 0.4 brings the regenerated baseline
with it. If master moves before that merge, its new commits need a fresh
inventory check on the merge result. (The local `master` branch of the worktree
is behind `origin/master`, at `c2c3e8c4`; it plays no part here.)

## 9. Reproduction

The baselines: `git show 2429d691^1:tools/physical-adapter-policy-baseline.json`
against the file at `2429d691`. Key of an entry: `(file, category, fingerprint)`.
The findings are the keys with a higher count on the right; the merge entry is
the one key of the 579 that master's own scan does not have.

```python
import json
def load(text):
    return {(f['file'], o['category'], o['fingerprint']): o['count']
            for f in json.loads(text)['files'] for o in f['occurrences']}
old, new = load(first_parent_baseline), load(merge_baseline)
added = {k: c - old.get(k, 0) for k, c in new.items() if c > old.get(k, 0)}
# len(added) == 579, sum(added.values()) == 606
```

Mapping an entry back to its literal needs the scanner, not the baseline
(the baseline holds only hashes): scan the sources at the same revision and
compare fingerprints. The CSV next to this file carries the result: origin,
review category, verdict, reason, file, current lines, item, guard category,
count, the literal (ASCII-escaped) and the full fingerprint.

## 10. Checkpoint 2: what was done and the final counts

Order of the work: `feat/0.4` `2cfe46a9` (track ddl's wave 1) merged into the branch first; it touches none of
`tools/`, `src/mssql_dump/`, `src/module_blob.rs` or `src/metadata_model/`, so the review of section 5 stands
unchanged.

### 10.1 G1, commit `8255560c`

- `tools/validate-physical-adapter-policy.ps1`: a new scanner method `TestOnlyModules` (C# 5, so Windows
  PowerShell 5.1 compiles it) lists the modules a file declares as `mod name;` under a cfg predicate that cannot
  be true outside tests; `Get-ScopedFiles` leaves out `name.rs`, `name/mod.rs` and everything below the module's
  directory (for a host that is not `mod.rs`, next to its own directory). `$ExcludedMssqlModules` keeps only
  `mxl_ir.rs` and `moxel.rs`. `tools/physical-adapter-policy-guard.md` and the OpenSpec requirement
  (`openspec/changes/guard-physical-adapter-policy/specs/...`) say so.
- The scope self-test (`Invoke-ScopeSelfTest`) builds a synthetic tree: a declared test module and its child (excluded),
  a `pub(crate)` one with `mod.rs` and a child (excluded), a module of a host file that is not `mod.rs` (excluded;
  the same-named sibling file is *not* its module and stays guarded), `#[cfg(any(test, feature = ...))]` (guarded), a
  plain module (guarded), a `#[path = "..."]` module (guarded), an inline test module (already removed by the token pass).
  It also checks that a UUID added to an excluded file is allowed and to a guarded one is rejected. With the rule turned
  off the self-test fails.
- Proof: **removal-only baseline diff**, entries by `(file, category, fingerprint)`: 8 755 to 8 605 entries, 10 035 to
  9 864 occurrences, files 42 to 40; removed exactly the 150 R1 entries (171 occurrences) and the two file records
  `src/mssql_dump/revision_mix_tests.rs` (47) and `src/mssql_dump/tests/onecdec.rs` (103); nothing added, nothing raised.
  The guard passes, and `-SelfTest` passes under `pwsh` and Windows PowerShell 5.1 (3 s each; the guard 5-11 s).

### 10.2 G3, commit `0bfc2f59`

- `src/metadata_model/index.rs`: `ROOT_COLLECTIONS` (the 46 `(folder, kind)` pairs) and `NESTED_COLLECTIONS` (the five
  nested kinds) are the only table. `kind_of_collection` reads both; the new `collection_of_kind` reads the first.
  `mssql_dump::root_family_folder` is one call to `collection_of_kind` (its three users keep the signature).
- `src/metadata_model/collection_tests.rs` carries the two old tables as data and checks: every one of the 46 kinds in
  both directions and through `root_family_folder`; the five nested kinds (a collection name, no root family: `None` from
  `collection_of_kind` and `root_family_folder`); a set of names that are neither kind nor folder, and that a folder is
  not a kind and a kind is not a folder; the table is exactly the two old ones (46 and 5, no duplicate kind or folder).
- Proof: the four new tests pass; the full lib suite passes (`cargo test --locked -p ibcmd-rs --lib --no-default-features`:
  3 328 passed, 9 ignored, 0 failed); the #387 integration tests pass (`cf_export` 4, `cf_roundtrip` 2,
  `cf_native_roundtrip` 2 + 1 ignored, `cf_load` 16, `cf_load_compiled` 12 + 1 ignored, `cf_overlay` 5,
  `external_export` 7, `extension_export` 11, `extension_adopted` 8, `form_events` 4, `cf_corpus` 2; `external_corpus`
  is ignored without `IBCMD_ONECDEC_CORPUS`); the baseline diff is **removal only**: 8 605 to 8 559 entries, exactly
  the 46 R3 entries, all in `mod.rs`.
- Offline equality on the БСП rows, a binary built before the change against a binary built after it (`iter` profile, the
  same rows, `mssql-dump-config --rows-dir ... --extract-metadata-xml --extract-module-text --no-binary-rows`, each run
  under the heavy lock; trees compared by SHA-256 with `compare_trees_fast.py`): БСП 8.3.27 (`--platform=8.3.27`) **12 199
  of 12 199 files identical**, БСП 8.5 (`--platform=8.5.1`) **12 338 of 12 338 identical**, no file on one side only. No
  УХ run for G3, as decided.

### 10.3 Filed

- #413 (milestone 0.8): one standard-picture table, with the evidence (0 hits of the six names in the native trees of
  БСП 8.3.27, БСП 8.5, УХ 8.3.27, УХ 8.5 and the seven БСП extension trees) and the proof plan of G2.
- #414 (milestone 0.8): the adopted-form text patching of `form_extension.rs` (15 entries: 13 of #387, 2 of
  `with_action_call_types`), with the proof on the four БСП extensions.

### 10.4 The final counts

| | Entries | Occurrences |
|---|---|---|
| findings of #387 | 578 | 605 |
| fixed and out of the baseline (G1 150, G3 46) | 196 | 217 |
| deferred, still in the baseline (G2, #413) | 205 | 205 |
| kept, with the reason of section 5 (13 of them also #414) | 177 | 183 |
| plus the merge entry, kept | 1 | 1 |

The baseline now holds 8 559 entries, 9 818 occurrences and 40 files (before this task: 8 755, 10 035, 42). The CSV has
the decision of every finding and, in `in_baseline_now`, whether it is still in the baseline. Master is unchanged by
this work: it turns green when 0.4 merges into it.
