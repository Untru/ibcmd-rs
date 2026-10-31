//! Diagnostic-only import observations. No result grants mutation authority.
//!
//! The metadata audit normalizes text. Its header and kind are candidates,
//! never proof of the original owner identity, full parent path or storage role.
#![allow(dead_code)] // Isolated extraction seam; production adapters are separately reviewed.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use flate2::{Decompress, FlushDecompress, Status};
use sha2::{Digest, Sha256};

use super::metadata::{MetadataTextRowAudit, metadata_text_row_audit_from_text};
use crate::metadata_owner_graph::{OwnerCollectionProvenance, OwnerGraphFamily};

pub(crate) const DIGEST_DOMAIN: &str = "ibcmd-import-observation-v1";

/// Assembled entry occurrence, not a SQL PartNo or physical provenance proof.
pub(crate) struct PackedCandidate<'a> {
    pub(crate) occurrence_ordinal: u64,
    pub(crate) origin_label: &'a str,
    pub(crate) table_label: &'a str,
    pub(crate) stored_name: &'a str,
    pub(crate) logical_key: &'a str,
    pub(crate) packed: &'a [u8],
}

pub(crate) struct ViewInput<'a> {
    pub(crate) view_label: &'a str,
    pub(crate) schema_label: &'a str,
    pub(crate) platform_label: &'a str,
    pub(crate) candidates: &'a [PackedCandidate<'a>],
}

