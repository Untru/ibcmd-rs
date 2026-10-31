//! Profile-gated codecs for native bodies that are not metadata-family rows.

// The body-profile selection is `ibcmd-compiler-families`'s; the root keeps
// naming its error from this module.
pub use ibcmd_compiler_families::compiler::bodies::BodyProfileError;

/// Profile-gated base-free codec for `Ext/CommandInterface.xml` native bodies:
/// `ibcmd-compiler-families`' `compiler::bodies::command_interface`.
pub mod command_interface {
    pub use ibcmd_compiler_families::compiler::bodies::command_interface::{
        CommandInterfaceBody, CommandInterfaceCodecError, CommandInterfaceCodecProfile,
        CommandInterfaceModel, CommandOrder, CommandPlacement, CommandPlacementMode,
        CommandReference, CommandVisibility, SubsystemVisibility, VisibilityValue,
        compile_command_interface, decode_command_interface,
    };
}
/// Profile-gated codecs for data-composition template bodies:
/// `ibcmd-compiler`'s `compiler::bodies::dcs`.
pub mod dcs {
    pub use ibcmd_compiler::compiler::bodies::dcs::{
        DcsBody, DcsBodyLayout, DcsCodecError, DcsCodecProfile, DcsTemplateKind, compile_dcs,
        compile_dcs_with_references, decode_dcs, decode_dcs_with_references,
    };
    // The transitional reader of historic fixtures, for the export adapter
    // and the template audit.
    pub(crate) use ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs;
}
// The dynamic-list, chart and planner writers are `ibcmd-form-writer`'s,
// with the form writer that calls them, and the role-rights writer is
// `ibcmd-metadata-blob`'s, with its packer; `ibcmd-metadata-blob` re-exports
// the former, and they keep their paths here.
pub use ibcmd_metadata_blob::dynamic_list;
/// Profile-gated base-free codec for managed `Form.xml` native bodies:
/// `ibcmd-compiler`'s `compiler::bodies::form`.
pub mod form {
    pub use ibcmd_compiler::compiler::bodies::form::{
        ManagedFormBody, ManagedFormCodecError, ManagedFormCodecProfile, compile_managed_form,
        decode_managed_form, decode_managed_form_layout, decode_native_managed_form_module_layout,
        validate_original_managed_form_layout, verify_module_only_edit,
        verify_native_form_module_only_edit,
    };
    // The transitional reader of historic fixtures, for the export and apply
    // adapters.
    pub(crate) use ibcmd_compiler::compiler::bodies::form::decode_compatible_managed_form;
}
pub mod form_chart {
    //! The chart writers are `ibcmd-form-writer`'s (re-exported by
    //! `ibcmd-metadata-blob`). The moxel writer that read them through this
    //! module is `ibcmd-compiler`'s now and names them there; the module
    //! keeps its public path.
}
/// Writer for the managed-form item records a native 8.3.27 body stores.
///
/// The code is the `ibcmd-form-native` crate; this module keeps its
/// crate-internal path `compiler::bodies::form_native` for the root.
pub mod form_native {
    pub(crate) use ibcmd_form_native::*;
}
pub use ibcmd_metadata_blob::form_planner;
/// Base-free serializers for the interface assets beside the command
/// interfaces: `ibcmd-compiler-families`' `compiler::bodies::interface_assets`.
pub mod interface_assets {
    pub use ibcmd_compiler_families::compiler::bodies::interface_assets::{
        ClientApplicationInterfaceModel, ClientApplicationNode, ClientApplicationPanelDef,
        HomePageWorkAreaItem, HomePageWorkAreaModel, StandaloneContentExtended,
        StandaloneContentModel,
    };
}
/// Profile-gated codec for SpreadsheetDocument (MOXCEL/MXL) bodies:
/// `ibcmd-compiler`'s `compiler::bodies::mxl`.
pub mod mxl {
    pub use ibcmd_compiler::compiler::bodies::mxl::{
        MxlBody, MxlCodecError, MxlCodecProfile, compile_mxl, decode_mxl,
    };
    // The transitional readers the MOXCEL exporter decodes through.
    pub(crate) use ibcmd_compiler::compiler::bodies::mxl::{
        decode_compatible_mxl, decode_inflated_compatible_mxl,
    };
}
/// The spreadsheet template writer and its stored blob:
/// `ibcmd-compiler`'s `compiler::bodies::mxl_native`.
pub(crate) mod mxl_native {
    // `module_blob` re-exports the packers at their public paths.
    pub use ibcmd_compiler::compiler::bodies::mxl_native::{
        pack_moxel_spreadsheet_blob_from_xml, pack_moxel_spreadsheet_blob_from_xml_with_source,
        pack_moxel_spreadsheet_blob_from_xml_with_source_and_hint,
    };
    // The partial-import patches of the SQL adapter.
    #[cfg(test)]
    pub(crate) use ibcmd_compiler::compiler::bodies::mxl_native::write_native_moxel_body;
    pub(crate) use ibcmd_compiler::compiler::bodies::mxl_native::{
        inflate_template_patch, preserve_identical_template_blob, preserve_spreadsheet_language,
    };
}
/// Profile-gated codec for predefined-data bodies:
/// `ibcmd-compiler`'s `compiler::bodies::predefined`.
pub mod predefined {
    pub use ibcmd_compiler::compiler::bodies::predefined::{
        CatalogPredefinedData, CatalogPredefinedItem, CatalogPredefinedLayout, PredefinedBody,
        PredefinedCodecError, PredefinedCodecProfile, PredefinedFamily, compile_catalog_predefined,
        decode_catalog_predefined, decode_predefined, encode_predefined,
    };
}
/// Profile-gated codec for role rights bodies:
/// `ibcmd-compiler`'s `compiler::bodies::rights`, read with the root's
/// extension-export state.
pub mod rights {
    pub use ibcmd_compiler::compiler::bodies::rights::{
        RightsCodecError, RightsCodecProfile, RoleObjectReference, RoleObjectReferenceField,
        RoleObjectRightsModel, RoleRestrictionTemplateModel, RoleRightModel, RoleRightsBody,
        RoleRightsModel, RoleRightsSummary, compile_role_rights,
    };
    // The transitional reader of the export adapter, which passes it the
    // extension-export state.
    pub(crate) use ibcmd_compiler::compiler::bodies::rights::decode_compatible_role_rights;
    use ibcmd_compiler::compiler::bodies::rights::decode_role_rights_with;

