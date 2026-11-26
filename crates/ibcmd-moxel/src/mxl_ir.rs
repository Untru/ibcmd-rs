//! Bounded canonical hand-off between the MOXCEL decoder and spreadsheet XML writer.
//!
//! This module intentionally does not know XML QNames or element ordering.  The
//! decoder records only information it has actually decoded; the existing XML
//! writer consumes the resulting plan without inspecting raw MOXCEL slots.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Stable owner of an MXL diagnostic.  Consumers can distinguish a failed
/// container/IR decode from a failure to project an already-decoded IR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MxlDiagnosticStage {
    Decoder,
    Writer,
}

/// A stable, machine-readable MXL diagnostic.
///
/// Messages remain useful to humans, but callers should key automation on
/// `(stage, code)` rather than parsing the message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MxlDiagnostic {
    stage: MxlDiagnosticStage,
    code: &'static str,
    message: String,
}

impl MxlDiagnostic {
    pub(super) fn decoder(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            stage: MxlDiagnosticStage::Decoder,
            code,
            message: message.into(),
        }
    }

    pub(super) fn writer(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            stage: MxlDiagnosticStage::Writer,
            code,
            message: message.into(),
        }
    }

    pub const fn stage(&self) -> MxlDiagnosticStage {
        self.stage
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }
}

impl Display for MxlDiagnostic {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let stage = match self.stage {
            MxlDiagnosticStage::Decoder => "decoder",
            MxlDiagnosticStage::Writer => "writer",
        };
        write!(formatter, "MXL {stage} [{}]: {}", self.code, self.message)
    }
}

impl Error for MxlDiagnostic {}

/// Raw native palette slot descriptors, retained before overrides or
/// compatibility synthesis are applied to the decoded spreadsheet values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MxlPaletteProvenance {
    pub(super) raw_slots: Vec<String>,
}

/// Explicit canonical-to-XML format-slot identity carried by spreadsheet IR.
///
/// The identity variant is deliberately represented rather than encoded as an
/// absent map.  This keeps the distinction between a proven identity mapping
/// and an explicit non-one-based XML projection observable at the hand-off.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum MxlFormatReferenceMap {
    Identity {
        slots: usize,
    },
    Explicit {
        canonical_to_xml: BTreeMap<usize, usize>,
        xml_to_canonical: BTreeMap<usize, usize>,
    },
}

impl MxlFormatReferenceMap {
    pub(super) fn identity(slots: usize) -> Self {
        Self::Identity { slots }
    }

    pub(super) fn explicit(
        canonical_to_xml: BTreeMap<usize, usize>,
        xml_to_canonical: BTreeMap<usize, usize>,
    ) -> Result<Self, MxlDiagnostic> {
        if canonical_to_xml.is_empty()
            || canonical_to_xml.len() != xml_to_canonical.len()
            || canonical_to_xml
                .iter()
                .any(|(canonical, xml)| xml_to_canonical.get(xml).copied() != Some(*canonical))
        {
            return Err(MxlDiagnostic::decoder(
                "mxl.decoder.format-map-inconsistent",
                "decoded canonical/XML format references are not bijective",
            ));
        }
        Ok(Self::Explicit {
            canonical_to_xml,
            xml_to_canonical,
        })
    }
}

/// All data the XML projection is allowed to use for palette and format-index
/// emission.  It is built while decoding, before entering the XML layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MxlSpreadsheetWritePlan {
    pub(super) palette: MxlPaletteProvenance,
    pub(super) format_map: MxlFormatReferenceMap,
    pub(super) output_format_indices: Vec<usize>,
    pub(super) output_format_index_map: BTreeMap<usize, usize>,
}

