//! Compatibility re-export of the portable brace model.

pub use ibcmd_restructure::brace::*;

/// Constructs a native brace list using the shared portable tree type.
#[macro_export]
macro_rules! brace_list {
    ($($item:expr),* $(,)?) => {
        $crate::metadata_model::brace::Brace::List(vec![$($item),*])
    };
}
