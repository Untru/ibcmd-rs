//! Bounded canonical data-composition settings.
//!
//! This module deliberately models only the verified minimum shared by a
//! standalone settings document and Form `ListSettings`. XML names, type IDs,
//! and writer order do not belong to this IR. A decoder must retain every
//! unsupported extension as an exact [`OpaqueFacet`] instead of guessing a
//! typed interpretation.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::opaque::{OpaqueFacet, OpaqueFacets};
use crate::provenance::SourceProvenance;
use crate::value::{CanonicalText, EnumToken};

/// Maximum opaque XML extensions retained by one DCS settings value.
pub const MAX_DCS_OPAQUE_EXTENSIONS: usize = 4_096;
/// Maximum aggregate variable-sized data retained by one DCS settings value.
pub const MAX_DCS_RETAINED_BYTES: usize = 67_108_864;

/// Failure to construct or revalidate bounded canonical DCS settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DcsBuildError {
    /// The opaque extension collection exceeded its DCS-specific item bound.
    TooManyOpaqueExtensions {
        /// Maximum accepted extensions.
        maximum: usize,
        /// Actual extensions.
        actual: usize,
    },
    /// An opaque extension was not anchored to the source profile of settings.
    OpaqueSourceProfileMismatch {
        /// Zero-based extension index.
        index: usize,
    },
    /// An opaque extension did not declare an XML placement kind.
    NonXmlPlacement {
        /// Zero-based extension index.
        index: usize,
        /// Exact rejected placement token.
        placement: String,
    },
    /// An opaque extension did not declare a canonical XML media kind.
    NonXmlMediaKind {
        /// Zero-based extension index.
        index: usize,
        /// Exact rejected media-kind token.
        media_kind: String,
    },
    /// Two opaque extensions claimed the same anchor and placement.
    DuplicateOpaquePlacement {
        /// Index of the second extension.
        index: usize,
    },
    /// Aggregate variable-sized data exceeded the DCS budget.
    RetainedBytesExceeded {
        /// Maximum accepted retained bytes.
        maximum: usize,
        /// Actual retained bytes.
        actual: usize,
    },
    /// Aggregate retained-byte arithmetic overflowed.
    RetainedByteCountOverflow,
}

impl Display for DcsBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyOpaqueExtensions { maximum, actual } => write!(
                formatter,
                "DCS settings exceed {maximum} opaque extensions (actual {actual})"
            ),
            Self::OpaqueSourceProfileMismatch { index } => write!(
                formatter,
                "DCS opaque extension {index} belongs to a different source profile"
            ),
            Self::NonXmlPlacement { index, placement } => write!(
                formatter,
                "DCS opaque extension {index} has non-XML placement `{placement}`"
            ),
            Self::NonXmlMediaKind { index, media_kind } => write!(
                formatter,
                "DCS opaque extension {index} has non-XML media kind `{media_kind}`"
            ),
            Self::DuplicateOpaquePlacement { index } => write!(
                formatter,
                "DCS opaque extension {index} duplicates an anchor and placement"
            ),
            Self::RetainedBytesExceeded { maximum, actual } => write!(
                formatter,
                "DCS settings exceed retained-byte budget {maximum} (actual {actual})"
            ),
            Self::RetainedByteCountOverflow => {
                formatter.write_str("DCS retained-byte count overflowed")
            }
        }
    }
}

impl Error for DcsBuildError {}

/// Verified typed minimum of `DataCompositionSettings`.
///
/// The two optional fields are structural model semantics. Their XML spelling,
/// omission rules, and order remain the responsibility of verified writer
/// rules. Unknown settings children and attributes are retained in exact source
/// order by `opaque_extensions`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSettings {
    items_user_setting_id: Option<CanonicalText>,
    items_view_mode: Option<EnumToken>,
    opaque_extensions: OpaqueFacets,
    provenance: SourceProvenance,
}

