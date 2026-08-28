//! Shared resource limits for untrusted standalone-converter inputs.
//!
//! Limits live in `ibcmd-core` so every container adapter applies the same
//! bounded contract without depending on a concrete binary format.
//!
//! # Two different things are bounded here, and they are not interchangeable
//!
//! * **Hostile-input defences** are shape bounds that no honest producer ever
//!   approaches: [`DEFAULT_MAX_COMPRESSION_RATIO`] (per payload), the per-entry
//!   payload cap, the entry count, and the nesting depth. They are fixed
//!   constants on purpose — a decompression bomb is defined by its *shape*, not
//!   by how large the file the user picked happens to be. Nothing in this
//!   module relaxes them.
//! * **Memory budgets** are the aggregate byte ceilings. A fixed constant is
//!   the wrong lever for them in an offline CLI: the tool reads one local file
//!   that the user named, and the exact physical ceiling on how many encoded
//!   bytes that traversal can ever account for is the length of that file,
//!   known at open time. A magic constant is simultaneously too small for a
//!   real production configuration (a 953 MiB `1cv8.cf` cannot be opened at
//!   all) and needlessly generous for a 4 KiB one.
//!   [`ResourceLimits::for_input_bytes`] therefore derives the aggregate
//!   budgets from the input, and the `DEFAULT_MAX_*_BYTES` constants below
//!   become the *floor* rather than the ceiling.
//!
//! Deriving the budgets makes the aggregate expansion check stronger, not
//! weaker: above the floor a container may expand at most
//! [`MAX_AGGREGATE_EXPANSION`]-fold in total, which is 50x tighter than the
//! per-payload ratio a bomb would have to satisfy anyway.
//!
//! # Known limitation: everything is held in RAM
//!
//! Decoding a container materialises every packed and unpacked payload in one
//! `StorageImage`, so the retained budget is real resident memory, not a
//! notional accounting ceiling. Measured on the 953 MiB (952 706 664 byte)
//! `1cv8.cf` reference, `cf export` peaks at 6.2–6.5 GiB resident across runs
//! and finishes in about 2.5 minutes — roughly 7x the input on disk once the
//! export writer's own buffers and allocator overhead are counted on top of the
//! payloads.
//!
//! That ratio, not any constant here, is the real ceiling: a 3 GiB ERP
//! configuration would want something like 21 GiB resident and would die on a
//! 16 GiB host no matter how large these budgets are set. The architectural
//! answer is streaming traversal that never holds a whole container at once,
//! which is deliberately out of scope for this module; naming the wall is not.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Default maximum nesting depth accepted from a container.
///
/// Hostile-input defence: fixed, never derived from the input.
pub const DEFAULT_MAX_CONTAINER_DEPTH: usize = 128;
/// Default maximum number of payload-bearing entries in one container.
///
/// Hostile-input defence: fixed, never derived from the input.
pub const DEFAULT_MAX_CONTAINER_ENTRIES: usize = 1_000_000;
/// Floor for the aggregate encoded-byte budget.
///
/// Above this floor the budget is the input's own length, which is the exact
/// physical bound: every encoded byte accounted during a traversal is read out
/// of the file the user named, and payload extents inside a container do not
/// overlap.
pub const DEFAULT_MAX_ENCODED_BYTES: u64 = 512 * 1_048_576;
/// Floor for the aggregate decoded-byte budget.
pub const DEFAULT_MAX_DECODED_BYTES: u64 = 512 * 1_048_576;
/// Floor for the aggregate heap-retained budget of one decoded image.
pub const DEFAULT_MAX_RETAINED_BYTES: u64 = 512 * 1_048_576;
/// Default maximum decoded-to-encoded ratio for compressed payloads.
///
/// Hostile-input defence: fixed, never derived from the input. This is the
/// check that actually rejects a decompression bomb, and it is applied to every
/// individual compressed payload.
pub const DEFAULT_MAX_COMPRESSION_RATIO: u64 = 200;
/// Aggregate decoded-to-encoded expansion allowed across one whole input.
///
/// A real configuration expands modestly in aggregate even though individual
/// payloads compress well: the 953 MiB `1cv8.cf` reference decodes to
/// 2 234 465 727 bytes, about 2.35 times its own on-disk size. Allowing 4:1
/// keeps a genuine, *input-proportional* aggregate ceiling — a 1 MiB hostile
/// file still cannot decode past the floor, and a large one cannot decode past
/// four times its own size — while remaining far below the 200:1 a per-payload
/// bomb must clear.
pub const MAX_AGGREGATE_EXPANSION: u64 = 4;

