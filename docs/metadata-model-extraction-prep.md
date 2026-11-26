# Metadata model extraction boundary
#559, prepared against master `67772569`.

The model has 25,692 Rust lines in 36 files. Direct root paths, including tests and excluding whole-module self paths: **7 modules / 56 references**. This preparatory change leaves **4 modules / 18 references**; production retains `parallel` and `platform`, and the tests retain `test_scratch` and the exporter collection consistency check.

Since then the platform seam is closed: the portable platform registry is
`crates/ibcmd-platform`, and `DescriptorContext::platform` /
`ExportContext::platform` return its `Platform` (see "Platform contract"
below). The model itself is now the crate `crates/ibcmd-metadata-model`
(see "Extraction" below). The tables keep the inventory as it was prepared,
with the paths of the time.

## Root dependency inventory

| Root owner | Direct references | Preparation / remaining seam |
|---|---:|---|
| brace_list | 18 | Same macro/type from ibcmd-restructure; root macro and its public path remain |
| compiler | 4 | Existing ibcmd-form-native and ibcmd-compiler-families; same functions/arguments |
| module_blob | 8 | Existing metadata-blob context/properties, module-blob inflater and 1c-text encoder; constructor bodies unchanged |
| mssql_dump | 9 | Eight production vocabulary references use ibcmd-1c-vocab. One test retains root_family_folder to check the root exporter facade |
| parallel | 5 | install uses the root configured global pool; move a single owner before extraction, never duplicate their OnceLock/atomic state |
| platform | 7 | Public DescriptorContext::platform and ExportContext::platform return the root PlatformSpec; includes FormLayout and the palette-color feature lookup |
| test_scratch | 5 | Existing root-owned cleanup guards in unit tests; keep cleanup behavior when splitting test ownership |

## Included test helper and complete owner counts

`external_data_source.rs` imports the current crate as `product` and includes
`tests/common/eds_generated.rs`. The literal `crate::` metric excludes this
alias. Counting this compiled include once adds two qualified root-path
occurrences and the `profile_registry` owner: **8 modules / 58 references
before, 6 / 20 after**. The direct 7/56 and 4/18 numbers above remain the
literal-path metric, not the complete closure.

The helper imports these external items:

- `compiler::graph::{ObjectStorageRoute, build_bootstrap_graph}`;
- `compiler::identity::collect_bootstrap_identities`;
- `compiler::root::{ConfigurationBodyProperties, compile_configuration_body}`;
- `compiler::version::SpecialEntryProfile`;
- `profile_registry::load_bundled_profile_registry`.

Its `metadata_model::brace::parse_row` access stays inside the target model.
Graph, identity and profile loading already have portable owners;
`compiler::root` and `compiler::version::SpecialEntryProfile` remain root-owned. The generated-configuration integration
helper therefore cannot be copied into a future model crate unchanged or
made a reverse dev-dependency on the root. Keep that cross-component test in
the root adapter, or split the helper before moving its pure model assertions.
This prep leaves its code, fixture paths and assertions unchanged.

## Exact imported root items before preparation

- `src/metadata_model/attribute.rs`: `crate::brace_list`, `crate::parallel`
  Pool calls: `parallel::install`
- `src/metadata_model/audit.rs`: `crate::module_blob::inflate_raw`, `crate::module_blob::parse_simple_metadata_xml_properties`, `crate::parallel`
  Pool calls: `parallel::install`
- `src/metadata_model/bodies_aggregates.rs`: `crate::brace_list`
- `src/metadata_model/bodies_flowchart.rs`: `crate::brace_list`, `crate::compiler::bodies::form_native`, `crate::compiler::bodies::form_native::format_native_color`, `crate::compiler::bodies::form_native::format_native_font`, `crate::module_blob::encode_base64`, `crate::mssql_dump::standard_picture_uuid`
- `src/metadata_model/bodies_predefined.rs`: `crate::brace_list`
- `src/metadata_model/bodies_rows.rs`: `crate::test_scratch::TestScratch::adopt`
- `src/metadata_model/bodies_value_table.rs`: `crate::brace_list`
- `src/metadata_model/collection_tests.rs`: `crate::mssql_dump::root_family_folder`
- `src/metadata_model/common.rs`: `crate::brace_list`, `crate::compiler::bodies::form_native`, `crate::compiler::bodies::form_native::format_native_color`, `crate::compiler::bodies::form_native::format_native_control_border`, `crate::compiler::bodies::form_native::format_native_font`, `crate::compiler::bodies::form_native::format_native_shortcut`, `crate::mssql_dump::COMMON_COMMAND_GROUPS`, `crate::mssql_dump::standard_picture_uuid`, `crate::platform::FormLayout::V8_5_1`, `crate::test_scratch::TestScratch::adopt`
- `src/metadata_model/common_export.rs`: `crate::compiler::bodies::form_native`, `crate::compiler::bodies::form_native::PALETTE_COLORS_8_5_1`, `crate::compiler::bodies::form_native::PLATFORM_STYLE_COLOR_CODES`, `crate::compiler::bodies::form_native::PLATFORM_STYLE_FONT_CODES`, `crate::compiler::bodies::form_native::WEB_COLOR_CODES`, `crate::compiler::bodies::form_native::WINDOWS_COLOR_CODES`, `crate::mssql_dump::COMMON_COMMAND_GROUPS`, `crate::mssql_dump::standard_picture_name`
- `src/metadata_model/export.rs`: `crate::brace_list`, `crate::platform::PlatformSpec`, `crate::platform::of_xml_dialect`, `crate::test_scratch::TestScratch::adopt`
- `src/metadata_model/export_audit.rs`: `crate::module_blob::parse_simple_metadata_xml_properties`, `crate::parallel`
  Pool calls: `parallel::install`
