//! Shared resource limits for untrusted standalone-converter inputs.
//!
//! Limits live in `ibcmd-core` so every container adapter applies the same
//! bounded contract without depending on a concrete binary format.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Default maximum nesting depth accepted from a container.
pub const DEFAULT_MAX_CONTAINER_DEPTH: usize = 128;
/// Default maximum number of payload-bearing entries in one container.
pub const DEFAULT_MAX_CONTAINER_ENTRIES: usize = 1_000_000;
/// Default aggregate encoded-byte budget.
pub const DEFAULT_MAX_ENCODED_BYTES: u64 = 512 * 1_048_576;
/// Default aggregate decoded-byte budget.
pub const DEFAULT_MAX_DECODED_BYTES: u64 = 512 * 1_048_576;
/// Default maximum decoded-to-encoded ratio for compressed payloads.
pub const DEFAULT_MAX_COMPRESSION_RATIO: u64 = 200;

/// Immutable limits shared by container traversal and payload decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceLimits {
    max_depth: usize,
    max_entries: usize,
    max_encoded_bytes: u64,
    max_decoded_bytes: u64,
    max_compression_ratio: u64,
}

impl ResourceLimits {
    /// Creates a non-zero, internally consistent limit set.
    pub fn new(
        max_depth: usize,
        max_entries: usize,
        max_encoded_bytes: u64,
        max_decoded_bytes: u64,
        max_compression_ratio: u64,
    ) -> Result<Self, ResourceLimitError> {
        for (name, value) in [
            ("max_depth", max_depth as u64),
            ("max_entries", max_entries as u64),
            ("max_encoded_bytes", max_encoded_bytes),
            ("max_decoded_bytes", max_decoded_bytes),
            ("max_compression_ratio", max_compression_ratio),
        ] {
            if value == 0 {
                return Err(ResourceLimitError::InvalidZero { name });
            }
        }
        Ok(Self {
            max_depth,
            max_entries,
            max_encoded_bytes,
            max_decoded_bytes,
            max_compression_ratio,
        })
    }

    pub const fn max_depth(self) -> usize {
        self.max_depth
    }

    pub const fn max_entries(self) -> usize {
        self.max_entries
    }

    pub const fn max_encoded_bytes(self) -> u64 {
        self.max_encoded_bytes
    }

    pub const fn max_decoded_bytes(self) -> u64 {
        self.max_decoded_bytes
    }

    pub const fn max_compression_ratio(self) -> u64 {
        self.max_compression_ratio
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_CONTAINER_DEPTH,
            max_entries: DEFAULT_MAX_CONTAINER_ENTRIES,
            max_encoded_bytes: DEFAULT_MAX_ENCODED_BYTES,
            max_decoded_bytes: DEFAULT_MAX_DECODED_BYTES,
            max_compression_ratio: DEFAULT_MAX_COMPRESSION_RATIO,
        }
    }
}

/// Mutable aggregate accounting for one container traversal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceBudget {
    limits: ResourceLimits,
    depth: usize,
    entries: usize,
    encoded_bytes: u64,
    decoded_bytes: u64,
}

impl ResourceBudget {
    pub const fn new(limits: ResourceLimits) -> Self {
        Self {
            limits,
            depth: 0,
            entries: 0,
            encoded_bytes: 0,
            decoded_bytes: 0,
        }
    }

    pub const fn limits(&self) -> ResourceLimits {
        self.limits
    }

    pub const fn depth(&self) -> usize {
        self.depth
    }

    pub const fn entries(&self) -> usize {
        self.entries
    }

    pub const fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }

    pub const fn decoded_bytes(&self) -> u64 {
        self.decoded_bytes
    }

    pub const fn remaining_encoded_bytes(&self) -> u64 {
        self.limits
            .max_encoded_bytes
            .saturating_sub(self.encoded_bytes)
    }

    pub const fn remaining_decoded_bytes(&self) -> u64 {
        self.limits
            .max_decoded_bytes
            .saturating_sub(self.decoded_bytes)
    }

    /// Enters one nested container level, rejecting excessive depth first.
    pub fn enter_container(&mut self) -> Result<(), ResourceLimitError> {
        let actual = self
            .depth
            .checked_add(1)
            .ok_or(ResourceLimitError::DepthExceeded {
                maximum: self.limits.max_depth,
                actual: usize::MAX,
            })?;
        if actual > self.limits.max_depth {
            return Err(ResourceLimitError::DepthExceeded {
                maximum: self.limits.max_depth,
                actual,
            });
        }
        self.depth = actual;
        Ok(())
    }

    /// Leaves one container level and rejects an unbalanced traversal.
    pub fn leave_container(&mut self) -> Result<(), ResourceLimitError> {
        if self.depth == 0 {
            return Err(ResourceLimitError::DepthUnderflow);
        }
        self.depth -= 1;
        Ok(())
    }

    /// Atomically accounts one decoded entry and its byte totals.
    ///
    /// Failed checks do not partially consume the budget.
    pub fn record_payload(
        &mut self,
        encoded_bytes: u64,
        decoded_bytes: u64,
        compressed: bool,
    ) -> Result<(), ResourceLimitError> {
        let entries =
            self.entries
                .checked_add(1)
                .ok_or(ResourceLimitError::EntryCountExceeded {
                    maximum: self.limits.max_entries,
                    actual: usize::MAX,
                })?;
        if entries > self.limits.max_entries {
            return Err(ResourceLimitError::EntryCountExceeded {
                maximum: self.limits.max_entries,
                actual: entries,
            });
        }
        let total_encoded = self.encoded_bytes.checked_add(encoded_bytes).ok_or(
            ResourceLimitError::EncodedBytesExceeded {
                maximum: self.limits.max_encoded_bytes,
                actual: u64::MAX,
            },
        )?;
        if total_encoded > self.limits.max_encoded_bytes {
            return Err(ResourceLimitError::EncodedBytesExceeded {
                maximum: self.limits.max_encoded_bytes,
                actual: total_encoded,
            });
        }
        let total_decoded = self.decoded_bytes.checked_add(decoded_bytes).ok_or(
            ResourceLimitError::DecodedBytesExceeded {
                maximum: self.limits.max_decoded_bytes,
                actual: u64::MAX,
            },
        )?;
        if total_decoded > self.limits.max_decoded_bytes {
            return Err(ResourceLimitError::DecodedBytesExceeded {
                maximum: self.limits.max_decoded_bytes,
                actual: total_decoded,
            });
        }
        if compressed {
            ensure_compression_ratio(
                encoded_bytes,
                decoded_bytes,
                self.limits.max_compression_ratio,
            )?;
        }
        self.entries = entries;
        self.encoded_bytes = total_encoded;
        self.decoded_bytes = total_decoded;
        Ok(())
    }
}