impl DcsSettings {
    /// Builds settings and validates all DCS-specific resource and provenance
    /// invariants.
    pub fn new(
        items_user_setting_id: Option<CanonicalText>,
        items_view_mode: Option<EnumToken>,
        opaque_extensions: OpaqueFacets,
        provenance: SourceProvenance,
    ) -> Result<Self, DcsBuildError> {
        validate_settings(
            items_user_setting_id.as_ref(),
            items_view_mode.as_ref(),
            &opaque_extensions,
            &provenance,
        )?;
        Ok(Self {
            items_user_setting_id,
            items_view_mode,
            opaque_extensions,
            provenance,
        })
    }

    /// Returns the optional exact user-setting identifier.
    pub const fn items_user_setting_id(&self) -> Option<&CanonicalText> {
        self.items_user_setting_id.as_ref()
    }

    /// Returns the optional open settings item view-mode token.
    pub const fn items_view_mode(&self) -> Option<&EnumToken> {
        self.items_view_mode.as_ref()
    }

    /// Returns unsupported extensions in exact source order.
    pub const fn opaque_extensions(&self) -> &OpaqueFacets {
        &self.opaque_extensions
    }

    /// Returns exact source provenance for the typed settings value.
    pub const fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSettingsWire {
    items_user_setting_id: Option<CanonicalText>,
    items_view_mode: Option<EnumToken>,
    opaque_extensions: DcsOpaqueFacetsWire,
    provenance: SourceProvenance,
}

struct DcsOpaqueFacetsWire(Vec<OpaqueFacet>);

struct DcsOpaqueFacetsVisitor;

impl<'de> Visitor<'de> for DcsOpaqueFacetsVisitor {
    type Value = DcsOpaqueFacetsWire;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "at most {MAX_DCS_OPAQUE_EXTENSIONS} DCS XML extensions retaining at most {MAX_DCS_RETAINED_BYTES} bytes"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut facets = Vec::with_capacity(
            sequence
                .size_hint()
                .unwrap_or_default()
                .min(MAX_DCS_OPAQUE_EXTENSIONS),
        );
        let mut retained_bytes = 0usize;
        while facets.len() < MAX_DCS_OPAQUE_EXTENSIONS {
            let Some(facet) = sequence.next_element::<OpaqueFacet>()? else {
                return Ok(DcsOpaqueFacetsWire(facets));
            };
            let byte_len = usize::try_from(facet.byte_len())
                .map_err(|_| de::Error::custom(DcsBuildError::RetainedByteCountOverflow))?;
            retained_bytes =
                checked_retained_bytes(retained_bytes, byte_len).map_err(de::Error::custom)?;
            retained_bytes =
                checked_retained_bytes(retained_bytes, facet.provenance().retained_byte_len())
                    .map_err(de::Error::custom)?;
            retained_bytes =
                checked_retained_bytes(retained_bytes, facet.placement().kind().as_str().len())
                    .map_err(de::Error::custom)?;
            retained_bytes =
                checked_retained_bytes(retained_bytes, facet.media_kind().as_str().len())
                    .map_err(de::Error::custom)?;
            facets.push(facet);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "DCS settings exceed {MAX_DCS_OPAQUE_EXTENSIONS} opaque extensions"
            )));
        }
        Ok(DcsOpaqueFacetsWire(facets))
    }
}

impl<'de> Deserialize<'de> for DcsOpaqueFacetsWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(DcsOpaqueFacetsVisitor)
    }
}

impl<'de> Deserialize<'de> for DcsSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSettingsWire::deserialize(deserializer)?;
        Self::new(
            wire.items_user_setting_id,
            wire.items_view_mode,
            OpaqueFacets::new(wire.opaque_extensions.0).map_err(de::Error::custom)?,
            wire.provenance,
        )
        .map_err(de::Error::custom)
    }
}

/// Physical context in which the same canonical settings semantics occurred.
///
/// This distinction preserves the verified delegation boundary without
/// duplicating the settings model or introducing XML wrapper names into core.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "context",
    content = "settings",
    rename_all = "snake_case"
)]
pub enum DcsSettingsEnvelope {
    /// A standalone DCS settings document.
    Settings(DcsSettings),
    /// Settings delegated by a Form `ListSettings` feature.
    ListSettings(DcsSettings),
}

impl DcsSettingsEnvelope {
    /// Creates a standalone settings envelope.
    pub const fn settings(settings: DcsSettings) -> Self {
        Self::Settings(settings)
    }

