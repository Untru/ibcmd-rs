# Physical-adapter policy guard

Run `pwsh -File tools/validate-physical-adapter-policy.ps1 -RepositoryRoot .`
and `pwsh -File tools/validate-physical-adapter-policy.ps1 -RepositoryRoot .
-SelfTest`.

The guarded production slice is `src/module_blob.rs` plus non-test Rust modules
under `src/mssql_dump/`, plus the non-test Rust modules under the `crates/`
directories that hold code moved out of them (`$GuardedCrateDirectories`):
`crates/ibcmd-1c-text/src/` (the 1C braced-text primitives and scalar fields,
and the source tree's file-name segment, formerly in `src/mssql_dump/mod.rs`,
`source_assets.rs` and `refs.rs`),
`crates/ibcmd-1c-vocab/src/` (the platform-type, standard-attribute, style and
command vocabularies, formerly in `src/mssql_dump/mod.rs`, `source_assets.rs`
and `command_interface.rs`; the role-right vocabulary, formerly in
`src/mssql_dump/role_rights.rs`; and the standard-picture table, formerly
`src/metadata_model/standard_pictures.rs`, which was outside the slice and
joined it with its 221 identifiers, and the compiler's built-in type table,
formerly in `src/metadata_model/types.rs`, which was outside the slice and
joined it with its 103 identifiers and one prefix check),
`crates/ibcmd-form-body/src/` (the form
body reader `form_body.rs`, formerly `src/mssql_dump/form_body.rs`; the
indexes, compatibility rules and timing report it is given, formerly in
`src/mssql_dump/mod.rs`, `refs.rs`, `dcs.rs`, `source_assets.rs`, `timing.rs`
and `form_ref_index.rs`; and the form body record, the event block and
adoption model, the 8.5 layout down-conversion with its group event table and
the data-composition normalizer it reads, formerly in `src/module_blob.rs`,
`src/mssql_dump/form_extension.rs`, `form/layout_8_5_1.rs`,
`form/xml_2_21_writer.rs` and `dcs.rs`; the record's reader with the physical
layout of its Form Attributes tail and the braced-record text primitives both
read, formerly in `src/module_blob.rs`; and the upgrade of an older platform's
root-`42` form body the reader applies first, formerly
`src/module_blob/form_root_42.rs`, which was outside the slice and joined it
with its 22 record tags), `crates/ibcmd-metadata-blob/src/` (the packers of
a configuration's stored bodies from their XML sources, with the source
context they read, formerly `src/module_blob.rs`, which keeps the
command-line wrappers and the entry points that read the rest of the root
and stays in the slice; and the role-rights writer, formerly in
`src/compiler/bodies/`, and the metadata XML element tree, formerly
`src/metadata_model/xml.rs`, which were outside the slice and joined it with
their records), `crates/ibcmd-form-writer/src/` (the native form body writer,
formerly in `crates/ibcmd-metadata-blob/src/lib.rs` with the dynamic-list,
chart and planner writers it calls; its 2.21 route, formerly
`src/mssql_dump/form/{load_8_5_1,xml_2_21_writer,xml_2_21_order}.rs`; the
entry points that dispatch among them, formerly in `src/module_blob.rs`; and
the adopted-form compile, formerly `src/mssql/offline_compile/adopted_form.rs`,
which was outside the slice and joined it with its records),
`crates/ibcmd-module-blob/src/` (the module blob
codec, the raw deflate helpers and digests, the raw-deflated packer and the
versions-blob patcher, formerly in `src/module_blob.rs`; the raw inflater,
formerly in `crates/ibcmd-1c-vocab/src/support.rs`; and the anyhow wrapper of
the V8 container, formerly `src/v8_container.rs`, which was outside the slice
and joined it with no occurrence) and
`crates/ibcmd-moxel/src/root_helpers/`.  Code that leaves the slice for a
crate takes the slice with it: its directory is listed
there and its baseline entries move to the new path in the same change, never
dropped.  Every `*.rs` file below a listed directory is scanned, so a file added
there is in the slice at once and fails as unbaselined until its record is
reviewed, a file deleted from one fails while its record is in the baseline,
and a listed directory that is missing fails the guard.  A crate made
only of moved code is listed by its `src/`.  The MOXCEL decoder itself
(`crates/ibcmd-moxel`, formerly `src/mssql_dump/moxel.rs` and `mxl_ir.rs`) was
outside the slice and stays outside it, so only the module directory that holds
what it took from the slice is listed; its root entry points in
`src/mssql_dump/moxel_entry.rs` are guarded.
A module the compiler reads only for tests is not production and is left out
with every file below its directory: the guard finds it from the declaration
`#[cfg(test)] mod name;` (also `pub`, or with other attributes) in a scoped
file, so `tests.rs`, `tests/`, `metadata_order_tests.rs` and
`revision_mix_tests.rs` need no listing and the next `*_tests.rs` file needs
none either.  A module with a
`#[path = "..."]` attribute is not resolved and stays guarded.  Inline items are
excluded only when their `cfg(...)` predicate requires `test=true`; ambiguous
or partly-production predicates remain guarded.  Schema-owned accessor calls without local literals
add no inventory.  This intentionally does not cover `src/source_oracle.rs`,
CLI/parity documentation, or MXL/MOXL production.

