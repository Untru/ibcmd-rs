//! Explicit resource accounting for an owned source conversion operation.
//!
//! Ordinary constructors and untrusted deserialization retain bounded defaults.
//! Source adapters may opt into checked accounting without fixed graph-size
//! ceilings. This choice never relaxes identity, syntax, ordering, ownership or
//! provenance invariants, and does not preallocate space from declared counts.

/// An operation's resource policy, supplied explicitly by its source adapter.
///
/// The policy is neither serialized with data nor inferred from input fields.
/// Individual source decoders must explicitly propagate it through construction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SourceOperationPolicy {
    /// Existing bounded construction and untrusted deserialization.
    #[default]
    Bounded,
    /// Checked source accounting; actual allocation and IO remain fallible.
    Source,
}

impl SourceOperationPolicy {
    /// Selects the existing bounded contract.
    pub const fn bounded_default() -> Self {
        Self::Bounded
    }

    /// Selects a complete source operation without fixed graph-size ceilings.
    pub const fn source_operation() -> Self {
        Self::Source
    }

    /// Returns a real optional budget, never a sentinel source ceiling.
    pub const fn budget(self, bounded: usize) -> Option<usize> {
        match self {
            Self::Bounded => Some(bounded),
            Self::Source => None,
        }
    }
}