/// Immutable limits shared by container traversal and payload decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceLimits {
    max_depth: usize,
    max_entries: usize,
    max_encoded_bytes: u64,
    max_decoded_bytes: u64,
    max_compression_ratio: u64,
    max_retained_bytes: u64,
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
            max_retained_bytes: DEFAULT_MAX_RETAINED_BYTES,
        })
    }

    /// Derives the aggregate memory budgets from the length of the input the
    /// user explicitly named.
    ///
    /// The shape defences (depth, entry count, per-payload compression ratio)
    /// keep their fixed defaults; only the aggregate byte budgets scale, and
    /// they never drop below the historical floors so that small inputs behave
    /// exactly as before.
    ///
    /// * encoded — the input length itself. Payload extents inside a container
    ///   are disjoint, so no honest traversal can account for more encoded
    ///   bytes than the file holds; this is a physical bound, not a guess.
    /// * decoded — [`MAX_AGGREGATE_EXPANSION`] times the input length, so the
    ///   aggregate expansion ceiling stays proportional to the chosen input
    ///   instead of being a constant that is wrong at both ends of the range.
    ///   Because the floor also applies, no input decodes less than it used to.
    /// * retained — the encoded plus the decoded bound. A decoded image retains
    ///   the packed *and* the unpacked bytes of every entry, so it cannot
    ///   legitimately retain more than the traversal was allowed to read plus
    ///   what it was allowed to decode. Deriving it removes the third
    ///   independent magic number rather than replacing it with a larger one.
    ///   This budget is resident memory: see the module-level note.
    #[must_use]
    pub const fn for_input_bytes(input_len: u64) -> Self {
        let max_encoded_bytes = if input_len > DEFAULT_MAX_ENCODED_BYTES {
            input_len
        } else {
            DEFAULT_MAX_ENCODED_BYTES
        };
        let scaled_decoded = input_len.saturating_mul(MAX_AGGREGATE_EXPANSION);
        let max_decoded_bytes = if scaled_decoded > DEFAULT_MAX_DECODED_BYTES {
            scaled_decoded
        } else {
            DEFAULT_MAX_DECODED_BYTES
        };
        let scaled_retained = input_len.saturating_add(scaled_decoded);
        let max_retained_bytes = if scaled_retained > DEFAULT_MAX_RETAINED_BYTES {
            scaled_retained
        } else {
            DEFAULT_MAX_RETAINED_BYTES
        };
        Self {
            max_depth: DEFAULT_MAX_CONTAINER_DEPTH,
            max_entries: DEFAULT_MAX_CONTAINER_ENTRIES,
            max_encoded_bytes,
            max_decoded_bytes,
            max_compression_ratio: DEFAULT_MAX_COMPRESSION_RATIO,
            max_retained_bytes,
        }
    }

    /// Replaces the heap-retention budget, rejecting a zero ceiling.
    pub fn with_max_retained_bytes(
        self,
        max_retained_bytes: u64,
    ) -> Result<Self, ResourceLimitError> {
        if max_retained_bytes == 0 {
            return Err(ResourceLimitError::InvalidZero {
                name: "max_retained_bytes",
            });
        }
        Ok(Self {
            max_retained_bytes,
            ..self
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

    /// Returns the aggregate heap-retention budget for one decoded image.
    ///
    /// This budget is resident memory, not accounting: see the module-level
    /// note on the streaming limitation.
    pub const fn max_retained_bytes(self) -> u64 {
        self.max_retained_bytes
    }

    /// Returns the retention budget clamped into the platform `usize` used by
    /// the neutral storage model.
    pub fn max_retained_bytes_usize(self) -> usize {
        usize::try_from(self.max_retained_bytes).unwrap_or(usize::MAX)
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
            max_retained_bytes: DEFAULT_MAX_RETAINED_BYTES,
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
        assert!(matches!(
            ResourceLimits::default().with_max_retained_bytes(0),
            Err(ResourceLimitError::InvalidZero {
                name: "max_retained_bytes"
            })
        ));
    }

    #[test]
    fn input_derived_budgets_scale_without_touching_shape_defences() {
        let small = ResourceLimits::for_input_bytes(4_096);
        assert_eq!(small.max_encoded_bytes(), DEFAULT_MAX_ENCODED_BYTES);
        assert_eq!(small.max_decoded_bytes(), DEFAULT_MAX_DECODED_BYTES);
        assert_eq!(small.max_retained_bytes(), DEFAULT_MAX_RETAINED_BYTES);

        // The 953 MiB production reference configuration.
        let production = ResourceLimits::for_input_bytes(999_292_928);
        assert_eq!(production.max_encoded_bytes(), 999_292_928);
        assert_eq!(
            production.max_decoded_bytes(),
            999_292_928 * MAX_AGGREGATE_EXPANSION
        );
        assert_eq!(
            production.max_retained_bytes(),
            999_292_928 + 999_292_928 * MAX_AGGREGATE_EXPANSION
        );

        // Deriving never lowers a budget below the historical fixed default,
        // so no input that used to open stops opening.
        for input_len in [0, 1, 4_096, 1_048_576, 536_870_912, 999_292_928, u64::MAX] {
            let limits = ResourceLimits::for_input_bytes(input_len);
            assert!(limits.max_encoded_bytes() >= DEFAULT_MAX_ENCODED_BYTES);
            assert!(limits.max_decoded_bytes() >= DEFAULT_MAX_DECODED_BYTES);
            assert!(limits.max_retained_bytes() >= DEFAULT_MAX_RETAINED_BYTES);
        }

        // Shape defences are identical at every input scale.
        for limits in [small, production, ResourceLimits::default()] {
            assert_eq!(limits.max_depth(), DEFAULT_MAX_CONTAINER_DEPTH);
            assert_eq!(limits.max_entries(), DEFAULT_MAX_CONTAINER_ENTRIES);
            assert_eq!(
                limits.max_compression_ratio(),
                DEFAULT_MAX_COMPRESSION_RATIO
            );
        }
    }

    #[test]
    fn input_derived_budgets_still_reject_a_decompression_bomb() {
        // A payload whose expansion exceeds the per-payload ratio is rejected
        // no matter how large the input the budgets were derived from: the
        // ratio defence is not part of the memory budget.
        let huge = ResourceLimits::for_input_bytes(u64::MAX);
        let mut budget = ResourceBudget::new(huge);
        let encoded = 1_024;
        let bomb = encoded * (DEFAULT_MAX_COMPRESSION_RATIO + 1);
        assert!(matches!(
            budget.record_payload(encoded, bomb, true),
            Err(ResourceLimitError::CompressionRatioExceeded {
                maximum: DEFAULT_MAX_COMPRESSION_RATIO,
                ..
            })
        ));
        assert_eq!(budget.entries(), 0);

        // The same bytes at the historical fixed defaults are rejected too, so
        // deriving budgets from the input did not move this boundary.
        let mut fixed = ResourceBudget::new(ResourceLimits::default());
        assert!(matches!(
            fixed.record_payload(encoded, bomb, true),
            Err(ResourceLimitError::CompressionRatioExceeded { .. })
        ));

        // And the aggregate expansion ceiling stays proportional: a small
        // hostile input cannot decode past the floor by chaining many payloads
        // that individually stay under the ratio.
        let derived = ResourceLimits::for_input_bytes(1_048_576);
        assert_eq!(derived.max_decoded_bytes(), DEFAULT_MAX_DECODED_BYTES);
        let mut aggregate = ResourceBudget::new(derived);
        let chunk_encoded = 1_048_576;
        let chunk_decoded = chunk_encoded * 100;
        let mut rejected = false;
        for _ in 0..16 {
            if aggregate
                .record_payload(chunk_encoded, chunk_decoded, true)
                .is_err()
            {
                rejected = true;
                break;
            }
        }
        assert!(rejected, "aggregate decoded budget must still fail closed");
    }
}
