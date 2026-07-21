//! Fail-closed canonical graph validation.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::{
    Diagnostic, DiagnosticCode, DiagnosticReport, ObjectPath, PathSegment, PropertyPath, Severity,
};
use crate::graph::GraphIndex;
use crate::identity::ObjectUuid;
use crate::model::{CanonicalConfiguration, CanonicalObject};

/// Stable code for a collision in the global object/generated-type UUID namespace.
pub const DUPLICATE_UUID_CODE: &str = "model.duplicate-uuid";
/// Stable code for duplicate exact logical object paths.
pub const DUPLICATE_PATH_CODE: &str = "model.duplicate-path";
/// Stable code for an owner UUID that is not a canonical object.
pub const DANGLING_OWNER_CODE: &str = "model.dangling-owner";
/// Stable code for a reference target absent from objects and generated types.
pub const DANGLING_REFERENCE_CODE: &str = "model.dangling-reference";
/// Stable code for self or multi-object ownership cycles.
pub const OWNERSHIP_CYCLE_CODE: &str = "model.ownership-cycle";

/// Encode-ready proof that a configuration has no graph validation errors.
///
/// Fields and construction are private. Callers can obtain this token only
/// through [`validate_configuration`].
#[derive(Debug)]
pub struct ValidatedConfiguration<'a> {
    configuration: &'a CanonicalConfiguration,
    graph: GraphIndex,
}

impl<'a> ValidatedConfiguration<'a> {
    /// Returns the validated immutable configuration.
    pub const fn configuration(&self) -> &'a CanonicalConfiguration {
        self.configuration
    }

    /// Returns complete duplicate-free graph indexes.
    pub const fn graph(&self) -> &GraphIndex {
        &self.graph
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum UuidOccurrence {
    Object {
        object_index: usize,
    },
    GeneratedType {
        object_index: usize,
        generated_type_index: usize,
    },
}

impl UuidOccurrence {
    const fn object_index(self) -> usize {
        match self {
            Self::Object { object_index } | Self::GeneratedType { object_index, .. } => {
                object_index
            }
        }
    }
}

/// Validates identity, ownership, and reference integrity without mutation.
///
/// Every returned diagnostic has `Severity::Error`, exact object/property
/// coordinates, and source profile evidence. The report constructor applies
/// canonical sorting, so diagnostic order does not depend on traversal order.
pub fn validate_configuration(
    configuration: &CanonicalConfiguration,
) -> Result<ValidatedConfiguration<'_>, DiagnosticReport> {
    let mut diagnostics = Vec::new();
    let uuid_occurrences = collect_uuid_occurrences(configuration);
    validate_duplicate_uuids(configuration, &uuid_occurrences, &mut diagnostics);
    validate_duplicate_paths(configuration, &mut diagnostics);

    let object_uuids = configuration
        .objects()
        .iter()
        .map(|object| object.identity().uuid())
        .collect::<BTreeSet<_>>();
    let reference_targets = uuid_occurrences.keys().copied().collect::<BTreeSet<_>>();
    validate_owners_and_references(
        configuration,
        &object_uuids,
        &reference_targets,
        &mut diagnostics,
    );
    validate_ownership_cycles(
        configuration,
        &uuid_occurrences,
        &object_uuids,
        &mut diagnostics,
    );

    let report = DiagnosticReport::from_diagnostics(diagnostics);
    if report.has_errors() {
        return Err(report);
    }

    let graph = GraphIndex::new(configuration)
        .expect("duplicate-free validated configuration produces complete graph indexes");
    Ok(ValidatedConfiguration {
        configuration,
        graph,
    })
}

fn collect_uuid_occurrences(
    configuration: &CanonicalConfiguration,
) -> BTreeMap<ObjectUuid, Vec<UuidOccurrence>> {
    let mut occurrences = BTreeMap::<ObjectUuid, Vec<UuidOccurrence>>::new();
    for (object_index, object) in configuration.objects().iter().enumerate() {
        occurrences
            .entry(object.identity().uuid())
            .or_default()
            .push(UuidOccurrence::Object { object_index });
        for (generated_type_index, generated_type) in object.generated_types().iter().enumerate() {
            occurrences.entry(generated_type.uuid()).or_default().push(
                UuidOccurrence::GeneratedType {
                    object_index,
                    generated_type_index,
                },
            );
        }
    }
    occurrences
}

