//! Cooperative cancellation of a long operation. The flag, process-wide, and
//! its one owner are the `ibcmd-cancel` crate; this module keeps the root
//! paths.

pub use ibcmd_cancel::{CancelScope, Cancelled, check, is_cancelled, scope};
