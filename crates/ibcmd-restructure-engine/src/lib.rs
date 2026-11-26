//! Native restructuring plans and execution, independent of CLI adapters.

pub mod restructure;
pub mod si;
pub mod text;
pub use ibcmd_restructure::brace_list;
pub use ibcmd_sql::sql;

mod metadata_model {
    pub use ibcmd_restructure::brace;
    pub mod objects {
        pub use ibcmd_restructure::descriptor::layout;
        pub mod parts {
            pub use ibcmd_restructure::descriptor::Slot;
        }
    }
}
mod mssql_config_apply {
    pub use crate::si;
    pub mod model {
        pub use crate::text::{hex_upper, is_uuid_text, quote_ident};
    }
}
mod compiler {
    pub use ibcmd_compiler_families::compiler::families;
}
mod module_blob {
    pub use ibcmd_1c_text::{decode_base64_mime_lenient as decode_base64_mime, encode_base64};
}