The baseline holds hashes and counts only.  A new scoped file, occurrence, or
increased count fails; removing entries from source passes.  To intentionally
add a reviewed compatibility policy, run the validator with `-WriteBaseline`,
inspect the hash-only diff alongside the source change, and commit both.  Do not
use a baseline refresh to conceal unrelated literals.  Fingerprint inputs
canonicalize CRLF and bare CR to LF, so platform line endings do not churn the
baseline.  A fingerprint hashes the whole logical statement around the literal,
and the scanner's paren depth runs across the file, so code that moves can
change fingerprints without changing its literals: a `const` made `pub`, or a
`match` whose arms the scanner splits differently in the new file.  A move
edits the baseline record by record, lists every changed fingerprint, and keeps
the records of files it does not touch, stale ones included.

`crates/ibcmd-metadata-source/src/` also belongs to the guarded slice. It
holds the `MetadataSource` trait through which the writers read the source
tree, which the packers' source context implements, and the source bytes and
memoised-answer types the trait returns, formerly in
`crates/ibcmd-metadata-blob/src/lib.rs`. The moved types hold no guarded
literal, so its one file joins the baseline with no occurrence and no record
of `ibcmd-metadata-blob` changes.

`crates/ibcmd-compiler-families/src/` also belongs to the guarded slice. It
contains the native family compilers, their bootstrap artifact/graph/identity
types, compiler axes and profile loader, formerly outside the slice under
`src/compiler/` and `src/profile_registry.rs`. All 798 existing production
fingerprint records join the slice unchanged; the move changes no fingerprint
or occurrence count. The earlier baseline records, including stale records,
remain untouched. The loader still embeds the canonical workspace `profiles/`
documents, with adjusted relative include paths and unchanged provenance names.
The subsequent mechanical clippy cleanup folds the identity collector's nested
configuration check into a short-circuiting let chain. Its one name-policy
fingerprint changes from
`06f20cdf6d98a912cc6698719fda5b234053dbc39bd3a4a6708ba1332a9289fb` to
`a1eb8c22ec2bfbf1d9159e97d2da8911a0a0d134b01e0e3abca8a2a4dc644539`,
with category and occurrence count (one) unchanged.

`crates/ibcmd-sql/src/` contains the database clients formerly in `src/sql/`,
which were outside the slice. Eight production files join it with their four
existing fingerprint records unchanged. The feature-gated live test module
stays excluded by its test-only declaration. All earlier records, including
the two stale records, remain unchanged.

