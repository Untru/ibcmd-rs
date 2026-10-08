# #345: source implementation for measured object bodies

This patch extends `apply-source-change` on 8.3.27.2214 to the 26 existing
ObjectModule/ManagerModule roles listed in `metadata-coverage-wave3.md` and
the three native C1 parent/type pairs: Report/SpreadsheetDocument,
DataProcessor/HTMLDocument and ExchangePlan/TextDocument. It does not close
#345. The author executes no compiler or tests. ROOT's first formatted
revision compiled and ran 3,958 library tests: 3,946 passed, one new test
failed and 11 were ignored. The failure was an unjustified invalid-byte
assumption in a negative fixture, now replaced by a real selected-file
read failure. This revision adds five initial-dependency regressions.
ROOT's second full run compiled 3,963 tests: 3,951 passed, one new fixture
failed and 11 were ignored. The new fixture incorrectly supplied a live
tail-backup path to Online/Worker; ROOT now supplies it only to Live. The
corrected fully qualified test passed and checks all three script modes.
An earlier short-name exact filter selected zero tests and is not counted
as validation. Four further regressions exercise nonempty pending/Params,
the three real template shapes and the exact graph row/byte boundaries;
the seven-test `source_dependency_` filter passed with no failures.
Independent review of both the production changes and those regressions
found no new P1/P2 in the reviewed scope. ROOT's locked, offline workspace
all-targets check with default features disabled passed in 1m19s. The
previous attempt encountered a full F drive while writing its log and is
not accepted. The complete current library suite has not been rerun after
the four further tests. Client measurements and the remaining acceptance below
are still required; the original failed logs are retained on F.

## Resulting route

Object-owned bodies now read a complete active Config metadata graph using
the existing full exporter. A full root candidate inventory, exact emitted
cardinality, selected-body presence and the source/active UUID-to-storage
routes are required. A scoped export cannot stand in for this graph.
Unrelated opaque assets do not become writable: the selected assets' own
diagnostics remain refusals, and their compiler and activation checks still
run. CommonModule/CommonForm retain their existing bounded routes.

The proposed tree starts as a copy of the active export. Only the chosen
body closure is overlaid. Its whole-tree classifier rejects descriptor,
sibling, file-addition, deletion and path-alias changes. A template is a
typed source body: its metadata descriptor fixes the measured parent/type
pair; an HTML body includes Template.xml and every existing file below
Ext/Template/. The original asset shape must match both trees. Selecting
the HTML manifest also detects changes to its pages/resources.

Object bodies stage via the existing file-selection route. Metadata packing
still reads and checks the owner's original base row, but a measured
body-only file selection compiles only the selected ObjectModule or
ManagerModule, or the template body family. It leaves unrelated help,
interfaces and other modules untouched. Metadata-file and mixed/unmeasured
file selections continue through the existing complete preparation route.
The final file-selection trim remains in force. Before the full active
export, source apply now captures a private typed SourceOwnerPreimages:
root, the complete ordinary UUID descriptor inventory, every selected
descriptor/body, pending Config inventory and the Params marker. Original
full headers and raw bytes are retained under the independent existing
2,048-row/32-MiB graph budget. Actual typed ownership and measured suffix
checks reject missing, ambiguous or unsupported bodies. Export/staging
preflights compare this same original inventory again; a post-stage read
cannot replace it. Its exact full-header/SHA and phantom-count guards run
under publication transaction locks in online/live/worker SQL, and under
exclusive ConfigApply table locks before any writes. This new full-owner
route requires the built-in SQL client: external sqlcmd cannot silently
skip the typed initial capture. The 8.5 source-family gate is unchanged.

## Retained evidence and remaining acceptance

The original evidence lives under `F:/ibcmd/lab/05/wave3/metadata/`:

- `evidence/modules-templates-B-source.json`: 26 module files and three
  template edits in a 12,198-file tree, including the HTML page path.
- `evidence/body2-modules-D1-twins.json` and the native/OWN D1 checkpoints:
  existing module payload/lifetime proof. Container headers, registrations,
  ConfigDumpInfo versions and Params differences are explicitly retained;
  payload agreement is not physical parity.
- `evidence/native-template-session-C1.json`: native template-only old B/B,
  new C/C, with modules unchanged. It is not OWN-template acceptance.
- `template-session345-operational-binding-source-v3/source-freeze.json`:
  the existing controller/host Source binding for the pending OWN-template
  experiment. It is not a reusable runtime grant or a completed experiment.

ROOT must measure the new high-level route rather than infer its runtime
behavior from the passing Source regressions or earlier lower-level controls:

1. For each of the 26 module roles, change only the selected existing body
   in an otherwise identical source tree. Retain the full original stage,
   Config/ConfigSave/Params/registration/Files headers and raw data. Prove
   old A/A and new B/B, exact identity of every observed module, complete
   native/OWN export comparisons and explicit physical differences. Verify
   unrelated bodies/owners retain their original data, including when an
   unrelated body cannot be regenerated by the compiler.
