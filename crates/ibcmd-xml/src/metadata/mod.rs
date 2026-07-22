//! Canonical metadata-envelope mapping and lossless fallback support.

mod business_objects;
mod common;
mod constant;
mod defined_type;
mod fallback;
mod functional_option;
mod functional_options_parameter;
mod hierarchical_objects;
mod language;
mod register_objects;
mod registry;
mod services;
mod session_parameter;
mod utility_objects;

pub use business_objects::{register_catalog_codec, register_document_codec};
pub use common::{
    MetadataDecodeError, MetadataEnvelope, decode_metadata_envelope,
    decode_metadata_envelope_with_dialect,
};
pub use constant::{bundled_metadata_registry, register_constant_codec};
pub use defined_type::register_defined_type_codec;
pub use functional_option::register_functional_option_codec;
pub use functional_options_parameter::register_functional_options_parameter_codec;
pub use hierarchical_objects::{
    register_business_process_codec, register_exchange_plan_codec, register_subsystem_codec,
    register_task_codec,
};
pub use language::register_language_codec;
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
