//! Enum spellings for native metadata descriptors.

/// Lock modes accepted by the shared descriptor codec.
pub const DATA_LOCK_CONTROL_MODE: &[(&str, i64)] = &[("Automatic", 0), ("Managed", 1)];

/// Data history modes accepted by the shared descriptor codec.
pub const DATA_HISTORY: &[(&str, i64)] = &[("DontUse", 0), ("Use", 1)];