2. Run template-only OWN B-to-C against modules B/templates B (the native
   C1 baseline), for all three prepared assets. Use the accepted immutable
   session-controller Source through a newly reviewed current binding and
   exact one-use authority. Prove old B/B, new C/C, modules still B, complete
   body/resource/descriptor groups and full native export comparison. The
   earlier modules-only D1 baseline has templates A; it must not silently
   replace the C1 template-only baseline. If a new A-to-B template control
   is selected, both native and OWN must use that same explicit staging.
3. Verify online behavior; separately exercise live/worker only after their
   current product/preflight gates and lifetime tests pass. This patch does
   not waive the pending full loaded-infobase creator behavior in #409.

## Exact >128-row experiment

Both production 128-row guards are preserved. New regressions generate
complete 128/129-row cohorts, with all original body preimages and the three
service rows, and assert that the small-byte 129-row cohort is still
refused. A byte-only budget change would not constitute native measurement.

Prepare 126 existing distinct measured CommonModule body UUIDs with a
callable marker A and a named probe per module. Change only those bodies
to B. Native `import files --partial` may also stage descriptor companions;
retain the actual stage rather than assume its cardinality. Select enough
existing objects to obtain an observed body-only OWN stage of 126 bodies
plus root/version/versions = 129 rows. Record the exact native stage count,
bytes, largest row, all parts, owner groups and all 126 identities. Also
retain a boundary control of 125 bodies plus three service rows = 128.

Perform native online import/apply first with retained old/new clients;
prove all probes old A/A, new B/B, full source export and full physical
before/staged/after differences. Record actual failure/fallback without
retry if native refuses. After ROOT accepts that measurement, propose the
smallest coherent replacement for both row gates while retaining the
16-MiB stored-row, 32-MiB stage/SQL-plan and 2,048-row/32-MiB metadata graph
limits. Then run the same 129-row OWN cohort and faults at each remaining
independent limit. No such positive dynamic native measurement is currently
claimed.

## Unmeasured families: exact inputs still needed

For nested/visible managed forms, supply the original top-level owner,
child collection, form descriptor, .0 body and unchanged help companions,
before/staged/after headers/raw bytes, native source export and old/new
client observations of the changed visible element and module. Existing
fixture-only decoded layouts do not admit these families.

For RecordSetModule/ValueManagerModule, other owner/module roles and the
remaining template parent/type pairs, supply one existing body per actual
owner/role (not a renamed measured role): its exact collection/UUID/suffix,
full dependency graph, native before/staged/applied rows, source and live
old/new probes. Other template kinds additionally need their full external
resources and retained native descriptor/type/body framing. This patch
keeps those unmeasured routes refused; the complete issue scope remains
open until those cases and the larger-stage controls pass.

## ROOT validation commands (not executed by the author)

Run `cargo fmt --check`, then the focused filters below from this worktree,
followed by the existing affected-module regression suites as appropriate:

```text
cargo test --lib measured_main_modules_cover_exact_native_d1_roles
cargo test --lib template_closure_admits_only_native_c1_parent_type_pairs
cargo test --lib html_resource_edit_pins_whole_existing_asset_and_refuses_owner_drift
cargo test --lib measured_object_module_edit_refuses_sibling_and_descriptor_changes
cargo test --lib measured_owner_routes_require_full_reference_export_and_keep_85_gate
cargo test --lib html_source_apply_overlays_full_existing_asset_and_stages_only_that_owner
cargo test --lib complete_owner_export_refuses_scoped_missing_and_unemitted_root_graphs
cargo test --lib narrow_preparation_keeps_metadata_and_unmeasured_mixed_selections_whole
cargo test --lib measured_selected_module_compiles_without_reading_invalid_sibling_bodies
cargo test --lib complete_129_row_body_cohort_remains_refused_until_native_measurement
cargo test --lib a_small_complete_cohort_over_128_is_not_admitted_by_byte_budget_alone
cargo test --lib source_dependency_capture_retains_original_graph_and_body_before_export
cargo test --lib source_dependency_proof_refuses_original_header_data_and_inventory_drift
cargo test --lib source_dependency_capture_refuses_missing_role_duplicate_and_unbound_bytes
cargo test --lib exclusive_source_initial_preimages_are_checked_under_locks_before_publication
cargo test --lib source_initial_dependency_preimages_precede_all_script_publications
cargo test --lib source_dependency_nonempty_pending_and_params_keep_original_guard_on_mutation
cargo test --lib source_dependency_native_templates_capture_matching_aliases_and_refuse_conflicts
cargo test --lib source_dependency_full_capture_row_boundary_refuses_before_ordinary_blob
cargo test --lib source_dependency_full_capture_byte_boundary_and_pending_cumulative_refusal
```

These tests cover Source/classifier/compiler behavior. They do not replace
client/native measurements. No commit, PR, SQL/native/client operation,
compiler/test/helper Source execution or grant was performed by the author.