    pub fn decode_role_rights(
        profile: &RightsCodecProfile,
        blob: &[u8],
    ) -> Result<RoleRightsBody, RightsCodecError> {
        decode_role_rights_with(
            profile,
            blob,
            crate::mssql_dump::extension::active().is_some(),
        )
    }
}
pub use ibcmd_metadata_blob::role_rights_writer;
/// Support-policy transfer of observed support data:
/// `ibcmd-compiler`'s `compiler::bodies::support`.
pub mod support {
    pub use ibcmd_compiler::compiler::bodies::support::{
        ObservedSupportData, SupportDataError, SupportDataKind, SupportLossReport,
        SupportPolicyProfile, SupportTransferOutcome, SupportTransferPolicy, transfer_support_data,
    };
}
/// Typed dispatch for base-free template bodies:
/// `ibcmd-compiler`'s `compiler::bodies::template`.
pub mod template {
    pub use ibcmd_compiler::compiler::bodies::template::{
        HtmlTemplateBody, TemplateBody, TemplateCodecError, TemplateCodecProfile,
        TemplateEmbeddedFile, TemplateKind, TemplateSource, compile_template,
        compile_template_with_references, decode_compatible_template, decode_template,
    };
    // The evidenced compilers of the SQL adapter and the source audit.
    pub(crate) use ibcmd_compiler::compiler::bodies::template::{
        compile_evidenced_template, compile_evidenced_template_with_resolvers,
    };
}

// The writers' and codecs' tests that read the root (its exporter, its test
// scratch).
#[cfg(test)]
mod form_chart_root_tests;
#[cfg(test)]
mod form_root_tests;
#[cfg(test)]
mod mxl_native_root_tests;
#[cfg(test)]
mod mxl_root_tests;
#[cfg(test)]
mod role_rights_writer_root_tests;

#[cfg(test)]
mod tests {
    use crate::compiler::bodies::command_interface::CommandInterfaceCodecProfile;
    use crate::compiler::bodies::dcs::DcsCodecProfile;
    use crate::compiler::bodies::form::ManagedFormCodecProfile;
    use crate::compiler::bodies::mxl::MxlCodecProfile;
    use crate::compiler::bodies::predefined::PredefinedCodecProfile;
    use crate::compiler::bodies::rights::RightsCodecProfile;
    use crate::compiler::bodies::support::SupportPolicyProfile;
    use crate::compiler::bodies::template::TemplateCodecProfile;
    use ibcmd_core::artifact::ProfileId;

    #[test]
    fn bundled_profile_selects_only_the_evidenced_body_cohort() {
        let registry = crate::profile_registry::load_bundled_profile_registry().unwrap();
        let supported = registry
            .get(&ProfileId::parse("platform-8.3.27.1989").unwrap())
            .unwrap();
        assert!(RightsCodecProfile::from_effective(supported).is_ok());
        assert!(PredefinedCodecProfile::from_effective(supported).is_ok());
        assert!(CommandInterfaceCodecProfile::from_effective(supported).is_ok());
        assert!(ManagedFormCodecProfile::from_effective(supported).is_ok());
        assert!(MxlCodecProfile::from_effective(supported).is_ok());
        assert!(DcsCodecProfile::from_effective(supported).is_ok());
        assert!(TemplateCodecProfile::from_effective(supported).is_ok());
        assert!(SupportPolicyProfile::from_effective(supported).is_ok());

        for id in ["platform-8.3.24.1819", "platform-8.5.1.1150"] {
            let profile = registry.get(&ProfileId::parse(id).unwrap()).unwrap();
            assert!(RightsCodecProfile::from_effective(profile).is_err());
            assert!(PredefinedCodecProfile::from_effective(profile).is_err());
            assert!(CommandInterfaceCodecProfile::from_effective(profile).is_err());
            assert!(ManagedFormCodecProfile::from_effective(profile).is_err());
            assert!(MxlCodecProfile::from_effective(profile).is_err());
            assert!(DcsCodecProfile::from_effective(profile).is_err());
            assert!(TemplateCodecProfile::from_effective(profile).is_err());
            assert!(SupportPolicyProfile::from_effective(profile).is_err());
        }
    }
}