- `src/metadata_model/external_data_source.rs`: `crate::brace_list`, `crate::compiler::families::native::validate_descriptor_layout`, `crate::module_blob::MetadataSourceContext`
- `src/metadata_model/forms_export.rs`: `crate::mssql_dump::standard_picture_name`
- `src/metadata_model/index.rs`: `crate::parallel`
  Pool calls: `parallel::install`
- `src/metadata_model/interface.rs`: `crate::brace_list`
- `src/metadata_model/mod.rs`: `crate::brace_list`, `crate::module_blob::MetadataSourceContext`, `crate::platform::PlatformSpec`, `crate::platform::of_xml_dialect`
- `src/metadata_model/objects.rs`: `crate::test_scratch::TestScratch::adopt`
- `src/metadata_model/objects_parts.rs`: `crate::brace_list`, `crate::mssql_dump::standard_picture_uuid`
- `src/metadata_model/registers.rs`: `crate::brace_list`, `crate::module_blob::MetadataSourceContext`
- `src/metadata_model/registers_export.rs`: qualified macro `crate::brace_list!`
- `src/metadata_model/registers_parts.rs`: `crate::brace_list`, `crate::mssql_dump::standard_picture_uuid`
- `src/metadata_model/root.rs`: `crate::brace_list`, `crate::module_blob::MetadataSourceContext::new`, `crate::platform::FEATURE_PALETTE_COLORS`, `crate::platform::feature_uuid`
- `src/metadata_model/root_export.rs`: `crate::test_scratch::TestScratch::adopt`
- `src/metadata_model/simple.rs`: `crate::brace_list`
- `src/metadata_model/types.rs`: `crate::brace_list`, `crate::module_blob::MetadataSourceContext`, `crate::parallel`
  Pool calls: `parallel::install`
- `src/metadata_model/websocket_client.rs`: `crate::brace_list`

## Why the crate move stops at preparation

`COMMON_COMMAND_GROUPS` was a real exporter-owned table, read by both model directions and the command-interface writer. Its exact ordered 11-pair value is now owned once by the existing vocabulary crate; the exporter keeps its old crate-visible alias. The collection consistency test remains a root integration check instead of becoming a tautological model-only assertion.

The remaining public platform boundary cannot be removed by an import rename: both context methods return `crate::platform::PlatformSpec`, and model code reads its release, layout, compatibility fields/methods and Display representation. `src/platform*` is reserved for the parallel work. Copying that registry or replacing the public context type would change ownership/API. This PR therefore stops at preparation; it does not claim the metadata-model crate extraction is complete.

The next step is a coordinated portable platform contract, a single portable pool owner, and explicit root-vs-crate test ownership. The MetadataSourceContext constructors retain their original bodies for the parallel MetadataSource trait work. All reserved source trees, including module_blob and its root entry points, are unchanged. No compiler/exporter callbacks or new policy are introduced.

## Guard transition

The vocabulary src directory was already guarded. Its new file takes all eleven UUID records from the existing mssql_dump/mod.rs record. All eleven fingerprints and occurrence counts are unchanged, despite widening the constant for the crate boundary. The other 169 existing file records, including the two stale records, are unchanged. No guard scope change, new dependency or baseline refresh is needed.

## Platform contract

The model's seven `crate::platform` sites now name `ibcmd_platform`:
`DescriptorContext::platform` and `ExportContext::platform` return
`ibcmd_platform::Platform` (re-exported as `crate::platform::Platform`)
from `ibcmd_platform::of_xml_dialect`; `common.rs` compares with
`ibcmd_platform::FormLayout::V8_5_1`; `root.rs` reads
`ibcmd_platform::feature_uuid(FEATURE_PALETTE_COLORS)`. The crate owns the
registry of builds and releases the bundled profiles declare; the root's
`PlatformSpec` wraps each `Platform` with the command line's XML selector
and the MSSQL capabilities, and `PlatformSpec::from(context.platform())`
gives a caller the root's spec. Production code in the model now reaches the
root only through `crate::parallel`, a facade over `ibcmd-parallel`.

## Extraction

`src/metadata_model/**` moved to `crates/ibcmd-metadata-model/src/metadata_model/`
(`git mv`); the crate mirrors the root's tree, so the moved code keeps its
`crate::metadata_model::…` paths. Its only production edit beyond the move
is `crate::parallel` -> `ibcmd_parallel` (the pools' single owner, so their
state is not duplicated); the items the root still names went from
`pub(crate)` to `#[doc(hidden)] pub`. The root's `src/metadata_model.rs`
glob-re-exports the crate and names each widened item crate-visibly, so the
root's paths and public API stay as they were; `src/lib.rs` re-exports
`brace_list!`.

Test ownership:
- the five `TestScratch` users moved with their modules and read a
  crate-private `#[cfg(test)]` copy of `ibcmd-filedb`'s `test_scratch.rs`;
- `collection_tests.rs` stays a root test: it checks the exporter's
  `mssql_dump::root_family_folder` against the model's table;
- the ExternalDataSource tests and their `generated` fixture stay root tests
  in `src/metadata_model/external_data_source.rs`: the fixture
  (`tests/common/eds_generated.rs`) builds a configuration with the root's
  compiler and profile registry.
