//! Canonical metadata-envelope mapping and lossless fallback support.

mod business_objects;
mod characteristics;
mod children;
mod common;
mod common_objects;
mod configuration_mobile;
mod configuration_v85_projection;
mod constant;
mod defined_type;
mod external_data_source;
mod external_objects;
mod fallback;
mod functional_option;
mod functional_options_parameter;
mod hierarchical_objects;
mod language;
mod order;
mod package;
mod register_objects;
mod registry;
mod services;
mod session_parameter;
mod utility_objects;
mod websocket_client;

pub use business_objects::{register_catalog_codec, register_document_codec};
pub use characteristics::{
    CharacteristicsXmlError, render_cct_characteristics_xml, render_characteristics_xml,
    render_metadata_characteristics_xml,
};
pub use children::{CctTemplateChildrenError, append_cct_template_children};
pub use common::{
    MetadataDecodeError, MetadataEnvelope, decode_configuration_envelope, decode_metadata_envelope,
    decode_metadata_envelope_with_dialect, decode_source_metadata_envelope,
    decode_source_metadata_envelope_with_policy,
};
pub use common_objects::{
    register_command_group_codec, register_common_command_codec, register_common_module_codec,
    register_common_picture_codec,
};
pub use configuration_mobile::parse_configuration_mobile_functionalities;
pub use configuration_v85_projection::validate_older_configuration_v85_defaults;
pub use constant::{bundled_metadata_registry, register_constant_codec};
pub use defined_type::register_defined_type_codec;
pub use external_data_source::decode_empty_external_data_source;
pub use external_objects::{
    decode_external_root, encode_external_root, register_external_data_processor_codec,
    register_external_report_codec,
};
pub use functional_option::register_functional_option_codec;
pub use functional_options_parameter::register_functional_options_parameter_codec;
pub use hierarchical_objects::{
    register_business_process_codec, register_exchange_plan_codec, register_subsystem_codec,
    register_task_codec,
};
pub use language::register_language_codec;
pub use order::{MetadataOrderError, order_metadata_features, order_produced_type_values};
pub use package::{
    ExternalSourceBinding, PackageContainedIdentity, PackageIntent, PackageRootIdentity,
    inspect_package_identity, inspect_package_intent,
};
pub use register_objects::{
    register_accounting_register_codec, register_accumulation_register_codec,
    register_calculation_register_codec, register_chart_of_accounts_codec,
    register_chart_of_calculation_types_codec, register_chart_of_characteristic_types_codec,
    register_information_register_codec, register_recalculation_codec,
};
pub use registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
pub use services::{
    register_event_subscription_codec, register_http_service_codec,
    register_integration_service_codec, register_scheduled_job_codec, register_web_service_codec,
    register_ws_reference_codec, register_xdto_package_codec,
};
pub use session_parameter::register_session_parameter_codec;
pub use utility_objects::{
    register_data_processor_codec, register_enum_codec, register_report_codec,
    register_settings_storage_codec,
};
pub use websocket_client::validate_websocket_client_headers_namespaces;
/// Emits the namespace declaration for a schema-known processor value type.
pub fn data_processor_builtin_type_namespace_attribute(reference: &str) -> Option<&'static str> {
    match ibcmd_schema::metadata_storage_facts::data_processor_builtin_type_namespace_uri(
        reference,
    )? {
        "http://v8.1c.ru/8.2/data/chart" => Some(r#" xmlns:d7p1="http://v8.1c.ru/8.2/data/chart""#),
        "http://v8.1c.ru/8.2/misc" => Some(r#" xmlns:d7p1="http://v8.1c.ru/8.2/misc""#),
        "http://v8.1c.ru/8.2/data/graphscheme" => {
            Some(r#" xmlns:d7p1="http://v8.1c.ru/8.2/data/graphscheme""#)
        }
        _ => None,
    }
}

/// A FilterCriterion with an empty type pattern keeps the Type property as
/// a self-closing element, at the standard metadata property indentation.
/// Populated patterns retain their existing typed children and qualifiers.
pub const FILTER_CRITERION_EMPTY_TYPE_XML: &str = "\t\t\t<Type/>\r\n";