`crates/ibcmd-restructure-engine/src/` and the existing portable model crate's
new `crates/ibcmd-restructure/src/descriptor/` directory also belong to the
slice. The engine holds the native restructuring planner, caches, database
execution and search-information records formerly under `src/restructure/`
and `src/mssql_config_apply/si.rs`, plus the apply model's text primitives.
The descriptor directory holds the shared object layouts and adoption
identity formerly under `src/metadata_model/objects*.rs` and
`src/mssql_dump/extension/mod.rs`. The portable model crate's earlier source
stays outside this slice, as the earlier MOXCEL decoder does. All 132 earlier
file records, including stale records, remain unchanged. Thirty-eight new
production file records join with their 300 existing fingerprint records.
The two public descriptor UUID constants retain their values and counts;
their fingerprints change only because `pub(crate)` becomes `pub`. The exact
before/after records are listed in the extraction's evidence. No baseline
refresh is used. Root CLI/settings/apply adapters remain in the root.

The guard tokenizes logical Rust statements outside comments and provably
test-only items, including multiline, raw-string, escaped-string and char forms.
It conservatively inventories immediate XML sink fragments and literal
concatenations.  It remains lexical inventory governance, not a full Rust parser
or general data-flow analysis, and cannot replace migration of XML policy into
schema-owned writers.

The standard common-command-group table in
`crates/ibcmd-1c-vocab/src/command_groups.rs`, formerly
`src/mssql_dump/mod.rs::COMMON_COMMAND_GROUPS`, is part of the same guarded
vocabulary directory. Its eleven existing UUID fingerprints and occurrence
counts move unchanged, record by record. The root retains its crate-visible
alias. All other existing file records, including stale records, remain
unchanged; neither the scope nor the baseline is regenerated.

`crates/ibcmd-dump-readers/src/` follows the same rule. The additional-index
reader, its owner model and the evidence-bound Configuration properties reader
moved from `src/mssql_dump/{additional_indexes,configuration_properties_evidence}.rs`.
Their two baseline file records move to the corresponding crate paths. All 11
existing additional-index fingerprint records keep their category, hash and
count; the Configuration reader and the new crate root have empty occurrence
lists. No fingerprint changes. Every other file record is preserved, including
the stale `config_dump_info.rs` and `incremental.rs` records. The source assets
module keeps the former owner-type path through an alias. Only existing text
and metadata-writer crates replace root aliases; no parsing policy changes.

Step 5f-1 of #559 (the form writer's preparation) adds three files to
guarded directories. `crates/ibcmd-1c-text/src/namespaces.rs` holds the 2.21
palette namespace rule, formerly in `src/mssql_dump/source_assets.rs`; its
four records (one `name-special-case`, three `xml-policy`) move unchanged.
`crates/ibcmd-1c-text/src/brace_list.rs`, the brace-list field splitter
formerly in `src/external/brace.rs`, and `crates/ibcmd-form-body/src/form_layout.rs`,
the stored form layout formerly in `src/platform/mod.rs`, were outside the
slice and join it with no occurrence. In `src/mssql_dump/form/xml_2_21_writer.rs`
one `xml-policy` fingerprint changes around an unchanged literal, because the
statement now names `ibcmd_form_body::parse_form_event_block` by its crate
path: `141bb3eb8dce1bd7cb7f0477f893d0e98d2e920fdd384fbb8d1c6311d39817a8` to
`3d272659525c543c2c5a548b0eec5ba3c5561a10d5023edb09bb1d094603420e`, count one.
Every other record, the two stale ones included, is unchanged.