fn validate_duplicate_uuids(
    configuration: &CanonicalConfiguration,
    occurrences: &BTreeMap<ObjectUuid, Vec<UuidOccurrence>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (uuid, locations) in occurrences {
        if locations.len() < 2 {
            continue;
        }
        for location in locations {
            let object = &configuration.objects()[location.object_index()];
            let property_path = match *location {
                UuidOccurrence::Object { .. } => identity_uuid_path(),
                UuidOccurrence::GeneratedType {
                    generated_type_index,
                    ..
                } => indexed_property_path("generated_types", generated_type_index, "uuid"),
            };
            let node_kind = match location {
                UuidOccurrence::Object { .. } => "object",
                UuidOccurrence::GeneratedType { .. } => "generated_type",
            };
            diagnostics.push(
                error_diagnostic(
                    DUPLICATE_UUID_CODE,
                    object,
                    property_path,
                    "UUID is declared by more than one object or generated type",
                )
                .with_context("uuid", &uuid.to_string())
                .expect("canonical UUID context is bounded")
                .with_context("node_kind", node_kind)
                .expect("static node-kind context is bounded"),
            );
        }
    }
}

fn validate_duplicate_paths(
    configuration: &CanonicalConfiguration,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut by_path = BTreeMap::<&ObjectPath, Vec<usize>>::new();
    for (object_index, object) in configuration.objects().iter().enumerate() {
        by_path
            .entry(object.identity().path())
            .or_default()
            .push(object_index);
    }
    for (_path, object_indexes) in by_path {
        if object_indexes.len() < 2 {
            continue;
        }
        for object_index in object_indexes {
            let object = &configuration.objects()[object_index];
            diagnostics.push(
                error_diagnostic(
                    DUPLICATE_PATH_CODE,
                    object,
                    identity_path_path(),
                    "logical object path is declared more than once",
                )
                .with_context("uuid", &object.identity().uuid().to_string())
                .expect("canonical UUID context is bounded"),
            );
        }
    }
}

fn validate_owners_and_references(
    configuration: &CanonicalConfiguration,
    object_uuids: &BTreeSet<ObjectUuid>,
    reference_targets: &BTreeSet<ObjectUuid>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for object in configuration.objects() {
        if let Some(owner) = object.owner()
            && !object_uuids.contains(&owner)
        {
            diagnostics.push(
                error_diagnostic(
                    DANGLING_OWNER_CODE,
                    object,
                    named_property_path("owner"),
                    "owner UUID does not identify a canonical object",
                )
                .with_context("target_uuid", &owner.to_string())
                .expect("canonical UUID context is bounded"),
            );
        }

        for (reference_index, reference) in object.references().iter().enumerate() {
            if reference_targets.contains(&reference.target()) {
                continue;
            }
            diagnostics.push(
                error_diagnostic(
                    DANGLING_REFERENCE_CODE,
                    object,
                    indexed_property_path("references", reference_index, "target"),
                    "reference target UUID is absent from objects and generated types",
                )
                .with_context("reference_kind", reference.kind().as_str())
                .expect("validated reference kind context is bounded")
                .with_context("target_uuid", &reference.target().to_string())
                .expect("canonical UUID context is bounded"),
            );
        }
    }
}

fn validate_ownership_cycles(
    configuration: &CanonicalConfiguration,
    uuid_occurrences: &BTreeMap<ObjectUuid, Vec<UuidOccurrence>>,
    object_uuids: &BTreeSet<ObjectUuid>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut unique_object_index = BTreeMap::<ObjectUuid, usize>::new();
    for (uuid, locations) in uuid_occurrences {
        if locations.len() == 1
            && let UuidOccurrence::Object { object_index } = locations[0]
        {
            unique_object_index.insert(*uuid, object_index);
        }
    }

    let mut owner_by_uuid = BTreeMap::<ObjectUuid, ObjectUuid>::new();
    for (uuid, object_index) in &unique_object_index {
        if let Some(owner) = configuration.objects()[*object_index].owner()
            && object_uuids.contains(&owner)
            && unique_object_index.contains_key(&owner)
        {
            owner_by_uuid.insert(*uuid, owner);
        }
    }

    let mut completed = BTreeSet::new();
    let mut cycles = Vec::<Vec<ObjectUuid>>::new();
    for start in unique_object_index.keys().copied() {
        if completed.contains(&start) {
            continue;
        }
        let mut path = Vec::new();
        let mut positions = BTreeMap::<ObjectUuid, usize>::new();
        let mut current = start;
        loop {
            if completed.contains(&current) {
                break;
            }
            if let Some(position) = positions.get(&current).copied() {
                cycles.push(path[position..].to_vec());
                break;
            }
            positions.insert(current, path.len());
            path.push(current);
            let Some(owner) = owner_by_uuid.get(&current).copied() else {
                break;
            };
            current = owner;
        }
        completed.extend(path);
    }

    for cycle in cycles {
        for uuid in cycle {
            let object = &configuration.objects()[unique_object_index[&uuid]];
            let owner = object
                .owner()
                .expect("cycle member has an owner edge to another object");
            diagnostics.push(
                error_diagnostic(
                    OWNERSHIP_CYCLE_CODE,
                    object,
                    named_property_path("owner"),
                    "ownership edge participates in a cycle",
                )
                .with_context("owner_uuid", &owner.to_string())
                .expect("canonical UUID context is bounded")
                .with_context("uuid", &uuid.to_string())
                .expect("canonical UUID context is bounded"),
            );
        }
    }
}

