//! Deterministic compiler for the native `versions` service entry.

use std::collections::BTreeMap;

use ibcmd_core::storage::{StoragePatch, StoragePatchEntry};
use sha2::{Digest, Sha256};

use super::graph::{BootstrapGraph, InventoryScope, SpecialEntryKind};
use super::identity::{GenerationUuid, derive_generation_uuid_v8};
use super::root::{SpecialEntryBuildError, compiled_special_entry, ensure_graph_profile};
use super::version::SpecialEntryProfile;

/// Compiles a complete, lexically ordered FileName-to-generation UUID map.
///
/// `compiled_without_versions` must contain every graph key except `versions`,
/// exactly once, as a compiled single-part entry. UUIDs are derived from exact
/// payload digests, so input enumeration order and process randomness cannot
/// affect the result.
pub fn compile_versions(
    graph: &BootstrapGraph,
    compiled_without_versions: &StoragePatch,
    profile: &SpecialEntryProfile,
) -> Result<StoragePatchEntry, SpecialEntryBuildError> {
    ensure_graph_profile(graph, profile)?;
    graph.validate_special_references()?;
    graph.validate_patch_inventory(compiled_without_versions, InventoryScope::BeforeVersions)?;

    let payloads = compiled_without_versions
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.target().key().as_str(),
                entry
                    .outcome()
                    .compiled_payload()
                    .expect("graph inventory validation proved a compiled payload"),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut generations = BTreeMap::<String, GenerationUuid>::new();
    for key in graph.inventory_keys() {
        if key.as_str() == SpecialEntryKind::Versions.key() {
            continue;
        }
        let payload = payloads
            .get(key.as_str())
            .expect("graph inventory validation proved exact key coverage");
        let generation = derive_generation_uuid_v8(
            b"versions-entry-generation-v1",
            &[
                profile.profile_id().as_str().as_bytes(),
                key.as_str().as_bytes(),
                payload.sha256().as_bytes(),
            ],
        );
        generations.insert(key.as_str().to_owned(), generation);
    }

    let inventory_digest = generation_inventory_digest(&generations);
    let versions_generation = derive_generation_uuid_v8(
        b"versions-self-generation-v1",
        &[
            profile.profile_id().as_str().as_bytes(),
            inventory_digest.as_slice(),
        ],
    );
    generations.insert(
        SpecialEntryKind::Versions.key().to_owned(),
        versions_generation,
    );
    let header_generation = derive_generation_uuid_v8(
        b"versions-header-generation-v1",
        &[
            profile.profile_id().as_str().as_bytes(),
            versions_generation.as_bytes(),
        ],
    );

    let plaintext = serialize_versions(&generations, header_generation)?;
    compiled_special_entry(SpecialEntryKind::Versions, &plaintext, profile)
}

fn generation_inventory_digest(generations: &BTreeMap<String, GenerationUuid>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ibcmd-bootstrap-versions-inventory-v1\0");
    for (key, generation) in generations {
        hash_field(&mut hasher, key.as_bytes());
        hash_field(&mut hasher, generation.as_bytes());
    }
    hasher.finalize().into()
}

fn hash_field(hasher: &mut Sha256, field: &[u8]) {
    hasher.update(
        u64::try_from(field.len())
            .expect("slice length fits into u64")
            .to_le_bytes(),
    );
    hasher.update(field);
}

fn serialize_versions(
    generations: &BTreeMap<String, GenerationUuid>,
    header_generation: GenerationUuid,
) -> Result<Vec<u8>, SpecialEntryBuildError> {
    let mut plaintext = Vec::with_capacity(generations.len().saturating_mul(80));
    plaintext.extend_from_slice(b"\xef\xbb\xbf{1,");
    let pair_count =
        generations
            .len()
            .checked_add(1)
            .ok_or(SpecialEntryBuildError::PairCountOverflow {
                entry: SpecialEntryKind::Versions.key(),
            })?;
    plaintext.extend_from_slice(pair_count.to_string().as_bytes());
    plaintext.extend_from_slice(b",\"\",");
    plaintext.extend_from_slice(header_generation.to_string().as_bytes());
    for (key, generation) in generations {
        plaintext.extend_from_slice(b",\"");
        append_quoted_text(&mut plaintext, key);
        plaintext.extend_from_slice(b"\",");
        plaintext.extend_from_slice(generation.to_string().as_bytes());
    }
    plaintext.push(b'}');
    Ok(plaintext)
}

fn append_quoted_text(target: &mut Vec<u8>, value: &str) {
    for byte in value.bytes() {
        if byte == b'"' {
            target.push(b'"');
        }
        target.push(byte);
    }
}

#[cfg(test)]
mod tests {
    use ibcmd_core::artifact::ProfileId;
    use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use ibcmd_core::identity::{LogicalIdentity, ObjectUuid};
    use ibcmd_core::model::{
        CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind,
    };
    use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
    use ibcmd_core::storage::{
        MultipartIdentity, StorageKey, StoragePatchOutcome, StoragePatchTarget, StorageProvenance,
    };
    use ibcmd_core::validate::validate_configuration;

    use crate::compiler::graph::{ObjectStorageRoute, build_bootstrap_graph};
    use crate::compiler::identity::collect_bootstrap_identities;
    use crate::compiler::root::inflate_for_test;