Step 5f-2 of #559 moves the native form writer into `crates/ibcmd-form-writer/src/`.
The six whole files (`dynamic_list.rs`, `form_chart.rs`, `form_planner.rs`
from `crates/ibcmd-metadata-blob/src/`, and `form/load_8_5_1.rs`,
`form/xml_2_21_writer.rs`, `form/xml_2_21_order.rs` from
`src/mssql_dump/form/`) keep their records unchanged. The 2 088 records
(2 259 occurrences) of `crates/ibcmd-metadata-blob/src/lib.rs` go to the file
that holds each statement now: 1 605 / 1 742 to `crates/ibcmd-form-writer/src/lib.rs`
and 492 / 518 to `crates/ibcmd-metadata-blob/src/lib.rs`; eight records whose
statement is in both files split their count between them. Two fingerprints
change around an unchanged literal, count one each: `NATIVE_ZERO_UUID`
became `#[doc(hidden)] pub` for the packers (`uuid-literal`
`79321f56ff08c266e5bef695e597cbdb4703976af4543fa7f65d71ba384dd082` to
`739a0a806368259e55cc24fa2c21b12f3469d183731d2b01c8eeda2bdea78696`), and
the `b"BaseForm"` prefilter of `compile_native_form_body_through` moved from
`src/module_blob.rs` into the crate's `lib.rs`, its statement naming
`crate::adopted_form::` (`name-special-case`
`c75167dd7e5bd797140c48ef96cea1ccdada78ea1a954d18d29d6a2cbfd43e3e` to
`2047a68de8f2eab7285e22838c5231b344c7cd023239685ad0d51e32ae8de552`).
`src/module_blob.rs` stays in the slice with no occurrence.
`crates/ibcmd-form-writer/src/adopted_form.rs` was outside the slice and
joins it with its 32 records (33 occurrences). Every other record, the two
stale ones included, is unchanged.

`crates/ibcmd-compiler-readiness/src/` and `crates/ibcmd-parallel/src/` also
belong to the slice. The first holds the compiler's bootstrap-readiness
manifest and build preflight, formerly `src/compiler/readiness.rs`; it still
embeds the canonical workspace `compatibility/bootstrap.json`, with an
adjusted relative include path. The second holds the process-wide worker
pools, formerly `src/parallel.rs`, whose root module re-exports them so the
pool and requested-worker state keep one owner. Both files were outside the
slice and join it with no occurrence. Every other record, the two stale ones
included, is unchanged.

The compiler-coordinator preparation of #559 moves the base-free
command-interface writers and what they read out of the root, in three
guarded directories:
- `crates/ibcmd-1c-vocab/src/`: the stored command-interface codes
  (`COMMAND_INTERFACE_PLACEMENTS`, `HOME_PAGE_WORK_AREA_TEMPLATES`, the
  standard panel definitions) go to the new `command_interface.rs`, and
  `command_interface_standard_command_for_code` to `commands.rs`, all from
  `src/mssql_dump/command_interface.rs`. Their 16 records (5
  `uuid-literal`, 11 `name-special-case`) move unchanged, record by record;
  the root keeps crate-visible aliases.
- `crates/ibcmd-compiler-families/src/compiler/bodies/`: the body-profile
  selection (`mod.rs`, formerly in `src/compiler/bodies/mod.rs`) and the
  command-interface and interface-asset codecs (`command_interface.rs`,
  `interface_assets.rs`, formerly `src/compiler/bodies/`) were outside the
  slice and join it: `mod.rs` with no occurrence, `command_interface.rs`
  with 8 records (1 `uuid-literal`, `NIL_UUID`; 7 `name-special-case`, the
  stored codes `"0"`/`"1"` and the `"B"` type tag) and
  `interface_assets.rs` with 3 `uuid-literal` (`NIL_UUID` and the client
  application group and panel markers).
- `crates/ibcmd-metadata-blob/src/interface_assets.rs`: the writers,
  formerly `src/module_blob/interface_assets.rs`, which was outside the
  slice (only `src/module_blob.rs` itself is scanned), join it with their 51
  records (1 `uuid-literal`, 34 `name-special-case`, 16 `xml-policy`). The
  three entry points that moved with them from `src/module_blob.rs` held no
  record. The mechanical clippy fixes that follow change two of the 51
  fingerprints around unchanged literals, count one each:
  `name-special-case` `3d91910acab3e05a938ebf1f16b28b9f6adf360ab3407255072b188c8876d7b1`
  to `615f82b54916833454c163b18af6032fc0338f0dc73208564b88eb8fa5b7ab5c`
  (an `else if` folded into a let chain) and `xml-policy`
  `2276ddf5fa7ac2b23903956bd92e99361daa8ef50f1b24c9bc698e3db30e019a` to
  `593ef6aa56a2e5ddcabd698a0e9d99fd1d4b294d1fa3d3445f48ce1dcacefd16`
  (`a == !b` written `a != b`).