    /// Creates a Form `ListSettings` envelope.
    pub const fn list_settings(settings: DcsSettings) -> Self {
        Self::ListSettings(settings)
    }

    /// Returns the shared canonical settings payload.
    pub const fn as_settings(&self) -> &DcsSettings {
        match self {
            Self::Settings(settings) | Self::ListSettings(settings) => settings,
        }
    }
}

fn validate_settings(
    items_user_setting_id: Option<&CanonicalText>,
    items_view_mode: Option<&EnumToken>,
    opaque_extensions: &OpaqueFacets,
    provenance: &SourceProvenance,
) -> Result<(), DcsBuildError> {
    if opaque_extensions.len() > MAX_DCS_OPAQUE_EXTENSIONS {
        return Err(DcsBuildError::TooManyOpaqueExtensions {
            maximum: MAX_DCS_OPAQUE_EXTENSIONS,
            actual: opaque_extensions.len(),
        });
    }

    let mut placements = BTreeSet::new();
    let mut retained_bytes = provenance.retained_byte_len();
    if let Some(value) = items_user_setting_id {
        retained_bytes = checked_retained_bytes(retained_bytes, value.as_str().len())?;
    }
    if let Some(value) = items_view_mode {
        retained_bytes = checked_retained_bytes(retained_bytes, value.as_str().len())?;
    }

    for (index, facet) in opaque_extensions.as_slice().iter().enumerate() {
        validate_opaque_extension(index, facet, provenance, &mut placements)?;
        let byte_len = usize::try_from(facet.byte_len())
            .map_err(|_| DcsBuildError::RetainedByteCountOverflow)?;
        retained_bytes = checked_retained_bytes(retained_bytes, byte_len)?;
        retained_bytes =
            checked_retained_bytes(retained_bytes, facet.provenance().retained_byte_len())?;
        retained_bytes =
            checked_retained_bytes(retained_bytes, facet.placement().kind().as_str().len())?;
        retained_bytes = checked_retained_bytes(retained_bytes, facet.media_kind().as_str().len())?;
    }
    Ok(())
}

fn validate_opaque_extension<'a>(
    index: usize,
    facet: &'a OpaqueFacet,
    provenance: &SourceProvenance,
    placements: &mut BTreeSet<(
        &'a crate::provenance::CanonicalAnchor,
        &'a crate::opaque::OpaquePlacement,
    )>,
) -> Result<(), DcsBuildError> {
    if facet.source_profile() != provenance.source_profile() {
        return Err(DcsBuildError::OpaqueSourceProfileMismatch { index });
    }
    let placement = facet.placement().kind().as_str();
    if !placement.starts_with("xml:") {
        return Err(DcsBuildError::NonXmlPlacement {
            index,
            placement: placement.to_owned(),
        });
    }
    let media_kind = facet.media_kind().as_str();
    if !matches!(media_kind, "application/xml" | "text/xml")
        && !media_kind
            .strip_prefix("application/")
            .is_some_and(|subtype| subtype.ends_with("+xml"))
    {
        return Err(DcsBuildError::NonXmlMediaKind {
            index,
            media_kind: media_kind.to_owned(),
        });
    }
    if !placements.insert((facet.anchor(), facet.placement())) {
        return Err(DcsBuildError::DuplicateOpaquePlacement { index });
    }
    Ok(())
}