    use super::*;

    fn uuid(value: u32) -> ObjectUuid {
        ObjectUuid::parse(&format!("00000000-0000-4000-8000-{value:012x}")).unwrap()
    }

    fn graph() -> BootstrapGraph {
        let objects = [(2, "Catalog"), (1, "Configuration")]
            .into_iter()
            .map(|(value, kind)| {
                let path =
                    ObjectPath::new(vec![PathSegment::name(&format!("object-{value}")).unwrap()])
                        .unwrap();
                let provenance = SourceProvenance::new(
                    ProfileId::parse("profile:test").unwrap(),
                    CanonicalAnchor::new(path.clone(), PropertyPath::root()),
                );
                CanonicalObject::new(CanonicalObjectParts::new(
                    LogicalIdentity::new(uuid(value), path),
                    MetadataKind::new(kind).unwrap(),
                    provenance,
                ))
                .unwrap()
            })
            .collect();
        let configuration = CanonicalConfiguration::new(objects).unwrap();
        let validated = validate_configuration(&configuration).unwrap();
        let identities = collect_bootstrap_identities(&validated).unwrap();
        build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            vec![
                ObjectStorageRoute::new(uuid(1), Vec::new()).unwrap(),
                ObjectStorageRoute::new(uuid(2), Vec::new()).unwrap(),
            ],
        )
        .unwrap()
    }

    fn compiled_patch(graph: &BootstrapGraph, reverse: bool) -> StoragePatch {
        let mut entries = graph
            .inventory_keys()
            .filter(|key| key.as_str() != SpecialEntryKind::Versions.key())
            .map(|key| {
                StoragePatchEntry::new(
                    StoragePatchTarget::new(
                        StorageKey::new(key.as_str()).unwrap(),
                        MultipartIdentity::single(),
                        StorageProvenance::new("fixture:versions").unwrap(),
                    ),
                    StoragePatchOutcome::compiled(format!("payload:{}", key.as_str()).into_bytes())
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        if reverse {
            entries.reverse();
        }
        StoragePatch::new(entries).unwrap()
    }

    fn split_versions_fields(text: &str) -> Vec<&str> {
        assert!(text.starts_with('{') && text.ends_with('}'));
        let bytes = text.as_bytes();
        let mut fields = Vec::new();
        let mut start = 1;
        let mut index = 1;
        let mut quoted = false;
        while index < bytes.len() - 1 {
            match bytes[index] {
                b'"' if quoted && bytes.get(index + 1) == Some(&b'"') => {
                    index += 2;
                    continue;
                }
                b'"' => quoted = !quoted,
                b',' if !quoted => {
                    fields.push(&text[start..index]);
                    start = index + 1;
                }
                _ => {}
            }
            index += 1;
        }
        fields.push(&text[start..bytes.len() - 1]);
        fields
    }

    #[test]
    fn versions_is_complete_sorted_and_independent_of_patch_order() {
        let graph = graph();
        let profile = SpecialEntryProfile::fixture("platform-test", 80_327);
        let first = compile_versions(&graph, &compiled_patch(&graph, false), &profile).unwrap();
        let reversed = compile_versions(&graph, &compiled_patch(&graph, true), &profile).unwrap();
        assert_eq!(first, reversed);

        let plaintext = inflate_for_test(first.outcome().compiled_payload().unwrap().bytes());
        assert!(plaintext.starts_with(b"\xef\xbb\xbf{1,6,\"\","));
        let text = std::str::from_utf8(&plaintext[3..]).unwrap();
        let fields = split_versions_fields(text);
        let pair_count = fields[1].parse::<usize>().unwrap();
        assert_eq!(fields.len(), 2 + pair_count * 2);
        assert_eq!(fields[2], "\"\"");
        let mut positions = graph
            .inventory_keys()
            .map(|key| text.find(&format!("\"{}\"", key.as_str())).unwrap())
            .collect::<Vec<_>>();
        let sorted_positions = {
            let mut sorted = positions.clone();
            sorted.sort_unstable();
            sorted
        };
        assert_eq!(positions, sorted_positions);
        positions.dedup();
        assert_eq!(positions.len(), graph.entries().len());
    }

    #[test]
    fn versions_rejects_missing_or_blocked_inventory() {
        let graph = graph();
        let profile = SpecialEntryProfile::fixture("platform-test", 80_327);
        let mut entries = compiled_patch(&graph, false).into_entries();
        entries.pop();
        let missing = StoragePatch::new(entries).unwrap();
        assert!(matches!(
            compile_versions(&graph, &missing, &profile),
            Err(SpecialEntryBuildError::Graph(
                super::super::graph::BootstrapGraphError::InventoryMismatch { .. }
            ))
        ));

        let mut entries = compiled_patch(&graph, false).into_entries();
        let target = entries[0].target().clone();
        entries[0] = StoragePatchEntry::new(
            target,
            StoragePatchOutcome::unsupported("fixture blocker").unwrap(),
        );
        let blocked = StoragePatch::new(entries).unwrap();
        assert!(matches!(
            compile_versions(&graph, &blocked, &profile),
            Err(SpecialEntryBuildError::Graph(
                super::super::graph::BootstrapGraphError::EntryBlocked { .. }
            ))
        ));
    }
}