No directory is added to `$GuardedCrateDirectories`. Every other record, the
two stale ones included, is unchanged.

`crates/ibcmd-compiler/src/` also belongs to the slice (#559, the compiler
coordinator). It holds the coordinator formerly under `src/compiler/`
(`mod.rs`, `bootstrap.rs`, `root.rs`, `version.rs`, `versions.rs`,
`overlay.rs`, `external_intake.rs`, `external_owned.rs`, `identity.rs`), the
managed-form codec and the template kind formerly in `src/compiler/bodies/`
(`form.rs`, `template_kind.rs`) and `src/file_identity.rs`. All of them were
outside the slice and join it with their 167 records (169 occurrences):
- `compiler/bootstrap.rs`: 88 `name-special-case` records, 90 occurrences
  (the source tree's `.xml` and `MetaDataObject`, the Configuration root's
  child family kinds and the Configuration property names it projects), and
  4 `xml-policy` (the placeholder values of its property-projection
  refusals);
- `compiler/root.rs`: 58 `uuid-literal` (the Configuration family slot
  table, the class ids and `NIL_UUID`) and 1 `name-special-case`
  (`"Configuration"`);
- `compiler/external_intake.rs`: 3 `name-special-case` (`ChildForms`,
  `ChildTemplates`, `.xml`);
- `compiler/external_owned.rs`: 4 `name-special-case` (`TemplateType`,
  `.xml`, `.txt`, `text/`);
- `compiler/bodies/template_kind.rs`: 8 `name-special-case` (the template
  kind names `TemplateKind::parse` matches);
- `compiler/bodies/form.rs`: 1 `xml-policy` (the `"<absent>"` layout marker
  of a refusal);
- `lib.rs`, `file_identity.rs`, `compiler/{mod,identity,overlay,version,versions}.rs`
  and the two facades `compiler/bodies/mod.rs` and `compiler/families/mod.rs`:
  none.
The root's facades (`src/compiler/mod.rs`, `src/compiler/bodies/mod.rs`,
`src/file_identity.rs`) stay outside the slice. No fingerprint changes. Every
other record, the two stale ones included, is unchanged.

`crates/ibcmd-platform/src/` also belongs to the slice (#559, the portable
platform contract). Its `lib.rs` holds the portable platform registry
formerly in `src/platform/mod.rs`, which was outside the slice; it joins it
with 4 `name-special-case` records, count one each: the `"8.3"` and
`"8.5.1"` arms of `parse_form_layout` (the `platform.form_layout` values)
and the `"2.20"` and `"2.21"` arms of `build_entry` (the
`platform.xml_format` values). `tests.rs` is a test-only module. The root's
`src/platform/mod.rs` stays outside the slice. No fingerprint changes.
Every other record, the two stale ones included, is unchanged.

`crates/ibcmd-metadata-model/src/` also belongs to the slice (#559, the
metadata model). Its `metadata_model/` holds the model formerly under
`src/metadata_model/`, which was outside the slice; the 34 production files
join it with their 1 312 records (1 344 occurrences: 350 `uuid-literal`, 902
`name-special-case`, 92 `xml-policy`). The largest are `export.rs` (153
records), `objects_parts.rs` (125), `common.rs` (105), `export_names.rs` (84),
`root.rs` (81), `registers.rs` (76), `registers_parts.rs` (74) and
`common_export.rs` (62); `lib.rs`, `metadata_model/brace.rs` and
`metadata_model/objects.rs` have none. `metadata_model/slot_evidence_tests.rs`
and `test_scratch.rs` are test-only modules. The root's facade
(`src/metadata_model.rs`) and the tests that stay in the root
(`src/metadata_model/{collection_tests,external_data_source}.rs`) are outside
the slice. Each record equals the one a scan of the same file as it stood
under `src/metadata_model/` gives: the move's edits (the `ibcmd_parallel`
import, rebased `include_str!` paths, `#[doc(hidden)] pub` items) and the
clippy fixes that follow change no fingerprint. Every other record, the two
stale ones included, is unchanged.

