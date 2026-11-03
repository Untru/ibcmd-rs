//! The managed-form body reader ([`form_body`]): it reads a stored form body
//! record and writes the form's `Form.xml`. Beside it: the per-export indexes
//! and the compatibility-mode rules its `FormParseContext` holds, the files a
//! form's items carry, the dump timing report its entry points add their parts
//! to, and the code it reads besides them: the form body record and its
//! reader (`ParsedFormBodyBlob`, `parse_form_body_plain`, with the upgrade of
//! an older platform's root-`42` body it applies first), the physical layout
//! of the record's Form Attributes tail (`attributes_tail.rs`), the
//! braced-record text primitives both read ([`record_text`]), an adopted
//! form's event block, base form slot and adoption model (`FormAdoption`), the
//! platform 8.5 layout's down-conversion (`layout_8_5_1`) and the
//! data-composition normalizer (`dcs.rs`), which the root's template exporters
//! share.
//!
//! Everything here moved from the root crate (`mssql_dump`'s `form_body.rs`,
//! `mod.rs`, `refs.rs`, `dcs.rs`, `source_assets.rs`, `timing.rs`,
//! `form_ref_index.rs`, `form_extension.rs`, `form/layout_8_5_1.rs` and
//! `form/xml_2_21_writer.rs`, and `module_blob.rs` with its `form_root_42.rs`),
//! which still reaches every item by its old name and path through imports.
//! The bodies are unchanged but for mechanical clippy fixes (listed in #588,
//! #592 and the step that moved the record reader); declarations and the
//! fields the root reads or fills are `pub`, and the four test helpers the
//! root's own tests call (`parse_form_child_item`,
//! `collect_form_child_item_indexes`, `extract_form_command_interface`,
//! `format_form_attributes_section_xml`) are `#[doc(hidden)] pub`. The
//! builders of these indexes read the root's metadata rows and stay there,
//! and so do the few methods that need a root type
//! (`MetadataFieldDeclarationIndex::written_as` and the sums of
//! `MssqlDumpTimingReport`), as root traits. The reader's root half stays
//! there too (`mssql_dump/form_body_entry.rs`): the extension export running
//! now, which `FormParseContext::new` is given, and the entry points that
//! decode a stored form through the root's codec; so does the record's
//! inflating entry point (`module_blob::parse_form_body_blob`). The tests that
//! decode a template or a form through the root's codec, or read its metadata
//! rows, stay in the root.
//!
//! The code came from the physical-adapter guard's slice, so the guard scans
//! every non-test `*.rs` file under this crate's `src/`
//! (`$GuardedCrateDirectories` in `tools/validate-physical-adapter-policy.ps1`)
//! and their literals stay in the guard's baseline. A file added here is in
//! the slice at once and fails the guard until its record is reviewed.

mod assets;
mod attributes_tail;
mod blob;
mod compatibility;
mod dcs;
mod declarations;
pub mod form_body;
mod form_extension;
mod form_layout;
mod form_ref_index;
mod form_root_42;
mod indexes;
pub mod layout_8_5_1;
pub mod record_text;
mod timing;

pub use assets::FormItemAsset;
pub use attributes_tail::{
    FormAttributesTailLayout, parse_form_attributes_conditional_appearance_tail,
    scan_form_attributes_tail_layout,
};
pub use blob::{
    FormBodyContainer, FormBodyRevision, ParsedFormBodyBlob, parse_form_body_plain,
    parse_original_form_body_plain,
};
pub use compatibility::{
    FormCompatibility, compatibility_mode_spells_any_ref, forms_mark_no_main_table_default_picture,
    forms_write_usual_group_behavior,
};
pub use dcs::{
    ANY_IB_REF_TYPE_ID, CanonicalDcsSettingsAdapterError, CanonicalDcsSettingsContext,
    CanonicalDcsSettingsInput, CanonicalFormServerStateFragment, DcsSettingsCanonicalizeError,
    DcsSettingsChild, DcsTemplateNormalizeError, FormListSettingsChildKind,
    FormListSettingsChildTransliteration, canonicalize_form_server_state_conditional_appearances,
    data_composition_type_id_xml, emit_canonical_dcs_settings_children,
    emit_canonical_dcs_settings_parts,
    normalize_data_composition_schema_template_documents_with_profiles,
    respell_form_server_state_namespaces,
    transliterate_form_attributes_conditional_appearance_document,
    transliterate_form_list_settings_child_document,
    transliterate_form_list_settings_group_items_document,
};
pub use declarations::{
    MetadataCommonAttributeContent, MetadataConstantDeclaration, MetadataFieldDeclarationIndex,
    MetadataTableStandardAttributes,
};
pub use form_extension::{
    CALL_TYPES, EventCallTypes, FormAdoption, FormBaseForm, FormEventBinding,
    adopted_base_record_slot, adoption_event_name, adoption_without_base_form,
    form_event_call_types, parse_form_event_block,
};
pub use form_layout::FormLayout;
pub use form_ref_index::FormObjectRefIndex;
pub use indexes::{
    DcsTypeIndex, DcsTypeResolution, InformationRegisterFieldReference,
    InformationRegisterFieldReferenceIndex, InformationRegisterMasterDimensionIndex,
    MetadataCommandReference,
};
pub use timing::MssqlDumpTimingReport;
