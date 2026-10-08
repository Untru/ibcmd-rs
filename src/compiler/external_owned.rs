//! Complete source ownership, not native body compilation or EPF publication.
use super::{
    artifact::ArtifactScope,
    bodies::template::TemplateKind,
    external_intake::{ExternalAssetClaim, ExternalIntakeError, admit_root, input, read_snapshot},
    families::assets::{SourceAssetRegistry, SourceAssetRole, SourceAssetRoute},
    graph::{BootstrapGraph, ObjectStorageRoute, StorageSuffix, build_artifact_graph},
    identity::{BootstrapIdentities, collect_artifact_identities},
};
use ibcmd_core::{
    artifact::ProfileId,
    asset::{AssetReference, MediaKind},
    diagnostic::{ObjectPath, PathSegment},
    identity::ObjectUuid,
    model::{CanonicalConfiguration, CanonicalObject, CanonicalObjectParts},
    source_policy::SourceOperationPolicy,
    validate::validate_configuration,
    value::CanonicalValueKind,
};
use ibcmd_schema::external_named::ExternalNamedKind;
use ibcmd_xml::{
    XmlReader,
    metadata::{
        ExternalNamedOwnerContext, MetadataEnvelope, decode_external_named_owner,
        encode_external_named_owner, encode_external_root, validate_external_owned_body,
    },
    source_tree::{ReaderLimits, SourceEntry, SourcePath, SourceTree},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// Routing facts only. The canonical object lives once in `configuration`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalNamedOwner {
    metadata_path: SourcePath,
    kind: ExternalNamedKind,
    declared_name: String,
    uuid: ObjectUuid,
}
impl ExternalNamedOwner {
    pub fn metadata_path(&self) -> &SourcePath {
        &self.metadata_path
    }
    pub const fn kind(&self) -> ExternalNamedKind {
        self.kind
    }
    pub fn declared_name(&self) -> &str {
        &self.declared_name
    }
    pub const fn uuid(&self) -> ObjectUuid {
        self.uuid
    }
}
/// A closed retained root/named metadata/body inventory. Ownership is distinct
/// from semantic body interpretation and cannot authorize native publication.
#[derive(Clone, Debug)]
pub struct ExternalOwnedSource {
    tree: SourceTree,
    root_path: SourcePath,
    envelope: MetadataEnvelope,
    configuration: CanonicalConfiguration,
    owners: Vec<ExternalNamedOwner>,
    claims: Vec<ExternalAssetClaim>,
    identities: BootstrapIdentities,
    graph: BootstrapGraph,
}
impl ExternalOwnedSource {
    pub fn read(
        source: impl AsRef<Path>,
        profile: ProfileId,
        path: ObjectPath,
        limits: ReaderLimits,
    ) -> Result<Self, ExternalIntakeError> {
        let (tree, selected) = read_snapshot(source.as_ref(), limits)?;
        Self::from_tree(tree, selected.as_ref(), profile, path)
    }
    pub fn from_tree(
        tree: SourceTree,
        selected: Option<&SourcePath>,
        profile: ProfileId,
        path: ObjectPath,
    ) -> Result<Self, ExternalIntakeError> {
        let snapshot = admit_root(&tree, selected, profile.clone(), path)?;
        let root_path = tree.entries()[snapshot.index].path().clone();
        let envelope = snapshot.envelope;
        let context = ExternalNamedOwnerContext::from_root(&envelope)
            .map_err(|e| input(root_path.as_str(), e))?;
        let scope = ArtifactScope::from_source_identity(snapshot.identity)
            .map_err(|e| input(root_path.as_str(), e))?;
        let registry = SourceAssetRegistry;
        let mut consumed = BTreeSet::from([root_path.clone()]);
        let mut owners = Vec::new();
        let mut objects = vec![envelope.root().clone()];
        let mut claims = Vec::new();
        let root_route = registry
            .route(
                envelope.root().kind().as_str(),
                SourceAssetRole::ObjectModule,
            )
            .ok_or_else(|| input(root_path.as_str(), "root ObjectModule route"))?;
        let root_module = owner_path(&root_path, root_route.relative_path())?;
        claim(
            &tree,
            &root_module,
            context.owner(),
            root_route,
            "text/x-1c-bsl",
            false,
            (&mut consumed, &mut claims),
        )?;
        for kind in [ExternalNamedKind::Form, ExternalNamedKind::Template] {
            for name in context.declared_names(kind) {
                let metadata_path =
                    owner_path(&root_path, &format!("{}/{name}.xml", kind.collection()))?;
                let entry = exact_entry(&tree, &metadata_path)?;
                if !consumed.insert(metadata_path.clone()) {
                    return Err(input(metadata_path.as_str(), "duplicate metadata owner"));
                }
                let document = XmlReader::from_slice(entry.bytes())
                    .map_err(|e| input(metadata_path.as_str(), e))?;
                let mut child_path = envelope.root().identity().path().clone();
                for segment in [kind.family(), name] {
                    child_path
                        .push_with_policy(
                            PathSegment::name_with_policy(segment, SourceOperationPolicy::Source)
                                .map_err(|e| input(metadata_path.as_str(), e))?,
                            SourceOperationPolicy::Source,
                        )
                        .map_err(|e| input(metadata_path.as_str(), e))?;
                }
                let object = decode_external_named_owner(
                    &document,
                    profile.clone(),
                    child_path,
                    &context,
                    kind,
                    name,
                )
                .map_err(|e| input(metadata_path.as_str(), e))?;
                let source_kind = match kind {
                    ExternalNamedKind::Form => "Form",
                    ExternalNamedKind::Template => object
                        .properties()
                        .iter()
                        .find(|f| f.name().as_str() == "TemplateType")
                        .and_then(|f| match f.value().kind() {
                            CanonicalValueKind::EnumToken(t) => Some(t.as_str()),
                            _ => None,
                        })
                        .ok_or_else(|| input(metadata_path.as_str(), "TemplateType"))?,
                };
                let source_file = if kind == ExternalNamedKind::Form {
                    "Form.xml"
                } else {
                    let template = TemplateKind::parse(source_kind)
                        .map_err(|e| input(metadata_path.as_str(), e))?;
                    if template == TemplateKind::HtmlDocument {
                        return Err(ExternalIntakeError::Unsupported {
                            path: metadata_path.to_string(),
                            coordinate: "HTML bundle ownership",
                        });
                    }
                    template.source_file()
                };
                let body_route = registry
                    .external_named_body_route(kind.family(), source_file)
                    .ok_or_else(|| input(metadata_path.as_str(), "named body route"))?;
                let body_path = owner_path(&metadata_path, body_route.relative_path())?;
                let body = exact_entry(&tree, &body_path)?;
                let media = if source_file.ends_with(".xml") {
                    let document = XmlReader::from_slice(body.bytes())
                        .map_err(|e| input(body_path.as_str(), e))?;
                    validate_external_owned_body(
                        &document,
                        source_kind,
                        &profile,
                        object.identity().path(),
                    )
                    .map_err(|e| input(body_path.as_str(), e))?;
                    "application/xml"
                } else if source_file.ends_with(".txt") {
                    "text/plain"
                } else {
                    "application/octet-stream"
                };
                let first = claims.len();
                claim(
                    &tree,
                    &body_path,
                    object.identity().uuid(),
                    body_route,
                    media,
                    true,
                    (&mut consumed, &mut claims),
                )?;
                if kind == ExternalNamedKind::Form {
                    let route = registry
                        .route("Form", SourceAssetRole::FormModule)
                        .ok_or_else(|| input(metadata_path.as_str(), "Form module route"))?;
                    let module = owner_path(&metadata_path, route.relative_path())?;
                    claim(
                        &tree,
                        &module,
                        object.identity().uuid(),
                        route,
                        "text/x-1c-bsl",
                        false,
                        (&mut consumed, &mut claims),
                    )?;
                }
                let mut parts = parts(&object);
                parts.assets = claims[first..].iter().map(|c| c.content.clone()).collect();
                let object = CanonicalObject::new_with_policy(parts, SourceOperationPolicy::Source)
                    .map_err(|e| input(metadata_path.as_str(), e))?;
                owners.push(ExternalNamedOwner {
                    metadata_path,
                    kind,
                    declared_name: name.to_owned(),
                    uuid: object.identity().uuid(),
                });
                objects.push(object);
            }
        }
        for entry in tree.entries() {
            if !consumed.contains(entry.path()) {
                return Err(ExternalIntakeError::Unconsumed {
                    path: entry.path().to_string(),
                });
            }
        }
        let configuration =
            CanonicalConfiguration::new_with_policy(objects, SourceOperationPolicy::Source)
                .map_err(|e| input(root_path.as_str(), e))?;
        let validated = validate_configuration(&configuration)
            .map_err(|e| input(root_path.as_str(), format!("{e:?}")))?;
        let identities = collect_artifact_identities(&validated, scope)
            .map_err(|e| input(root_path.as_str(), e))?;
        let mut routes = Vec::new();
        for object in configuration.objects() {
            // Form body+module are distinct claims in the same physical bundle.
            let suffixes = claims
                .iter()
                .filter(|c| c.owner == object.identity().uuid())
                .map(|c| c.route.suffix())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|s| StorageSuffix::new(s).map_err(|e| input(root_path.as_str(), e)))
                .collect::<Result<Vec<_>, _>>()?;
            routes.push(
                ObjectStorageRoute::new(object.identity().uuid(), suffixes)
                    .map_err(|e| input(root_path.as_str(), e))?,
            );
        }
        let graph = build_artifact_graph(&identities, profile, routes)
            .map_err(|e| input(root_path.as_str(), e))?;
        Ok(Self {
            tree,
            root_path,
            envelope,
            configuration,
            owners,
            claims,
            identities,
            graph,
        })
    }
    pub fn tree(&self) -> &SourceTree {
        &self.tree
    }
    pub fn root_path(&self) -> &SourcePath {
        &self.root_path
    }
    pub fn envelope(&self) -> &MetadataEnvelope {
        &self.envelope
    }
    pub fn configuration(&self) -> &CanonicalConfiguration {
        &self.configuration
    }
    pub fn named_owners(&self) -> &[ExternalNamedOwner] {
        &self.owners
    }
    pub fn claims(&self) -> &[ExternalAssetClaim] {
        &self.claims
    }
    pub fn identities(&self) -> &BootstrapIdentities {
        &self.identities
    }
    pub fn graph(&self) -> &BootstrapGraph {
        &self.graph
    }

    /// Prepares a full CURRENT inverse without IO, publication or topology
    /// mutation. Desired asset references must describe the actual new bytes.
    pub fn with_current(
        &self,
        root: &MetadataEnvelope,
        current: &CanonicalConfiguration,
        asset_edits: &[SourceEntry],
    ) -> Result<Self, ExternalIntakeError> {
        let profile = self.envelope.root().provenance().source_profile();
        if root.external_source_binding() != self.envelope.external_source_binding()
            || root.source_document() != self.envelope.source_document()
            || root.root().identity().path() != self.envelope.root().identity().path()
            || current.objects().len() != self.configuration.objects().len()
            || current.objects().first() != Some(root.root())
        {
            return Err(ExternalIntakeError::CurrentMismatch {
                coordinate: "root CURRENT binding/source/graph",
            });
        }
        let validated = validate_configuration(current)
            .map_err(|e| input(self.root_path.as_str(), format!("{e:?}")))?;
        let ids = collect_artifact_identities(&validated, self.identities.scope())
            .map_err(|e| input(self.root_path.as_str(), e))?;
        if ids != self.identities {
            return Err(ExternalIntakeError::CurrentMismatch {
                coordinate: "CURRENT identity/topology",
            });
        }
        let context = ExternalNamedOwnerContext::from_root(&self.envelope)
            .map_err(|e| input(self.root_path.as_str(), e))?;
        let mut replacements = BTreeMap::new();
        replacements.insert(
            self.root_path.clone(),
            SourceEntry::from_bytes(
                self.root_path.clone(),
                encode_external_root(root, profile)
                    .map_err(|e| input(self.root_path.as_str(), e))?,
            )
            .map_err(|e| input(self.root_path.as_str(), e))?,
        );
        for owner in &self.owners {
            let desired = current
                .objects()
                .iter()
                .find(|o| o.identity().uuid() == owner.uuid)
                .ok_or_else(|| input(owner.metadata_path.as_str(), "missing CURRENT owner"))?;
            let mut metadata = parts(desired);
            metadata.assets.clear();
            let metadata =
                CanonicalObject::new_with_policy(metadata, SourceOperationPolicy::Source)
                    .map_err(|e| input(owner.metadata_path.as_str(), e))?;
            let original = exact_entry(&self.tree, &owner.metadata_path)?;
            let document = XmlReader::from_slice(original.bytes())
                .map_err(|e| input(owner.metadata_path.as_str(), e))?;
            let bytes = encode_external_named_owner(
                &document,
                &metadata,
                profile,
                &context,
                owner.kind,
                &owner.declared_name,
            )
            .map_err(|e| input(owner.metadata_path.as_str(), e))?;
            replacements.insert(
                owner.metadata_path.clone(),
                SourceEntry::from_bytes(owner.metadata_path.clone(), bytes)
                    .map_err(|e| input(owner.metadata_path.as_str(), e))?,
            );
        }
        for edit in asset_edits {
            if !self.claims.iter().any(|c| &c.source_path == edit.path())
                || replacements
                    .insert(edit.path().clone(), edit.clone())
                    .is_some()
            {
                return Err(ExternalIntakeError::CurrentMismatch {
                    coordinate: "unknown/duplicate CURRENT asset",
                });
            }
        }
        let entries = self
            .tree
            .entries()
            .iter()
            .map(|e| replacements.remove(e.path()).unwrap_or_else(|| e.clone()))
            .collect();
        let tree = SourceTree::new(entries).map_err(|e| input("CURRENT owned snapshot", e))?;
        let prepared = Self::from_tree(
            tree,
            Some(&self.root_path),
            profile.clone(),
            root.root().identity().path().clone(),
        )?;
        if prepared.configuration != *current
            || prepared.identities != self.identities
            || prepared.owners != self.owners
            || prepared.envelope.external_source_binding() != root.external_source_binding()
        {
            return Err(ExternalIntakeError::CurrentMismatch {
                coordinate: "whole CURRENT forward canonical/binding/assets equality",
            });
        }
        Ok(prepared)
    }
}
fn parts(object: &CanonicalObject) -> CanonicalObjectParts {
    let mut parts = CanonicalObjectParts::new(
        object.identity().clone(),
        object.kind().clone(),
        object.provenance().clone(),
    );
    parts.owner = object.owner();
    parts.properties = object.properties().to_vec();
    parts.references = object.references().to_vec();
    parts.generated_types = object.generated_types().to_vec();
    parts.assets = object.assets().to_vec();
    parts.opaque_facets = object.opaque_facets().clone();
    parts
}
fn owner_path(metadata: &SourcePath, relative: &str) -> Result<SourcePath, ExternalIntakeError> {
    let path =
        SourceAssetRegistry.external_owner_relative_path(Path::new(metadata.as_str()), relative);
    let path = path
        .to_str()
        .ok_or_else(|| input(metadata.as_str(), "non-UTF8 owner path"))?
        .replace('\\', "/");
    SourcePath::new(&path).map_err(|e| input(metadata.as_str(), e))
}
fn exact_entry<'a>(
    tree: &'a SourceTree,
    path: &SourcePath,
) -> Result<&'a SourceEntry, ExternalIntakeError> {
    tree.entries()
        .iter()
        .find(|e| e.path() == path)
        .ok_or_else(|| input(path.as_str(), "missing mandatory owner metadata/body"))
}
fn claim(
    tree: &SourceTree,
    path: &SourcePath,
    owner: ObjectUuid,
    route: &'static SourceAssetRoute,
    media: &str,
    mandatory: bool,
    accumulated: (&mut BTreeSet<SourcePath>, &mut Vec<ExternalAssetClaim>),
) -> Result<(), ExternalIntakeError> {
    let (consumed, claims) = accumulated;
    let Some(entry) = tree.entries().iter().find(|e| e.path() == path) else {
        return if mandatory {
            Err(input(path.as_str(), "missing mandatory body"))
        } else {
            Ok(())
        };
    };
    if !consumed.insert(path.clone()) {
        return Err(input(path.as_str(), "duplicate source claim"));
    }
    if media.starts_with("text/") {
        std::str::from_utf8(entry.bytes()).map_err(|e| input(path.as_str(), e))?;
    }
    let size = u64::try_from(entry.bytes().len()).map_err(|e| input(path.as_str(), e))?;
    let content = AssetReference::new(
        entry.digest(),
        size,
        MediaKind::new(media).map_err(|e| input(path.as_str(), e))?,
    )
    .map_err(|e| input(path.as_str(), e))?;
    content
        .verify_bytes(entry.bytes())
        .map_err(|e| input(path.as_str(), e))?;
    claims.push(ExternalAssetClaim {
        source_path: path.clone(),
        owner,
        route,
        content,
    });
    Ok(())
}