`crates/ibcmd-mssql-live/src/` also belongs to the slice (#559, the MSSQL
main activation and its live track). It holds the modules formerly at
`src/{mssql_platform_profile,profile_process,rac_process,mssql_main_activation,mssql_live_continue,mssql_live_gate,mssql_recovery_artifact,mssql_live_artifact,mssql_artifact,mssql_worker_switch}.rs`
and the managed worker's lifetime authority formerly under
`src/mssql_managed_worker{.rs,/}` (`child`, `command`, `identity`, `native`,
`native/authentication`, `undo`), all outside the slice. Its 18 production
files join it with 36 `name-special-case` records (37 occurrences):
`mssql_main_activation.rs` 13, `mssql_platform_profile.rs` 8,
`mssql_live_continue.rs` 5, `mssql_recovery_artifact.rs` 3,
`mssql_live_artifact.rs` 2, `mssql_live_gate.rs` 2,
`mssql_managed_worker/native.rs` 2 (3 occurrences) and
`mssql_managed_worker/undo.rs` 1; the other ten have none. The test files
(`mssql_managed_worker/{tests,undo/tests,native/census_observer_tests,native/authentication/tests}.rs`,
`test_scratch.rs`) are test-only modules. Each record equals the one a scan
of the same file as it stood under `src/` gives, except two whose statement
a clippy fix rewrote around unchanged literals in `mssql_live_artifact.rs`
(`name-special-case`, count one each): `"ibcmd-recovery-"`
`f66333428b825d39464b92a66eb98c887e622b26973f9b5b280ecdd7d758cc77` to
`c767f1c57d7ecc242aa56303c259fdb52d8535cd46d4257f1b5ec588fcd38fab`, and
`".pack"` `3aed7d93c8326c1cd357da9d39bb2a72a3d544c8b58e8a256604ae748b2ca2f4`
to `e3e923154d2b4d5f7c65fd510342f4bc6bee1900cff91eec54e148c16527b63b`. The
root's facades and the managed worker's command glue
(`src/mssql_managed_worker/cli.rs`) stay outside the slice. Every other
record, the two stale ones included, is unchanged.

The body codecs of #559 join `crates/ibcmd-compiler/src/`, which is already
in the slice. `compiler/bodies/{dcs,mxl,mxl_native,predefined,rights,support,template}.rs`,
formerly under `src/compiler/bodies/`, were outside the slice and join it
with 452 records (464 occurrences):
- `compiler/bodies/mxl_native.rs` (the spreadsheet writer): 424 records,
  431 occurrences (13 `uuid-literal`; 321 `name-special-case`, 326
  occurrences; 90 `xml-policy`, 92 occurrences);
- `compiler/bodies/rights.rs`: 1 `uuid-literal`, 9 `name-special-case`
  (10 occurrences);
- `compiler/bodies/dcs.rs`: 4 `name-special-case` (5 occurrences), 2
  `xml-policy` (3 occurrences);
- `compiler/bodies/predefined.rs`: 2 `uuid-literal`, 4 `name-special-case`;
- `compiler/bodies/mxl.rs`: 4 `name-special-case`;
- `compiler/bodies/template.rs`: 2 `name-special-case`;
- `compiler/bodies/support.rs`: none.
`compiler/bodies/dcs_2_21.rs`, formerly `src/mssql_dump/dcs_2_21.rs`, keeps
its 4 `xml-policy` records unchanged at the new path. Each record equals the
one a scan of the same file as it stood under `src/` gives: the edits that
prepared the move (crate paths, the rights codec's extension parameter,
`#[doc(hidden)] pub` items), the move's own (rebased fixture paths, the
spreadsheet profile fixture's `test-fixtures` gate, the split-out root
tests) and the clippy fixes that follow change no fingerprint. The root's
facades and the tests that stay in the root
(`src/compiler/bodies/{mxl_native_root_tests,mxl_root_tests}.rs`) are
outside the slice. Every other record, the two stale ones included, is
unchanged.