fn error_diagnostic(
    code: &'static str,
    object: &CanonicalObject,
    property_path: PropertyPath,
    message: &'static str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::new(code).expect("static model diagnostic code is valid"),
        Severity::Error,
        object.identity().path().clone(),
        property_path,
        message,
    )
    .expect("static model diagnostic message is bounded")
    .with_profiles(Some(object.provenance().source_profile().clone()), None)
}

fn named_segment(value: &'static str) -> PathSegment {
    PathSegment::name(value).expect("static model property segment is valid")
}

fn identity_uuid_path() -> PropertyPath {
    PropertyPath::new(vec![named_segment("identity"), named_segment("uuid")])
        .expect("static model property path is bounded")
}

fn identity_path_path() -> PropertyPath {
    PropertyPath::new(vec![named_segment("identity"), named_segment("path")])
        .expect("static model property path is bounded")
}

fn named_property_path(name: &'static str) -> PropertyPath {
    PropertyPath::new(vec![named_segment(name)]).expect("static model property path is bounded")
}

fn indexed_property_path(
    collection: &'static str,
    index: usize,
    field: &'static str,
) -> PropertyPath {
    let index = u32::try_from(index).expect("bounded model collection index fits into u32");
    PropertyPath::new(vec![
        named_segment(collection),
        PathSegment::index(index),
        named_segment(field),
    ])
    .expect("static indexed model property path is bounded")
}

#[cfg(test)]
mod tests {
    use crate::artifact::ProfileId;
    use crate::diagnostic::PathSegment;
    use crate::identity::LogicalIdentity;
    use crate::model::{
        CanonicalObject, CanonicalObjectParts, GeneratedType, GeneratedTypeKind, MetadataKind,
        ObjectReference, ReferenceKind,
    };
    use crate::provenance::{CanonicalAnchor, SourceProvenance};

    use super::*;

    fn uuid(suffix: u32) -> ObjectUuid {
        ObjectUuid::parse(&format!("00000000-0000-0000-0000-{suffix:012x}")).unwrap()
    }

    fn path(name: &str) -> ObjectPath {
        ObjectPath::new(vec![
            PathSegment::name("objects").unwrap(),
            PathSegment::name(name).unwrap(),
        ])
        .unwrap()
    }

    fn parts(id: u32, name: &str) -> CanonicalObjectParts {
        let object_path = path(name);
        CanonicalObjectParts::new(
            LogicalIdentity::new(uuid(id), object_path.clone()),
            MetadataKind::new("Catalog").unwrap(),
            SourceProvenance::new(
                ProfileId::parse("profile:source").unwrap(),
                CanonicalAnchor::new(object_path, PropertyPath::root()),
            ),
        )
    }

    fn object(parts: CanonicalObjectParts) -> CanonicalObject {
        CanonicalObject::new(parts).unwrap()
    }