#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub(crate) max_candidates: usize,
    pub(crate) max_label_bytes: usize,
    pub(crate) max_packed_bytes: usize,
    pub(crate) max_inflated_bytes: usize,
    pub(crate) max_collection_items: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObserveError {
    Cancelled,
    Limit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphStatus {
    Unsupported,
    Rejected,
    ObservedCandidates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProfileStatus {
    Unsupported,
    /// Catalog recognition only; does not validate a payload's actual profile.
    RecognizedExactPlatformLabel,
}

/// Ordered raw collection items remain opaque: no recursive UUID guessing.
pub(crate) struct CollectionObservation {
    provenance: OwnerCollectionProvenance,
    item_sha256: Vec<[u8; 32]>,
}

impl CollectionObservation {
    pub(crate) fn provenance(&self) -> OwnerCollectionProvenance {
        self.provenance
    }

    pub(crate) fn item_sha256(&self) -> &[[u8; 32]] {
        &self.item_sha256
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NormalizationTrace {
    Unavailable,
}

/// Owned copies of exact occurrence labels plus bounded decoder diagnostics.
/// Deliberately no Debug implementation: labels may contain private names.
pub(crate) struct CandidateObservation {
    occurrence_ordinal: u64,
    origin_label: String,
    table_label: String,
    stored_name: String,
    logical_key: String,
    packed_sha256: [u8; 32],
    inflated_utf8_sha256: Option<[u8; 32]>,
    normalized_text_sha256: Option<[u8; 32]>,
    audit_issue: Option<&'static str>,
    ambiguous: bool,
    uuid_candidate: Option<String>,
    name_candidate: Option<String>,
    kind_candidate: Option<String>,
    graph_status: GraphStatus,
    graph_diagnostic: Option<super::MetadataSourceExtractionDiagnostic>,
    collections: Vec<CollectionObservation>,
}

impl CandidateObservation {
    pub(crate) fn occurrence_ordinal(&self) -> u64 {
        self.occurrence_ordinal
    }
    pub(crate) fn origin_label(&self) -> &str {
        &self.origin_label
    }
    pub(crate) fn table_label(&self) -> &str {
        &self.table_label
    }
    pub(crate) fn stored_name(&self) -> &str {
        &self.stored_name
    }
    pub(crate) fn logical_key(&self) -> &str {
        &self.logical_key
    }
    pub(crate) fn packed_sha256(&self) -> &[u8; 32] {
        &self.packed_sha256
    }
    pub(crate) fn inflated_utf8_sha256(&self) -> Option<&[u8; 32]> {
        self.inflated_utf8_sha256.as_ref()
    }
    /// Individual normalization steps are not exposed by the existing audit.
    /// audit_issue=None never proves original bytes or semantics unchanged.
    pub(crate) fn normalization_trace(&self) -> NormalizationTrace {
        NormalizationTrace::Unavailable
    }
    pub(crate) fn normalized_text_sha256(&self) -> Option<&[u8; 32]> {
        self.normalized_text_sha256.as_ref()
    }
    pub(crate) fn audit_issue(&self) -> Option<&'static str> {
        self.audit_issue
    }
    /// True when an occurrence cannot bind uniquely, including a missing
    /// key-anchored header. This is not a count or proof of multiple owners.
    /// audit_issue=header distinguishes that absence from occurrence collisions.
    pub(crate) fn ambiguous(&self) -> bool {
        self.ambiguous
    }
    /// The audit searches by the supplied key; this is a key-anchored header
    /// candidate, not an independently discovered UUID or full parent binding.
    pub(crate) fn uuid_candidate(&self) -> Option<&str> {
        self.uuid_candidate.as_deref()
    }
    pub(crate) fn name_candidate(&self) -> Option<&str> {
        self.name_candidate.as_deref()
    }
    pub(crate) fn kind_candidate(&self) -> Option<&str> {
        self.kind_candidate.as_deref()
    }
    pub(crate) fn graph_status(&self) -> GraphStatus {
        self.graph_status
    }
    pub(crate) fn graph_diagnostic(&self) -> Option<&super::MetadataSourceExtractionDiagnostic> {
        self.graph_diagnostic.as_ref()
    }
    pub(crate) fn collections(&self) -> &[CollectionObservation] {
        &self.collections
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnsupportedDimension {
    RawProfileValidation,
    FullParentBinding,
    BodyRoleBinding,
    Versions,
    StorageProjection,
    OwnerCompleteness,
}

pub(crate) struct ViewObservation {
    view_label: String,
    schema_label: String,
    platform_label: String,
    ordered_view_sha256: [u8; 32],
    profile_status: ProfileStatus,
    candidates: Vec<CandidateObservation>,
}

impl ViewObservation {
    pub(crate) fn view_label(&self) -> &str {
        &self.view_label
    }
    pub(crate) fn schema_label(&self) -> &str {
        &self.schema_label
    }
    pub(crate) fn platform_label(&self) -> &str {
        &self.platform_label
    }
    /// Domain differs from a canonical packed-map digest. Never interchange.
    pub(crate) fn ordered_view_sha256(&self) -> &[u8; 32] {
        &self.ordered_view_sha256
    }
    pub(crate) fn profile_status(&self) -> ProfileStatus {
        self.profile_status
    }
    pub(crate) fn candidates(&self) -> &[CandidateObservation] {
        &self.candidates
    }
    pub(crate) fn unsupported_dimensions(&self) -> &'static [UnsupportedDimension] {
        &[
            UnsupportedDimension::RawProfileValidation,
            UnsupportedDimension::FullParentBinding,
            UnsupportedDimension::BodyRoleBinding,
            UnsupportedDimension::Versions,
            UnsupportedDimension::StorageProjection,
            UnsupportedDimension::OwnerCompleteness,
        ]
    }
    /// Full parent, body role, versions and preprojection hooks are absent.
    pub(crate) fn require_owner_completeness(&self) -> Result<(), UnsupportedAuthority> {
        Err(UnsupportedAuthority)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct UnsupportedAuthority;

fn frame(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn add(total: &mut usize, bytes: usize, limit: usize) -> Result<(), ObserveError> {
    *total = total.checked_add(bytes).ok_or(ObserveError::Limit)?;
    if *total > limit {
        return Err(ObserveError::Limit);
    }
    Ok(())
}

fn family(kind: &str) -> Option<OwnerGraphFamily> {
    [
        OwnerGraphFamily::Catalog,
        OwnerGraphFamily::Document,
        OwnerGraphFamily::BusinessProcess,
        OwnerGraphFamily::ChartOfCharacteristicTypes,
    ]
    .into_iter()
    .find(|family| family.as_str() == kind)
}

fn unsafe_label(value: &str) -> bool {
    value.is_empty()
        || value.chars().any(char::is_control)
        || value.starts_with(['/', '\\'])
        || value.contains(':')
        || value.split(['/', '\\']).any(|part| {
            let mut components = Path::new(part).components();
            !matches!(
                (components.next(), components.next()),
                (Some(Component::Normal(_)), None)
            )
        })
}

fn alias(value: &str) -> String {
    value.replace('\\', "/").to_lowercase()
}

/// Entire call is synchronous. The callback may borrow local cancellation state.
/// Limits bound supplied bytes and retained diagnostics, not private decoder CPU.
pub(crate) fn observe_view(
    input: ViewInput<'_>,
    limits: Limits,
    check: &mut dyn FnMut() -> Result<(), ObserveError>,
) -> Result<ViewObservation, ObserveError> {
    check()?;
    if input.candidates.len() > limits.max_candidates {
        return Err(ObserveError::Limit);
    }
    let mut labels = 0;
    for label in [input.view_label, input.schema_label, input.platform_label] {
        add(&mut labels, label.len(), limits.max_label_bytes)?;
    }
    let mut packed_bytes = 0;
    for candidate in input.candidates {
        check()?;
        for label in [
            candidate.origin_label,
            candidate.table_label,
            candidate.stored_name,
            candidate.logical_key,
        ] {
            add(&mut labels, label.len(), limits.max_label_bytes)?;
        }
        add(
            &mut packed_bytes,
            candidate.packed.len(),
            limits.max_packed_bytes,
        )?;
    }
    let mut hash = Sha256::new();
    frame(&mut hash, DIGEST_DOMAIN.as_bytes());
    for label in [input.view_label, input.schema_label, input.platform_label] {
        frame(&mut hash, label.as_bytes());
    }
    hash.update((input.candidates.len() as u64).to_le_bytes());
    let mut observations = Vec::with_capacity(input.candidates.len());
    let mut inflated_bytes = 0;
    let mut normalized_bytes = 0;
    let mut collection_items = 0;
    // Maps retain only occurrence indexes. They never select a winning key.
    let mut keys: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut names: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut ordinals: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (index, candidate) in input.candidates.iter().enumerate() {
        check()?;
        hash.update(candidate.occurrence_ordinal.to_le_bytes());
        for label in [
            candidate.origin_label,
            candidate.table_label,
            candidate.stored_name,
            candidate.logical_key,
        ] {
            frame(&mut hash, label.as_bytes());
        }
        frame(&mut hash, candidate.packed);
        keys.entry(alias(candidate.logical_key))
            .or_default()
            .push(index);
        names
            .entry(alias(candidate.stored_name))
            .or_default()
            .push(index);
        ordinals
            .entry(candidate.occurrence_ordinal)
            .or_default()
            .push(index);
        let mut observation = CandidateObservation {
            occurrence_ordinal: candidate.occurrence_ordinal,
            origin_label: candidate.origin_label.to_owned(),
            table_label: candidate.table_label.to_owned(),
            stored_name: candidate.stored_name.to_owned(),
            logical_key: candidate.logical_key.to_owned(),
            packed_sha256: Sha256::digest(candidate.packed).into(),
            inflated_utf8_sha256: None,
            normalized_text_sha256: None,
            audit_issue: None,
            ambiguous: unsafe_label(candidate.logical_key) || unsafe_label(candidate.stored_name),
            uuid_candidate: None,
            name_candidate: None,
            kind_candidate: None,
            graph_status: GraphStatus::Unsupported,
            graph_diagnostic: None,
            collections: Vec::new(),
        };
        let mut decoder = Decompress::new(false);
        let mut text = Vec::new();
        let mut buffer = [0; 4096];
        let inflated = loop {
            check()?;
            let before_in = decoder.total_in();
            let before_out = decoder.total_out();
            let status = decoder.decompress(
                &candidate.packed[before_in as usize..],
                &mut buffer,
                FlushDecompress::None,
            );
            let produced = (decoder.total_out() - before_out) as usize;
            add(&mut inflated_bytes, produced, limits.max_inflated_bytes)?;
            text.extend_from_slice(&buffer[..produced]);
            match status {
                Ok(Status::StreamEnd) => break decoder.total_in() == candidate.packed.len() as u64,
                Err(_) => break false,
                Ok(_) if decoder.total_in() == before_in && produced == 0 => break false,
                Ok(_) => {}
            }
        };
        if !inflated {
            observation.audit_issue = Some("inflate");
        } else if let Ok(text) = String::from_utf8(text) {
            observation.inflated_utf8_sha256 = Some(Sha256::digest(text.as_bytes()).into());
            check()?;
            let audit = metadata_text_row_audit_from_text(
                candidate.logical_key,
                text.trim_start_matches('\u{feff}').to_owned(),
            );
            let row = match audit {
                MetadataTextRowAudit::Extracted(row) => Some(row),
                MetadataTextRowAudit::ExtractedWithWarning(row, warning) => {
                    observation.audit_issue = Some(warning.reason.as_str());
                    Some(row)
                }
                MetadataTextRowAudit::Miss(miss) => {
                    observation.audit_issue = Some(miss.reason.as_str());
                    None
                }
            };
            if let Some(row) = row {
                add(
                    &mut normalized_bytes,
                    row.text.len(),
                    limits.max_inflated_bytes,
                )?;
                observation.normalized_text_sha256 =
                    Some(Sha256::digest(row.text.as_bytes()).into());
                observation.kind_candidate = row.kind.clone();
                if row.header.is_none() && row.kind.as_deref().and_then(family).is_some() {
                    // No header bound to this exact key. Preserve the audit warning;
                    // do not search for another UUID or infer multiple owners.
                    observation.ambiguous = true;
                    observation.graph_status = GraphStatus::Rejected;
                }
                if let Some(header) = row.header.as_ref() {
                    observation.uuid_candidate = Some(header.uuid.clone());
                    observation.name_candidate = Some(header.name.clone());
                    observation.ambiguous |= header.uuid != candidate.logical_key;
                    if let Some(family) = row.kind.as_deref().and_then(family) {
                        // A warning or mismatched exact key cannot become a graph witness.
                        if observation.audit_issue.is_some() || observation.ambiguous {
                            observation.graph_status = GraphStatus::Rejected;
                        } else {
                            check()?;
                            match super::decode_owner_graph(family, &row.text, header) {
                                Ok(graph) => {
                                    observation.graph_status = GraphStatus::ObservedCandidates;
                                    for collection in graph.collections {
                                        check()?;
                                        add(
                                            &mut collection_items,
                                            collection.items.len(),
                                            limits.max_collection_items,
                                        )?;
                                        observation.collections.push(CollectionObservation {
                                            provenance: collection.provenance(),
                                            item_sha256: collection
                                                .items
                                                .iter()
                                                .map(|item| Sha256::digest(item.as_bytes()).into())
                                                .collect(),
                                        });
                                    }
                                }
                                Err(diagnostic) => {
                                    observation.graph_status = GraphStatus::Rejected;
                                    observation.graph_diagnostic = Some(diagnostic);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            observation.audit_issue = Some("utf8");
        }
        observations.push(observation);
    }
    for occurrences in keys.values().chain(names.values()).chain(ordinals.values()) {
        if occurrences.len() > 1 {
            for index in occurrences {
                observations[*index].ambiguous = true;
                observations[*index].graph_status = GraphStatus::Rejected;
            }
        }
    }
    check()?;
    let profile_status = match crate::platform::parse(input.platform_label) {
        Ok(spec) if spec.is_exact_build() && spec.native_profile().is_some() => {
            ProfileStatus::RecognizedExactPlatformLabel
        }
        _ => ProfileStatus::Unsupported,
    };
    Ok(ViewObservation {
        view_label: input.view_label.to_owned(),
        schema_label: input.schema_label.to_owned(),
        platform_label: input.platform_label.to_owned(),
        ordered_view_sha256: hash.finalize().into(),
        profile_status,
        candidates: observations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_owner_graph::OwnerHeaderEncoding;
    use flate2::{Compression, write::DeflateEncoder};
    use std::io::Write;

    const OWNER: &str = "10000000-0000-0000-0000-000000000001";

    fn limits() -> Limits {
        Limits {
            max_candidates: 16,
            max_label_bytes: 4096,
            max_packed_bytes: 65536,
            max_inflated_bytes: 65536,
            max_collection_items: 128,
        }
    }

    fn pack(text: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(text).unwrap();
        encoder.finish().unwrap()
    }

    fn candidate<'a>(key: &'a str, packed: &'a [u8], ordinal: u64) -> PackedCandidate<'a> {
        PackedCandidate {
            occurrence_ordinal: ordinal,
            origin_label: "fixture",
            table_label: "CONFIG",
            stored_name: key,
            logical_key: key,
            packed,
        }
    }

    fn input<'a>(candidates: &'a [PackedCandidate<'a>]) -> ViewInput<'a> {
        ViewInput {
            view_label: "old",
            schema_label: "unverified-fixture-schema",
            platform_label: "unknown-platform",
            candidates,
        }
    }

    fn observe(candidates: &[PackedCandidate<'_>]) -> ViewObservation {
        observe_view(input(candidates), limits(), &mut || Ok(())).unwrap()
    }

    // Clean-room descriptor, built from engine-owned schema fields. No raw corpus.
    fn graph_text(family: OwnerGraphFamily, duplicate_form: bool) -> String {
        let layout = family.layout();
        let mut fields = vec!["0".to_owned(); layout.owner_field_count];
        fields[0] = layout.owner_discriminators[0].to_owned();
        let header = format!(
            "{{3,{{1,0,{OWNER}}},\"Owner\",{{0}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}}"
        );
        fields[layout.owner_header_slot] = match layout.owner_header_encoding {
            OwnerHeaderEncoding::Direct => header,
            OwnerHeaderEncoding::Wrapped => format!("{{0,{header}}}"),
        };
        for (index, generated) in layout.generated_types.iter().enumerate() {
            fields[generated.type_slot] = format!("20000000-0000-0000-0000-{:012x}", index * 2 + 1);
            fields[generated.value_slot] =
                format!("20000000-0000-0000-0000-{:012x}", index * 2 + 2);
        }
        for (index, value) in layout.owner_reserved_fields {
            fields[*index] = (*value).to_owned();
        }
        fields[super::super::characteristics::characteristics_slot(family.into())] =
            "{0,{0}}".to_owned();
        let collections: Vec<_> = layout.collection_layouts().iter().map(|collection| {
            if duplicate_form && collection.role == crate::metadata_owner_graph::OwnerCollectionRole::Form {
                "{MARKER,2,30000000-0000-0000-0000-000000000001,30000000-0000-0000-0000-000000000001}".replace("MARKER", collection.marker)
            } else { format!("{{{},0}}", collection.marker) }
        }).collect();
        format!(
            "{{1,{{{}}},{},{}}}",
            fields.join(","),
            layout.root_collection_count_token,
            collections.join(",")
        )
    }

    #[test]
    fn typed_family_lookup_keeps_the_exact_supported_boundary() {
        for name in [
            "Catalog",
            "Document",
            "BusinessProcess",
            "ChartOfCharacteristicTypes",
        ] {
            assert_eq!(family(name).map(OwnerGraphFamily::as_str), Some(name));
        }
        for name in [
            "",
            "catalog",
            "CATALOG",
            "Catalog ",
            " Catalog",
            "Form",
            "Task",
            "Catalog.New",
        ] {
            assert!(family(name).is_none(), "unexpected family {name:?}");
        }
    }

    #[test]
    fn component_parser_preserves_the_original_lexical_predicate() {
        fn original(value: &str) -> bool {
            value.is_empty()
                || value.chars().any(char::is_control)
                || value.starts_with(['/', '\\'])
                || value.contains(':')
                || value
                    .split(['/', '\\'])
                    .any(|part| matches!(part, "" | "." | ".."))
        }
        for value in [
            "",
            ".",
            "..",
            "...",
            "a",
            "A",
            "a.b",
            "a/b",
            "a\\b",
            "a//b",
            "a\\\\b",
            "a/./b",
            "a\\..\\b",
            "/root",
            "\\root",
            "a/",
            "a\\",
            ":",
            "C:relative",
            " a ",
            "Каталог/Поле",
            "控制",
            "a／b",
            "a\nb",
            "a\0b",
            "a\u{7f}b",
        ] {
            assert_eq!(
                unsafe_label(value),
                original(value),
                "lexical change for {value:?}"
            );
        }
    }

    #[test]
    fn four_families_preserve_schema_collections_without_authority() {
        for family in [
            OwnerGraphFamily::Catalog,
            OwnerGraphFamily::Document,
            OwnerGraphFamily::BusinessProcess,
            OwnerGraphFamily::ChartOfCharacteristicTypes,
        ] {
            let packed = pack(graph_text(family, false).as_bytes());
            let result = observe(&[candidate(OWNER, &packed, 7)]);
            let row = &result.candidates()[0];
            assert_eq!(
                row.graph_status(),
                GraphStatus::ObservedCandidates,
                "{}",
                family.as_str()
            );
            assert_eq!(row.uuid_candidate(), Some(OWNER));
            assert_eq!(row.kind_candidate(), Some(family.as_str()));
            assert_eq!(row.name_candidate(), Some("Owner"));
            assert_eq!(
                row.collections().len(),
                family.layout().collection_layouts().len()
            );
            for (observed, expected) in row
                .collections()
                .iter()
                .zip(family.layout().collection_layouts())
            {
                assert_eq!(
                    observed.provenance(),
                    OwnerCollectionProvenance::from_layout(*expected)
                );
                assert!(observed.item_sha256().is_empty());
            }
            assert_eq!(result.profile_status(), ProfileStatus::Unsupported);
            assert_eq!(
                result.require_owner_completeness(),
                Err(UnsupportedAuthority)
            );
        }
    }

    #[test]
    fn duplicate_collection_items_keep_order_and_hashes() {
        let packed = pack(graph_text(OwnerGraphFamily::Catalog, true).as_bytes());
        let result = observe(&[candidate(OWNER, &packed, 0)]);
        let row = &result.candidates()[0];
        assert_eq!(row.graph_status(), GraphStatus::ObservedCandidates);
        let forms = row
            .collections()
            .iter()
            .find(|item| {
                item.provenance().role == crate::metadata_owner_graph::OwnerCollectionRole::Form
            })
            .unwrap();
        assert_eq!(forms.item_sha256().len(), 2);
        assert_eq!(forms.item_sha256()[0], forms.item_sha256()[1]);
        assert!(result.require_owner_completeness().is_err());
    }

    #[test]
    fn exact_duplicates_are_retained_and_marked_ambiguous() {
        let packed = pack(b"opaque");
        let result = observe(&[candidate("key", &packed, 0), candidate("key", &packed, 0)]);
        assert_eq!(result.candidates().len(), 2);
        assert!(
            result
                .candidates()
                .iter()
                .all(|row| row.ambiguous() && row.graph_status() == GraphStatus::Rejected)
        );
    }

    #[test]
    fn slash_case_aliases_do_not_choose_a_winner() {
        let packed = pack(b"opaque");
        let result = observe(&[candidate("A/B", &packed, 1), candidate("a\\b", &packed, 2)]);
        assert_eq!(result.candidates()[0].logical_key(), "A/B");
        assert_eq!(result.candidates()[1].logical_key(), "a\\b");
        assert!(
            result
                .candidates()
                .iter()
                .all(CandidateObservation::ambiguous)
        );
    }

    #[test]
    fn ordinal_collision_alone_is_ambiguous() {
        let packed = pack(b"opaque");
        let result = observe(&[candidate("a", &packed, 3), candidate("b", &packed, 3)]);
        assert!(
            result
                .candidates()
                .iter()
                .all(CandidateObservation::ambiguous)
        );
    }

    #[test]
    fn header_key_mismatch_does_not_mint_owner_binding() {
        let packed = pack(graph_text(OwnerGraphFamily::Catalog, false).as_bytes());
        let result = observe(&[candidate(
            "10000000-0000-0000-0000-000000000002",
            &packed,
            0,
        )]);
        assert!(result.candidates()[0].ambiguous());
        assert_eq!(result.candidates()[0].graph_status(), GraphStatus::Rejected);
        assert!(result.candidates()[0].collections().is_empty());
    }

    #[test]
    fn missing_key_anchored_header_is_unbound_not_an_invented_other_owner() {
        let text = graph_text(OwnerGraphFamily::Catalog, false)
            .replace(&format!("{{1,0,{OWNER}}}"), "{0}");
        let packed = pack(text.as_bytes());
        let result = observe(&[candidate(OWNER, &packed, 0)]);
        let row = &result.candidates()[0];
        assert_eq!(row.kind_candidate(), Some("Catalog"));
        assert_eq!(row.audit_issue(), Some("header"));
        assert!(row.ambiguous());
        assert_eq!(row.graph_status(), GraphStatus::Rejected);
        assert!(row.uuid_candidate().is_none());
        assert!(row.name_candidate().is_none());
        assert!(row.graph_diagnostic().is_none());
        assert!(row.collections().is_empty());
        assert!(result.require_owner_completeness().is_err());
    }

    #[test]
    fn full_view_digest_binds_order_bytes_labels_and_ordinals() {
        let a = pack(b"a");
        let b = pack(b"b");
        let baseline = observe(&[candidate("a", &a, 1), candidate("b", &b, 2)]);
        for changed in [
            observe(&[candidate("b", &b, 2), candidate("a", &a, 1)]),
            observe(&[candidate("a", &b, 1), candidate("b", &b, 2)]),
            observe(&[candidate("a", &a, 9), candidate("b", &b, 2)]),
        ] {
            assert_ne!(
                baseline.ordered_view_sha256(),
                changed.ordered_view_sha256()
            );
        }
        let candidates = [candidate("a", &a, 1), candidate("b", &b, 2)];
        let mut new = input(&candidates);
        new.view_label = "new";
        let changed = observe_view(new, limits(), &mut || Ok(())).unwrap();
        assert_ne!(
            baseline.ordered_view_sha256(),
            changed.ordered_view_sha256()
        );
    }

    #[test]
    fn hashes_are_framed_not_concatenated() {
        let packed = pack(b"opaque");
        let mut a = candidate("key", &packed, 1);
        a.origin_label = "ab";
        a.table_label = "c";
        let mut b = candidate("key", &packed, 1);
        b.origin_label = "a";
        b.table_label = "bc";
        assert_ne!(
            observe(&[a]).ordered_view_sha256(),
            observe(&[b]).ordered_view_sha256()
        );
    }

    #[test]
    fn result_outlives_borrowed_payload_and_callback() {
        let result = {
            let packed = pack(b"opaque");
            let candidates = [candidate("key", &packed, 0)];
            let mut checks = 0;
            observe_view(input(&candidates), limits(), &mut || {
                checks += 1;
                Ok(())
            })
            .unwrap()
        };
        assert_eq!(result.candidates()[0].logical_key(), "key");
        assert_eq!(result.candidates()[0].occurrence_ordinal(), 0);
    }

    #[test]
    fn typed_cancellation_returns_no_partial_observation() {
        let packed = pack(b"opaque");
        let candidates = [candidate("key", &packed, 0)];
        let mut checks = 0;
        let result = observe_view(input(&candidates), limits(), &mut || {
            checks += 1;
            if checks == 4 {
                Err(ObserveError::Cancelled)
            } else {
                Ok(())
            }
        });
        assert!(matches!(result, Err(ObserveError::Cancelled)));
    }

    #[test]
    fn bounds_reject_candidates_labels_packed_and_inflated_bytes() {
        let packed = pack(&vec![b'x'; 20000]);
        let candidates = [candidate("key", &packed, 0)];
        for bounded in [
            Limits {
                max_candidates: 0,
                ..limits()
            },
            Limits {
                max_label_bytes: 0,
                ..limits()
            },
            Limits {
                max_packed_bytes: 0,
                ..limits()
            },
            Limits {
                max_inflated_bytes: 16,
                ..limits()
            },
        ] {
            assert!(matches!(
                observe_view(input(&candidates), bounded, &mut || Ok(())),
                Err(ObserveError::Limit)
            ));
        }
    }

    #[test]
    fn collection_item_budget_is_cumulative() {
        let packed = pack(graph_text(OwnerGraphFamily::Catalog, true).as_bytes());
        let candidates = [candidate(OWNER, &packed, 0)];
        assert!(matches!(
            observe_view(
                input(&candidates),
                Limits {
                    max_collection_items: 1,
                    ..limits()
                },
                &mut || Ok(())
            ),
            Err(ObserveError::Limit)
        ));
    }

    #[test]
    fn warnings_and_opaque_misses_are_retained() {
        let packed = pack(b"opaque");
        let invalid_utf8 = pack(&[255]);
        let result = observe(&[
            candidate("a", &packed, 0),
            candidate("b", &invalid_utf8, 1),
            candidate("c", &[255, 255, 255], 2),
        ]);
        assert_eq!(result.candidates()[0].audit_issue(), Some("object_code"));
        assert_eq!(result.candidates()[1].audit_issue(), Some("utf8"));
        assert_eq!(result.candidates()[2].audit_issue(), Some("inflate"));
        assert!(
            result
                .candidates()
                .iter()
                .all(|row| row.graph_status() == GraphStatus::Unsupported)
        );
        assert!(result.require_owner_completeness().is_err());
    }

    #[test]
    fn truncated_or_trailing_deflate_never_becomes_graph_candidates() {
        let valid = pack(graph_text(OwnerGraphFamily::Catalog, false).as_bytes());
        let mut trailing = valid.clone();
        trailing.extend_from_slice(b"opaque suffix");
        let truncated = valid[..valid.len() - 1].to_vec();
        for invalid in [trailing, truncated] {
            let result = observe(&[candidate(OWNER, &invalid, 0)]);
            assert_eq!(result.candidates()[0].audit_issue(), Some("inflate"));
            assert_eq!(
                result.candidates()[0].graph_status(),
                GraphStatus::Unsupported
            );
            assert!(result.candidates()[0].uuid_candidate().is_none());
            assert!(result.require_owner_completeness().is_err());
        }
    }

    #[test]
    fn unsafe_keys_are_retained_but_cannot_bind_a_graph() {
        let packed = pack(b"opaque");
        for key in ["", "../key", "/absolute", "C:\\key", "a\0b", "a//b"] {
            let result = observe(&[candidate(key, &packed, 0)]);
            assert_eq!(result.candidates()[0].logical_key(), key);
            assert!(result.candidates()[0].ambiguous());
            assert!(result.require_owner_completeness().is_err());
        }
    }

    #[test]
    fn audit_without_warning_does_not_certify_no_normalization() {
        let original = format!("\u{feff}{}", graph_text(OwnerGraphFamily::Catalog, false));
        let packed = pack(original.as_bytes());
        let result = observe(&[candidate(OWNER, &packed, 0)]);
        let row = &result.candidates()[0];
        assert_eq!(row.audit_issue(), None);
        assert_eq!(row.normalization_trace(), NormalizationTrace::Unavailable);
        let expected: [u8; 32] = Sha256::digest(original.as_bytes()).into();
        assert_eq!(row.inflated_utf8_sha256(), Some(&expected));
        assert_ne!(row.inflated_utf8_sha256(), row.normalized_text_sha256());
        assert_eq!(row.graph_status(), GraphStatus::ObservedCandidates);
        assert!(result.require_owner_completeness().is_err());
    }

    #[test]
    fn malformed_supported_graph_is_rejected() {
        let text = graph_text(OwnerGraphFamily::Catalog, false);
        let marker = OwnerGraphFamily::Catalog.layout().collection_markers[0];
        let packed = pack(text.replace(marker, "not-a-marker").as_bytes());
        let result = observe(&[candidate(OWNER, &packed, 0)]);
        assert_eq!(result.candidates()[0].graph_status(), GraphStatus::Rejected);
        assert!(result.candidates()[0].collections().is_empty());
        assert!(result.candidates()[0].graph_diagnostic().is_some());
        assert!(result.require_owner_completeness().is_err());
    }
}