`crates/ibcmd-apply-check/src/` also belongs to the slice (#559, the
restructuring check of a configuration apply). Its `apply_check/` holds the
check formerly under `src/apply_check/`, which was outside the slice. The
crate's 19 guarded files join it with 163 records, 164 occurrences (11
`uuid-literal`, 109 `name-special-case`, 43 `xml-policy` records with 44
occurrences):
- `apply_check/roles.rs`: 46 `name-special-case`;
- `apply_check/rules.rs`: 27 `name-special-case`;
- `apply_check/trees.rs`: 8 `name-special-case`, 1 `xml-policy` (2
  occurrences);
- `apply_check/dbtree.rs`: 2 `name-special-case`, 3 `xml-policy`;
- `apply_check/s1.rs`: 5 `name-special-case`;
- `apply_check/plan.rs`: 4 `name-special-case`;
- `apply_check/upgrade.rs`: 3 `uuid-literal`, 1 `name-special-case`;
- `apply_check/check.rs` and `apply_check/model.rs`: 2 `name-special-case`
  each;
- `apply_check/dbtree_tests.rs` (5 `uuid-literal`, 9 `name-special-case`,
  15 `xml-policy`) and `apply_check/trees_tests.rs` (3 `uuid-literal`, 3
  `name-special-case`, 24 `xml-policy`): unit tests, but their modules are
  declared with `#[path = "..."]`, which the guard does not resolve, so they
  stay in the slice;
- `lib.rs`, `apply_check/{mod,cli,descriptor,root_row,rule_id,sql,tree_diff}.rs`:
  none.
`apply_check/{corpus_tests,record_format_tests,s1_matrix_tests,s1h_corpus_tests}.rs`
and `test_scratch.rs` are test-only modules. The root's facade
(`src/apply_check.rs`) stays outside the slice. Each record equals the one a
scan of the same file as it stood under `src/apply_check/` gives: the move's
edits (the crate paths of two `module_blob` imports, a rebased
`include_str!` path) and the clippy fixes that follow change no
fingerprint. Every other record, the two stale ones included, is unchanged.

`crates/ibcmd-cancel/src/` also belongs to the slice (#559). Its `lib.rs`
is the process-wide cancellation flag formerly `src/cancel.rs`, which was
outside the slice; the root's `cancel` module re-exports it, so the flag
keeps one owner. The file joins the slice with no occurrence. Every other
record, the two stale ones included, is unchanged.

The config-apply seams of #559 change the slice in two places:
- `crates/ibcmd-mssql-live/src/mssql_main_activation.rs` takes the main
  activation's inputs (`read_activation_inputs`, `allowed_activation_targets`
  and the marker rule), formerly in `src/mssql.rs`, which was outside the
  slice. The statement that names the three service rows
  (`"root" | "version" | "versions"`) joins it with three
  `name-special-case` records, count one each; the file's other records are
  unchanged.
- `crates/ibcmd-dynamic-generation/src/` also belongs to the slice. Its
  `lib.rs` is the dynamic-generation overlay formerly
  `src/mssql_dump/dynamic_generation.rs`, with its process-wide registry of
  installed overlays and `stored_row_name`, formerly in
  `src/mssql_dump/mod.rs`. The moved file held no occurrence: the crate's
  `lib.rs` joins with an empty record, and the root's facade at the old path
  keeps that path's empty record. The live lab test
  (`src/mssql_dump/dynamic_generation/live.rs`) is a test-only module.
No fingerprint changes. Every other record, the two stale ones included, is
unchanged.