    fn codes(report: &DiagnosticReport) -> Vec<&str> {
        report
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code().as_str())
            .collect()
    }

    #[test]
    fn duplicate_uuid_and_path_diagnostics_are_stable_and_addressed() {
        let first = parts(1, "same");
        let mut second = parts(1, "same");
        second.generated_types.push(GeneratedType::new(
            uuid(1),
            GeneratedTypeKind::new("ObjectType").unwrap(),
        ));
        let configuration =
            CanonicalConfiguration::new(vec![object(first), object(second)]).unwrap();
        let report = validate_configuration(&configuration).unwrap_err();
        assert!(codes(&report).contains(&DUPLICATE_UUID_CODE));
        assert!(codes(&report).contains(&DUPLICATE_PATH_CODE));
        let duplicate = report
            .diagnostics()
            .iter()
            .find(|diagnostic| {
                diagnostic.code().as_str() == DUPLICATE_UUID_CODE
                    && diagnostic.property_path().to_string()
                        == "$/name:generated_types/index:0/name:uuid"
            })
            .unwrap();
        assert_eq!(duplicate.object_path(), &path("same"));
        assert_eq!(duplicate.severity(), Severity::Error);
        assert_eq!(
            duplicate.source_profile().map(|profile| profile.as_str()),
            Some("profile:source")
        );
        assert_eq!(duplicate.target_profile(), None);
        assert_eq!(duplicate.context()["uuid"], uuid(1).to_string());
    }

    #[test]
    fn dangling_owner_and_reference_have_exact_property_paths() {
        let mut invalid = parts(1, "invalid");
        invalid.owner = Some(uuid(99));
        invalid.references.push(ObjectReference::new(
            ReferenceKind::new("metadata:link").unwrap(),
            uuid(98),
        ));
        let configuration = CanonicalConfiguration::new(vec![object(invalid)]).unwrap();
        let report = validate_configuration(&configuration).unwrap_err();
        let owner = report
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code().as_str() == DANGLING_OWNER_CODE)
            .unwrap();
        assert_eq!(owner.property_path().to_string(), "$/name:owner");
        assert_eq!(owner.context()["target_uuid"], uuid(99).to_string());
        let reference = report
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code().as_str() == DANGLING_REFERENCE_CODE)
            .unwrap();
        assert_eq!(
            reference.property_path().to_string(),
            "$/name:references/index:0/name:target"
        );
        assert_eq!(reference.context()["target_uuid"], uuid(98).to_string());
    }

    #[test]
    fn generated_type_is_a_valid_reference_target_but_not_an_owner() {
        let mut target = parts(1, "target");
        target.generated_types.push(GeneratedType::new(
            uuid(101),
            GeneratedTypeKind::new("ObjectType").unwrap(),
        ));
        let mut source = parts(2, "source");
        source.references.push(ObjectReference::new(
            ReferenceKind::new("type").unwrap(),
            uuid(101),
        ));
        let configuration =
            CanonicalConfiguration::new(vec![object(source), object(target)]).unwrap();
        let validated = validate_configuration(&configuration).unwrap();
        assert!(validated.graph().contains_reference_target(uuid(101)));

        let mut invalid_owner = parts(3, "bad-owner");
        invalid_owner.owner = Some(uuid(101));
        let configuration =
            CanonicalConfiguration::new(vec![object(parts(1, "target")), object(invalid_owner)])
                .unwrap();
        let report = validate_configuration(&configuration).unwrap_err();
        assert!(codes(&report).contains(&DANGLING_OWNER_CODE));
    }

    #[test]
    fn self_and_multi_node_ownership_cycles_are_detected_iteratively() {
        let mut self_owned = parts(1, "self");
        self_owned.owner = Some(uuid(1));
        let self_configuration = CanonicalConfiguration::new(vec![object(self_owned)]).unwrap();
        let self_report = validate_configuration(&self_configuration).unwrap_err();
        assert_eq!(codes(&self_report), vec![OWNERSHIP_CYCLE_CODE]);

        let mut first = parts(1, "first");
        first.owner = Some(uuid(2));
        let mut second = parts(2, "second");
        second.owner = Some(uuid(3));
        let mut third = parts(3, "third");
        third.owner = Some(uuid(1));
        let configuration =
            CanonicalConfiguration::new(vec![object(third), object(first), object(second)])
                .unwrap();
        let report = validate_configuration(&configuration).unwrap_err();
        assert_eq!(
            codes(&report),
            vec![
                OWNERSHIP_CYCLE_CODE,
                OWNERSHIP_CYCLE_CODE,
                OWNERSHIP_CYCLE_CODE
            ]
        );
        assert!(
            report
                .diagnostics()
                .iter()
                .all(|diagnostic| { diagnostic.property_path().to_string() == "$/name:owner" })
        );
    }

    #[test]
    fn only_a_valid_model_receives_graph_and_encode_ready_token() {
        let mut child = parts(2, "child");
        child.owner = Some(uuid(1));
        child.references.push(ObjectReference::new(
            ReferenceKind::new("parent").unwrap(),
            uuid(1),
        ));
        let configuration =
            CanonicalConfiguration::new(vec![object(child), object(parts(1, "parent"))]).unwrap();
        let validated = validate_configuration(&configuration).unwrap();
        assert_eq!(validated.graph().object_index_by_uuid(uuid(2)), Some(0));
        assert_eq!(validated.configuration(), &configuration);
    }
}