fn checked_retained_bytes(current: usize, additional: usize) -> Result<usize, DcsBuildError> {
    let actual = current
        .checked_add(additional)
        .ok_or(DcsBuildError::RetainedByteCountOverflow)?;
    if actual > MAX_DCS_RETAINED_BYTES {
        return Err(DcsBuildError::RetainedBytesExceeded {
            maximum: MAX_DCS_RETAINED_BYTES,
            actual,
        });
    }
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use crate::artifact::ProfileId;
    use crate::asset::MediaKind;
    use crate::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use crate::opaque::OpaquePlacement;
    use crate::provenance::CanonicalAnchor;

    use super::*;

    fn anchor(property: &str) -> CanonicalAnchor {
        CanonicalAnchor::new(
            ObjectPath::new(vec![PathSegment::name("dcs_settings").unwrap()]).unwrap(),
            PropertyPath::new(vec![PathSegment::name(property).unwrap()]).unwrap(),
        )
    }

    fn provenance(profile: &str, property: &str) -> SourceProvenance {
        SourceProvenance::with_locator(
            ProfileId::parse(profile).unwrap(),
            anchor(property),
            "fixture:dcs/settings.xml",
        )
        .unwrap()
    }

    fn extension(profile: &str, ordinal: u32, bytes: &[u8]) -> OpaqueFacet {
        OpaqueFacet::new(
            provenance(profile, "extensions"),
            OpaquePlacement::new("xml:child", ordinal).unwrap(),
            bytes.to_vec(),
            MediaKind::new("application/xml").unwrap(),
        )
        .unwrap()
    }

    fn settings(extensions: Vec<OpaqueFacet>) -> DcsSettings {
        DcsSettings::new(
            Some(CanonicalText::new("main-settings").unwrap()),
            Some(EnumToken::new("QuickAccess").unwrap()),
            OpaqueFacets::new(extensions).unwrap(),
            provenance("platform:8.3.24", "settings"),
        )
        .unwrap()
    }

    #[test]
    fn standalone_and_list_settings_share_one_deterministic_payload() {
        let payload = settings(vec![extension(
            "platform:8.3.24",
            2,
            b"<future xmlns=\"urn:future\" exact=\"yes\"/>",
        )]);
        let standalone = DcsSettingsEnvelope::settings(payload.clone());
        let list_settings = DcsSettingsEnvelope::list_settings(payload);

        let standalone_json = serde_json::to_string(&standalone).unwrap();
        let list_json = serde_json::to_string(&list_settings).unwrap();
        assert!(standalone_json.starts_with(r#"{"context":"settings","settings":"#));
        assert!(list_json.starts_with(r#"{"context":"list_settings","settings":"#));
        assert_eq!(
            serde_json::from_str::<DcsSettingsEnvelope>(&standalone_json).unwrap(),
            standalone
        );
        assert_eq!(
            serde_json::from_str::<DcsSettingsEnvelope>(&list_json).unwrap(),
            list_settings
        );
        assert_eq!(
            serde_json::to_string(
                serde_json::from_str::<DcsSettingsEnvelope>(&list_json)
                    .unwrap()
                    .as_settings()
            )
            .unwrap(),
            serde_json::to_string(list_settings.as_settings()).unwrap()
        );
    }

    #[test]
    fn opaque_extension_retains_exact_bytes_placement_and_provenance() {
        let expected = b"<x:Future xmlns:x=\"urn:x\">\n  exact text\n</x:Future>";
        let value = settings(vec![extension("platform:8.3.24", 7, expected)]);
        let facet = &value.opaque_extensions().as_slice()[0];

        assert_eq!(facet.placement().kind().as_str(), "xml:child");
        assert_eq!(facet.placement().ordinal(), 7);
        assert_eq!(
            facet.provenance().locator().unwrap().as_str(),
            "fixture:dcs/settings.xml"
        );
        let profile = ProfileId::parse("platform:8.3.24").unwrap();
        assert_eq!(facet.emit_permit(&profile).unwrap().bytes(), expected);
    }

    #[test]
    fn mismatched_profile_and_non_xml_facets_fail_closed() {
        let mismatched = extension("platform:8.3.25", 0, b"<future/>");
        assert!(matches!(
            DcsSettings::new(
                None,
                None,
                OpaqueFacets::new(vec![mismatched]).unwrap(),
                provenance("platform:8.3.24", "settings"),
            ),
            Err(DcsBuildError::OpaqueSourceProfileMismatch { index: 0 })
        ));

        let non_xml_placement = OpaqueFacet::new(
            provenance("platform:8.3.24", "extensions"),
            OpaquePlacement::new("binary:tail", 0).unwrap(),
            b"<future/>".to_vec(),
            MediaKind::new("application/xml").unwrap(),
        )
        .unwrap();
        assert!(matches!(
            DcsSettings::new(
                None,
                None,
                OpaqueFacets::new(vec![non_xml_placement]).unwrap(),
                provenance("platform:8.3.24", "settings"),
            ),
            Err(DcsBuildError::NonXmlPlacement { index: 0, .. })
        ));

        let non_xml_media = OpaqueFacet::new(
            provenance("platform:8.3.24", "extensions"),
            OpaquePlacement::new("xml:child", 0).unwrap(),
            b"<future/>".to_vec(),
            MediaKind::new("application/octet-stream").unwrap(),
        )
        .unwrap();
        assert!(matches!(
            DcsSettings::new(
                None,
                None,
                OpaqueFacets::new(vec![non_xml_media]).unwrap(),
                provenance("platform:8.3.24", "settings"),
            ),
            Err(DcsBuildError::NonXmlMediaKind { index: 0, .. })
        ));
    }

    #[test]
    fn duplicate_placement_and_excessive_extension_count_are_rejected() {
        let first = extension("platform:8.3.24", 1, b"<first/>");
        let duplicate = extension("platform:8.3.24", 1, b"<second/>");
        assert!(matches!(
            DcsSettings::new(
                None,
                None,
                OpaqueFacets::new(vec![first, duplicate]).unwrap(),
                provenance("platform:8.3.24", "settings"),
            ),
            Err(DcsBuildError::DuplicateOpaquePlacement { index: 1 })
        ));

        let extensions = (0..=MAX_DCS_OPAQUE_EXTENSIONS)
            .map(|ordinal| extension("platform:8.3.24", u32::try_from(ordinal).unwrap(), b""))
            .collect();
        assert!(matches!(
            DcsSettings::new(
                None,
                None,
                OpaqueFacets::new(extensions).unwrap(),
                provenance("platform:8.3.24", "settings"),
            ),
            Err(DcsBuildError::TooManyOpaqueExtensions { .. })
        ));
    }

    #[test]
    fn retained_byte_budget_rejects_limit_plus_one_and_overflow() {
        assert_eq!(
            checked_retained_bytes(MAX_DCS_RETAINED_BYTES, 1),
            Err(DcsBuildError::RetainedBytesExceeded {
                maximum: MAX_DCS_RETAINED_BYTES,
                actual: MAX_DCS_RETAINED_BYTES + 1,
            })
        );
        assert_eq!(
            checked_retained_bytes(usize::MAX, 1),
            Err(DcsBuildError::RetainedByteCountOverflow)
        );
    }

    #[test]
    fn deserialization_revalidates_invariants_and_denies_unknown_fields() {
        let value = settings(vec![extension("platform:8.3.24", 0, b"<future/>")]);
        let mut json = serde_json::to_value(value).unwrap();
        json.as_object_mut()
            .unwrap()
            .insert("guessed_qname".to_owned(), serde_json::json!("Settings"));
        assert!(serde_json::from_value::<DcsSettings>(json).is_err());

        let value = settings(vec![extension("platform:8.3.24", 0, b"<future/>")]);
        let mut json = serde_json::to_value(value).unwrap();
        json["opaque_extensions"][0]["provenance"]["source_profile"] =
            serde_json::json!("platform:8.3.25");
        let error = serde_json::from_value::<DcsSettings>(json)
            .unwrap_err()
            .to_string();
        assert!(error.contains("different source profile"));

        let mut envelope =
            serde_json::to_value(DcsSettingsEnvelope::settings(settings(Vec::new()))).unwrap();
        envelope
            .as_object_mut()
            .unwrap()
            .insert("guessed_wrapper".to_owned(), serde_json::json!(true));
        assert!(serde_json::from_value::<DcsSettingsEnvelope>(envelope).is_err());

        let facet = serde_json::to_value(extension("platform:8.3.24", 0, b"<future/>")).unwrap();
        let over_limit = serde_json::json!({
            "items_user_setting_id": null,
            "items_view_mode": null,
            "opaque_extensions": vec![facet; MAX_DCS_OPAQUE_EXTENSIONS + 1],
            "provenance": provenance("platform:8.3.24", "settings"),
        });
        let error = serde_json::from_value::<DcsSettings>(over_limit)
            .unwrap_err()
            .to_string();
        assert!(error.contains("exceed 4096 opaque extensions"));
    }
}