impl MxlSpreadsheetWritePlan {
    pub(super) fn new(
        palette: MxlPaletteProvenance,
        format_map: MxlFormatReferenceMap,
        output_format_indices: Vec<usize>,
        output_format_index_map: BTreeMap<usize, usize>,
        format_count: usize,
    ) -> Result<Self, MxlDiagnostic> {
        let projection_complete = format_count > 0
            && output_format_indices.len() == format_count
            && output_format_index_map.len() == format_count
            && output_format_indices
                .iter()
                .enumerate()
                .all(|(offset, index)| {
                    *index > 0
                        && *index <= format_count
                        && output_format_index_map.get(index).copied() == Some(offset + 1)
                });
        let expected_xml_to_canonical = output_format_indices
            .iter()
            .enumerate()
            .map(|(offset, canonical)| (offset + 1, *canonical))
            .collect::<BTreeMap<_, _>>();
        let format_map_matches_projection = match &format_map {
            MxlFormatReferenceMap::Identity { slots } => {
                *slots == format_count
                    && output_format_indices
                        .iter()
                        .enumerate()
                        .all(|(offset, canonical)| *canonical == offset + 1)
                    && output_format_index_map
                        .iter()
                        .all(|(canonical, xml)| canonical == xml)
            }
            MxlFormatReferenceMap::Explicit {
                canonical_to_xml,
                xml_to_canonical,
            } => {
                canonical_to_xml.len() == format_count
                    && xml_to_canonical.len() == format_count
                    && canonical_to_xml.iter().all(|(canonical, xml)| {
                        *canonical > 0
                            && *canonical <= format_count
                            && *xml > 0
                            && *xml <= format_count
                            && xml_to_canonical.get(xml).copied() == Some(*canonical)
                    })
                    && canonical_to_xml == &output_format_index_map
                    && xml_to_canonical == &expected_xml_to_canonical
            }
        };
        if !projection_complete || !format_map_matches_projection {
            return Err(MxlDiagnostic::writer(
                "mxl.writer.format-plan-incomplete",
                "decoded format map and output order are not the same complete one-based XML projection",
            ));
        }
        Ok(Self {
            palette,
            format_map,
            output_format_indices,
            output_format_index_map,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> MxlPaletteProvenance {
        MxlPaletteProvenance {
            raw_slots: Vec::new(),
        }
    }

    #[test]
    fn write_plan_accepts_exact_identity_projection() {
        let plan = MxlSpreadsheetWritePlan::new(
            palette(),
            MxlFormatReferenceMap::identity(3),
            vec![1, 2, 3],
            BTreeMap::from([(1, 1), (2, 2), (3, 3)]),
            3,
        )
        .unwrap();
        assert_eq!(plan.output_format_indices, vec![1, 2, 3]);
    }

    #[test]
    fn write_plan_accepts_exact_reordered_projection() {
        let plan = MxlSpreadsheetWritePlan::new(
            palette(),
            MxlFormatReferenceMap::Explicit {
                canonical_to_xml: BTreeMap::from([(1, 2), (2, 1), (3, 3)]),
                xml_to_canonical: BTreeMap::from([(1, 2), (2, 1), (3, 3)]),
            },
            vec![2, 1, 3],
            BTreeMap::from([(1, 2), (2, 1), (3, 3)]),
            3,
        )
        .unwrap();
        assert_eq!(plan.output_format_indices, vec![2, 1, 3]);
    }

    #[test]
    fn write_plan_accepts_complete_projection_from_sparse_source_order() {
        let plan = MxlSpreadsheetWritePlan::new(
            palette(),
            MxlFormatReferenceMap::Explicit {
                canonical_to_xml: BTreeMap::from([(1, 3), (2, 1), (3, 4), (4, 2)]),
                xml_to_canonical: BTreeMap::from([(1, 2), (2, 4), (3, 1), (4, 3)]),
            },
            vec![2, 4, 1, 3],
            BTreeMap::from([(1, 3), (2, 1), (3, 4), (4, 2)]),
            4,
        )
        .unwrap();
        assert_eq!(plan.output_format_indices, vec![2, 4, 1, 3]);
    }

    fn assert_invalid_map(format_map: MxlFormatReferenceMap) {
        let error = MxlSpreadsheetWritePlan::new(
            palette(),
            format_map,
            vec![2, 1],
            BTreeMap::from([(1, 2), (2, 1)]),
            2,
        )
        .unwrap_err();
        assert_eq!(error.stage(), MxlDiagnosticStage::Writer);
        assert_eq!(error.code(), "mxl.writer.format-plan-incomplete");
    }

    #[test]
    fn write_plan_rejects_identity_slot_count_mismatch() {
        let error = MxlSpreadsheetWritePlan::new(
            palette(),
            MxlFormatReferenceMap::identity(3),
            vec![1, 2],
            BTreeMap::from([(1, 1), (2, 2)]),
            2,
        )
        .unwrap_err();
        assert_eq!(error.stage(), MxlDiagnosticStage::Writer);
        assert_eq!(error.code(), "mxl.writer.format-plan-incomplete");
    }

    #[test]
    fn write_plan_rejects_identity_map_for_reordered_projection() {
        assert_invalid_map(MxlFormatReferenceMap::identity(2));
    }

    #[test]
    fn write_plan_rejects_zero_format_map_entries() {
        assert_invalid_map(MxlFormatReferenceMap::Explicit {
            canonical_to_xml: BTreeMap::from([(0, 1), (2, 2)]),
            xml_to_canonical: BTreeMap::from([(1, 0), (2, 2)]),
        });
    }

    #[test]
    fn write_plan_rejects_out_of_range_format_map_entries() {
        assert_invalid_map(MxlFormatReferenceMap::Explicit {
            canonical_to_xml: BTreeMap::from([(1, 3), (2, 1)]),
            xml_to_canonical: BTreeMap::from([(1, 2), (3, 1)]),
        });
    }

    #[test]
    fn write_plan_rejects_map_that_disagrees_with_output_projection() {
        assert_invalid_map(MxlFormatReferenceMap::Explicit {
            canonical_to_xml: BTreeMap::from([(1, 1), (2, 2)]),
            xml_to_canonical: BTreeMap::from([(1, 1), (2, 2)]),
        });
    }
}