/// Checks a decoded-to-encoded ratio without overflow.
pub fn ensure_compression_ratio(
    encoded_bytes: u64,
    decoded_bytes: u64,
    maximum: u64,
) -> Result<(), ResourceLimitError> {
    let allowed = encoded_bytes.saturating_mul(maximum);
    if decoded_bytes > allowed {
        return Err(ResourceLimitError::CompressionRatioExceeded {
            maximum,
            encoded: encoded_bytes,
            decoded: decoded_bytes,
        });
    }
    Ok(())
}

/// A deterministic resource-limit failure suitable for adapter diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceLimitError {
    InvalidZero {
        name: &'static str,
    },
    DepthExceeded {
        maximum: usize,
        actual: usize,
    },
    DepthUnderflow,
    EntryCountExceeded {
        maximum: usize,
        actual: usize,
    },
    EncodedBytesExceeded {
        maximum: u64,
        actual: u64,
    },
    DecodedBytesExceeded {
        maximum: u64,
        actual: u64,
    },
    CompressionRatioExceeded {
        maximum: u64,
        encoded: u64,
        decoded: u64,
    },
}

impl Display for ResourceLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidZero { name } => {
                write!(formatter, "resource limit `{name}` must be non-zero")
            }
            Self::DepthExceeded { maximum, actual } => write!(
                formatter,
                "container depth {actual} exceeds the configured maximum {maximum}"
            ),
            Self::DepthUnderflow => write!(formatter, "container traversal left depth zero"),
            Self::EntryCountExceeded { maximum, actual } => write!(
                formatter,
                "container entry count {actual} exceeds the configured maximum {maximum}"
            ),
            Self::EncodedBytesExceeded { maximum, actual } => write!(
                formatter,
                "encoded payload total {actual} exceeds the configured maximum {maximum} bytes"
            ),
            Self::DecodedBytesExceeded { maximum, actual } => write!(
                formatter,
                "decoded payload total {actual} exceeds the configured maximum {maximum} bytes"
            ),
            Self::CompressionRatioExceeded {
                maximum,
                encoded,
                decoded,
            } => write!(
                formatter,
                "decoded payload ratio {decoded}/{encoded} exceeds the configured maximum {maximum}:1"
            ),
        }
    }
}

impl Error for ResourceLimitError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> ResourceLimits {
        ResourceLimits::new(2, 2, 10, 20, 3).unwrap()
    }

    #[test]
    fn budget_checks_depth_and_unbalanced_leave() {
        let mut budget = ResourceBudget::new(tiny());
        budget.enter_container().unwrap();
        budget.enter_container().unwrap();
        assert!(matches!(
            budget.enter_container(),
            Err(ResourceLimitError::DepthExceeded {
                maximum: 2,
                actual: 3
            })
        ));
        budget.leave_container().unwrap();
        budget.leave_container().unwrap();
        assert_eq!(
            budget.leave_container(),
            Err(ResourceLimitError::DepthUnderflow)
        );
    }

    #[test]
    fn failed_payload_accounting_is_atomic() {
        let mut budget = ResourceBudget::new(tiny());
        budget.record_payload(2, 6, true).unwrap();
        assert!(matches!(
            budget.record_payload(2, 7, true),
            Err(ResourceLimitError::CompressionRatioExceeded { .. })
        ));
        assert_eq!(budget.entries(), 1);
        assert_eq!(budget.encoded_bytes(), 2);
        assert_eq!(budget.decoded_bytes(), 6);
    }

    #[test]
    fn zero_limits_are_rejected() {
        assert!(matches!(
            ResourceLimits::new(0, 1, 1, 1, 1),
            Err(ResourceLimitError::InvalidZero { name: "max_depth" })
        ));
    }
}