`crates/ibcmd-config-apply/src/` also belongs to the slice (#559). It holds
the own configuration apply, formerly `src/mssql_config_apply/` (without
the developer command, which stays in the root as
`src/mssql_config_apply_command.rs`), the S1 structural gate, formerly
`src/restructure/s1.rs`, and the guard of its structure phase, formerly
`src/restructure/size_guard/phase.rs`; all were outside the slice. Its
production files join with 212 records (214 occurrences):
- `mssql_config_apply/dynamic_overlay.rs`: 25 records;
- `mssql_config_apply/dynamic_metadata.rs`: 23;
- `mssql_config_apply/dynamic.rs`: 14;
- `mssql_config_apply/mod.rs`: 11;
- `mssql_config_apply/gate.rs`: 7;
- `mssql_config_apply/objects.rs`: 5;
- `mssql_config_apply/params_marker.rs`: 4;
- `mssql_config_apply/{dynamic_platform85,model,recovery,removals}.rs`: 3
  each;
- `restructure/s1.rs`: 2 `name-special-case`;
- `mssql_config_apply/registrations.rs`: 1;
- `mssql_config_apply/{gate_parts,objects_analysis,removals,synonyms}_tests.rs`
  (10, 27, 54 with 56 occurrences, and 17 records): unit tests, but their
  modules are declared with `#[path = "..."]`, which the guard does not
  resolve, so they stay in the slice;
- `lib.rs`, `restructure/mod.rs`, `restructure/size_guard/phase.rs` and
  `mssql_config_apply/{check_gate,errors,si,sqlgen,synonyms,versions}.rs`:
  none.
`mssql_config_apply/dynamic/activation_tests.rs`,
`restructure/{tests_s1,tests_plan,tests_sections}.rs` and `test_scratch.rs`
are test-only modules. The root's facades (`src/mssql_config_apply.rs`,
`src/restructure/s1.rs`) stay outside the slice. Each record equals the one
a scan of the same file as it stood at its old path gives, except two: the
clippy fix of the S1 gate's blocker filter changes the statement that holds
its two `name-special-case` records, whose fingerprints change with it
(category, count and literal unchanged):
- `"deleted"`: `797728d2303e010ac925f5f8d2c8ca4d0cb3fc0e537cf952b95d27771a6abb42`
  -> `33d61819e63890a2c8292d9506c27226dc1be273779940a5905940772c412a83`;
- `"root"`: `0c8ac485677fde8beacc402785e5960dcc28442d8eb3846e67575e2d53f1b32e`
  -> `6d47de36a6e7b6b493fc52ea5725d0fb774c94b196483bd62f9079f419200cd9`.
Every other record, the two stale ones included, is unchanged.

The exporter's prerequisite leaf modules of #559 join the slice in four
crates; all of them were outside it:
- `crates/ibcmd-dump-readers/src/` (already listed) takes
  `metadata_owner_graph.rs`, formerly `src/metadata_owner_graph.rs`, with
  41 records (45 occurrences: 19 `uuid-literal`, 22 `name-special-case`);
  `external/brace.rs`, formerly `src/external/brace.rs`, with none; and
  `extension/adoption.rs` (4 `name-special-case`, 1 `uuid-literal`) and
  `extension/predefined.rs` (10 records, 12 occurrences: 5
  `name-special-case`, 5 `xml-policy`), formerly under `src/extension/`.
- `crates/ibcmd-sql/src/` (already listed) takes `runtime_evidence_schema.rs`,
  formerly `src/runtime_evidence_schema.rs`, with 13 records (5
  `name-special-case`, 8 `xml-policy`).
- `crates/ibcmd-platform-spec/src/` (new) holds `legacy_version.rs`,
  `platform/mod.rs` and `adapters/mssql_legacy.rs`, formerly at the same
  paths under `src/`: `mssql_legacy.rs` with 1 `uuid-literal` (the nil
  uuid), the others and `lib.rs` with none. `platform/tests.rs` is a
  test-only module.
- `crates/ibcmd-mssql-extensions/src/` (new) holds
  `mssql_extension_stage.rs`, formerly `src/mssql_extension_stage.rs`, with
  9 `name-special-case` records; `lib.rs` has none.
Each record equals the one a scan of the same file at its old path in the
commit before its move gives. The 19 `uuid-literal` records of
`metadata_owner_graph.rs` differ from a scan of the file before #559's
preparation of it, because the statements that hold them changed from
`pub(crate) const` to `pub const`; they were not in the baseline before.
The root's facades (`src/platform.rs`, `src/adapters/mod.rs`,
`src/mssql_extension_stage.rs`, `src/external/mod.rs`,
`src/extension/mod.rs`) stay outside the slice. No baseline fingerprint
changes. Every other record, the two stale ones included, is unchanged.
