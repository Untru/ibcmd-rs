//! Base-free native compiler for managed `Form` metadata records.
//!
//! # What a form is, physically
//!
//! A managed form occupies two storage records, not one:
//!
//! * `<uuid>` — the *metadata* record compiled here, projected from
//!   `.../Forms/<Name>.xml`;
//! * `<uuid>.0` — the *body*, projected from `.../Forms/<Name>/Ext/Form.xml`
//!   plus `.../Forms/<Name>/Ext/Form/Module.bsl` and compiled by the existing
//!   base-free packer in [`crate::compiler::bodies::form`].
//!
//! That split is read straight off our own `cf export` of the retained native
//! corpora: the JSON report maps storage key `c099c30b-…` to the single output
//! `Catalogs/CorpusList/Forms/ListForm.xml`, and key `c099c30b-….0` to the two
//! outputs `…/Ext/Form/Module.bsl` and `…/Ext/Form.xml`.  This module owns only
//! the first of the two; it does not reimplement the body packer.
//!
//! # Why the tail of the record is one evidenced cohort, not a field map
//!
//! All 13 `Form` metadata records retained across the bundled
//! `8.3.27.2214` corpora are byte-identical modulo uuid, name, synonym and
//! comment:
//!
//! ```text
//! {1,
//! {0,
//! {13,
//! {3,
//! {1,0,<uuid>},"<Name>",
//! {1,"ru","<Synonym>"},"<Comment>",0,0,00000000-0000-0000-0000-000000000000,0},0,1,
//! {2,
//! {"#",1708fdaa-cbce-4289-b373-07a5a74bee91,1},
//! {"#",1708fdaa-cbce-4289-b373-07a5a74bee91,2}
//! }
//! }
//! },0}
//! ```
//!
//! and every matching `Forms/<Name>.xml` carries exactly
//! `FormType=Managed`, `IncludeHelpInContents=false` and
//! `UsePurposes=[PlatformApplication, MobilePlatformApplication]`.
//!
//! Zero variance means the two scalar slots after the metadata header (`0`,
//! `1`) cannot be attributed to individual XML properties — nothing in the
//! evidence moves one without moving the other.  Guessing an attribution and
//! then encoding a *different* property combination would emit bytes no
//! platform capture supports.  So this compiler treats the observed
//! combination as a single named cohort: it reproduces the evidenced tail
//! exactly, and refuses any other property combination with the offending
//! property, what was expected, and what was found.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::{ProfileId, StorageProfileId};
use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
use ibcmd_core::identity::{LogicalIdentity, ObjectUuid};
use ibcmd_core::model::{CanonicalObject, CanonicalObjectParts, MetadataKind};
use ibcmd_core::profile::EffectiveProfile;
use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
use ibcmd_core::storage::{
    MultipartIdentity, StorageBuildError, StoragePatchBuildError, StoragePatchEntry,
    StoragePatchOutcome, StoragePatchTarget, StorageProvenance,
};
use ibcmd_core::validate::ValidatedConfiguration;
use ibcmd_core::value::{CanonicalField, CanonicalText, CanonicalValue, CanonicalValueKind};
use ibcmd_core::version::PlatformBuild;
use ibcmd_xml::{AttributeKind, XmlDocument, XmlElement, XmlNode};

use super::super::CompileAxes;
use super::super::graph::BootstrapGraph;
use super::native::{
    NativeMetadataHeader, NativeValue, exact_list, exact_token, inflate_and_parse, inline_list,
    metadata_header, parse_metadata_header, raw_deflate, serialize, styled_list,
    styled_list_with_tail, text, token,
};

const LAYOUT_KEY: &str = "bootstrap.metadata.form.layout";
const LAYOUT: &str = "form-v13-managed-platform-mobile-v1-crlf-utf8-bom";
const SUPPORTED_STORAGE_PROFILE: &str = "storage:mssql-config-configsave";

/// Open metadata family this module compiles.
pub const FORM_FAMILY: &str = "Form";

const MD_NAMESPACE: &str = "http://v8.1c.ru/8.3/MDClasses";
const V8_NAMESPACE: &str = "http://v8.1c.ru/8.1/data/core";
const XSI_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema-instance";
const APP_NAMESPACE: &str = "http://v8.1c.ru/8.2/managed-application/core";

/// Exact ordered `Properties` schema of every retained `Forms/<Name>.xml`.
const PROPERTY_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "FormType",
    "IncludeHelpInContents",
    "UsePurposes",
];

/// Evidenced `FormType` token.
const EVIDENCED_FORM_TYPE: &str = "Managed";
/// Evidenced `IncludeHelpInContents` value.
const EVIDENCED_INCLUDE_HELP_IN_CONTENTS: bool = false;
/// Evidenced `UsePurposes` sequence, in document order.
const EVIDENCED_USE_PURPOSES: &[&str] = &["PlatformApplication", "MobilePlatformApplication"];
/// `xsi:type` local name every retained `UsePurposes` value carries.
const USE_PURPOSE_XSI_TYPE: &str = "ApplicationUsePurpose";
/// Native type uuid of `app:ApplicationUsePurpose`.
const USE_PURPOSE_TYPE_UUID: &str = "1708fdaa-cbce-4289-b373-07a5a74bee91";
/// Native `Form` object marker.
const FORM_OBJECT_MARKER: &str = "13";

/// Exact platform/storage layout selected for one `Form` metadata compilation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormMetadataProfile {
    profile_id: ProfileId,
    platform_build: PlatformBuild,
    storage_profile: StorageProfileId,
}

impl FormMetadataProfile {
    /// Selects the `Form` metadata layout without deriving one axis from another.
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, FormMetadataProfileError> {
        let platform_build = profile
            .platform_build
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| FormMetadataProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "platform_build",
            })?;
        let storage_profile = profile
            .storage_profile
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| FormMetadataProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
            })?;
        if storage_profile.as_str() != SUPPORTED_STORAGE_PROFILE {
            return Err(FormMetadataProfileError::UnsupportedCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
                value: storage_profile.to_string(),
            });
        }
        let layout = profile.constants.get(LAYOUT_KEY).ok_or_else(|| {
            FormMetadataProfileError::MissingConstant {
                profile: profile.id.clone(),
                key: LAYOUT_KEY,
            }
        })?;
        if layout.value != LAYOUT {
            return Err(FormMetadataProfileError::UnsupportedLayout {
                profile: profile.id.clone(),
                value: layout.value.clone(),
            });
        }
        Ok(Self {
            profile_id: profile.id.clone(),
            platform_build,
            storage_profile,
        })
    }

    /// Returns the exact selected profile identity.
    pub const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    #[cfg(test)]
    fn fixture() -> Self {
        Self {
            profile_id: ProfileId::parse("platform-8.3.27.1989").unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormMetadataProfileError {
    MissingCoordinate {
        profile: ProfileId,
        coordinate: &'static str,
    },
    MissingConstant {
        profile: ProfileId,
        key: &'static str,
    },
    UnsupportedCoordinate {
        profile: ProfileId,
        coordinate: &'static str,
        value: String,
    },
    UnsupportedLayout {
        profile: ProfileId,
        value: String,
    },
}

impl Display for FormMetadataProfileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCoordinate {
                profile,
                coordinate,
            } => write!(
                formatter,
                "profile `{profile}` has no `{coordinate}` coordinate"
            ),
            Self::MissingConstant { profile, key } => {
                write!(formatter, "profile `{profile}` has no `{key}` constant")
            }
            Self::UnsupportedCoordinate {
                profile,
                coordinate,
                value,
            } => write!(
                formatter,
                "profile `{profile}` declares unsupported `{coordinate}` value `{value}`"
            ),
            Self::UnsupportedLayout { profile, value } => write!(
                formatter,
                "profile `{profile}` declares unsupported `{LAYOUT_KEY}={value}`"
            ),
        }
    }
}

impl Error for FormMetadataProfileError {}

/// The single evidenced combination of the property-driven tail of a `Form`
/// metadata record.  New variants may only be added together with retained
/// platform bytes that exhibit them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormEvidencedCohort {
    /// `FormType=Managed`, `IncludeHelpInContents=false`,
    /// `UsePurposes=[PlatformApplication, MobilePlatformApplication]`.
    ManagedPlatformAndMobile,
}

/// Strict native intermediate representation of one `Form` metadata record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormMetadataNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<(String, String)>,
    pub comment: String,
    pub cohort: FormEvidencedCohort,
}

/// Compiles one validated `Form` object into its `<uuid>` storage row.
pub fn compile_form_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &FormMetadataProfile,
) -> Result<StoragePatchEntry, FormMetadataBuildError> {
    validate_coordinates(graph, axes, profile)?;
    let object_index = validated
        .graph()
        .object_index_by_uuid(object_uuid)
        .ok_or(FormMetadataBuildError::UnknownObject(object_uuid))?;
    let object = &validated.configuration().objects()[object_index];
    let ir = project_object(object)?;
    let route = graph
        .primary_object_entry(object_uuid)
        .ok_or(FormMetadataBuildError::MissingPrimaryRoute(object_uuid))?;
    let bytes = encode_form_metadata_blob(&ir, profile)?;
    let provenance =
        StorageProvenance::new(&format!("bootstrap:{}:metadata:Form", profile.profile_id))?;
    Ok(StoragePatchEntry::new(
        StoragePatchTarget::new(route.key().clone(), MultipartIdentity::single(), provenance),
        StoragePatchOutcome::compiled(bytes)?,
    ))
}

/// Encodes one `Form` metadata record into its exact raw-DEFLATE payload.
pub fn encode_form_metadata_blob(
    value: &FormMetadataNativeIr,
    _profile: &FormMetadataProfile,
) -> Result<Vec<u8>, FormMetadataBuildError> {
    raw_deflate(&build_native(value)).map_err(native_error)
}

/// Renders the exact UTF-8 plaintext behind [`encode_form_metadata_blob`].
pub fn form_metadata_plaintext(
    value: &FormMetadataNativeIr,
    _profile: &FormMetadataProfile,
) -> Result<Vec<u8>, FormMetadataBuildError> {
    serialize(&build_native(value)).map_err(native_error)
}

/// Strictly decodes an evidenced `Form` metadata record back into IR.
pub fn decode_form_metadata_blob(
    blob: &[u8],
    _profile: &FormMetadataProfile,
) -> Result<FormMetadataNativeIr, FormMetadataBuildError> {
    let root = inflate_and_parse(blob).map_err(native_error)?;
    parse_native(&root)
}

fn build_native(value: &FormMetadataNativeIr) -> NativeValue {
    let FormEvidencedCohort::ManagedPlatformAndMobile = value.cohort;
    styled_list(
        vec![
            token("1"),
            styled_list_with_tail(
                vec![
                    token("0"),
                    styled_list_with_tail(
                        vec![
                            token(FORM_OBJECT_MARKER),
                            metadata_header(&NativeMetadataHeader {
                                uuid: value.uuid,
                                name: value.name.clone(),
                                synonyms: value.synonyms.clone(),
                                comment: value.comment.clone(),
                            }),
                            token("0"),
                            token("1"),
                            styled_list_with_tail(
                                vec![token("2"), use_purpose(1), use_purpose(2)],
                                vec![1, 2],
                            ),
                        ],
                        vec![1, 4],
                    ),
                ],
                vec![1],
            ),
            token("0"),
        ],
        vec![1],
    )
}

fn use_purpose(ordinal: u32) -> NativeValue {
    inline_list(vec![
        text("#"),
        token(USE_PURPOSE_TYPE_UUID),
        token(ordinal.to_string()),
    ])
}

fn parse_native(root: &NativeValue) -> Result<FormMetadataNativeIr, FormMetadataBuildError> {
    let root = exact_list(root, 3, "Form root").map_err(native_error)?;
    exact_token(&root[0], "1", "Form root marker").map_err(native_error)?;
    exact_token(&root[2], "0", "Form root tail").map_err(native_error)?;
    let envelope = exact_list(&root[1], 2, "Form envelope").map_err(native_error)?;
    exact_token(&envelope[0], "0", "Form envelope marker").map_err(native_error)?;
    let object = exact_list(&envelope[1], 5, "Form object").map_err(native_error)?;
    exact_token(&object[0], FORM_OBJECT_MARKER, "Form object marker").map_err(native_error)?;
    let header = parse_metadata_header(&object[1]).map_err(native_error)?;
    exact_token(&object[2], "0", "Form evidenced tail slot 1").map_err(native_error)?;
    exact_token(&object[3], "1", "Form evidenced tail slot 2").map_err(native_error)?;
    let purposes = exact_list(&object[4], 3, "Form UsePurposes").map_err(native_error)?;
    exact_token(&purposes[0], "2", "Form UsePurposes count").map_err(native_error)?;
    for (index, ordinal) in [(1usize, "1"), (2usize, "2")] {
        let item = exact_list(&purposes[index], 3, "Form UsePurpose").map_err(native_error)?;
        if item[0].as_text() != Some("#") {
            return Err(FormMetadataBuildError::Native(
                "Form UsePurpose is not a typed value reference".to_owned(),
            ));
        }
        exact_token(&item[1], USE_PURPOSE_TYPE_UUID, "Form UsePurpose type")
            .map_err(native_error)?;
        exact_token(&item[2], ordinal, "Form UsePurpose ordinal").map_err(native_error)?;
    }
    Ok(FormMetadataNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        cohort: FormEvidencedCohort::ManagedPlatformAndMobile,
    })
}

fn project_object(
    object: &CanonicalObject,
) -> Result<FormMetadataNativeIr, FormMetadataBuildError> {
    if object.kind().as_str() != FORM_FAMILY {
        return Err(FormMetadataBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "metadata kind is not Form",
        });
    }
    if object.owner().is_none() {
        return Err(FormMetadataBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "Form has no owning metadata object",
        });
    }
    if !object.references().is_empty()
        || !object.generated_types().is_empty()
        || !object.assets().is_empty()
    {
        return Err(FormMetadataBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "Form reference/generated-type/asset inventory is not empty",
        });
    }
    require_property_schema(object)?;
    let form_type = enum_property(object, "FormType")?;
    if form_type != EVIDENCED_FORM_TYPE {
        return Err(FormMetadataBuildError::UnevidencedProperty {
            object: object.identity().uuid(),
            property: "FormType",
            expected: EVIDENCED_FORM_TYPE.to_owned(),
            actual: form_type.to_owned(),
        });
    }
    let include_help = bool_property(object, "IncludeHelpInContents")?;
    if include_help != EVIDENCED_INCLUDE_HELP_IN_CONTENTS {
        return Err(FormMetadataBuildError::UnevidencedProperty {
            object: object.identity().uuid(),
            property: "IncludeHelpInContents",
            expected: EVIDENCED_INCLUDE_HELP_IN_CONTENTS.to_string(),
            actual: include_help.to_string(),
        });
    }
    let purposes = enum_sequence_property(object, "UsePurposes")?;
    if purposes != EVIDENCED_USE_PURPOSES {
        return Err(FormMetadataBuildError::UnevidencedProperty {
            object: object.identity().uuid(),
            property: "UsePurposes",
            expected: EVIDENCED_USE_PURPOSES.join(", "),
            actual: purposes.join(", "),
        });
    }
    let name = text_property(object, "Name")?.to_owned();
    if name.is_empty() {
        return Err(FormMetadataBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "Form Name is empty",
        });
    }
    Ok(FormMetadataNativeIr {
        uuid: object.identity().uuid(),
        name,
        synonyms: localized_property(object, "Synonym")?,
        comment: text_property(object, "Comment")?.to_owned(),
        cohort: FormEvidencedCohort::ManagedPlatformAndMobile,
    })
}

// ---------------------------------------------------------------------------
// `Forms/<Name>.xml` -> canonical object
// ---------------------------------------------------------------------------

/// Projects one `Forms/<Name>.xml` source document into a canonical object
/// owned by `owner`.
///
/// The owner is supplied by the caller because the document itself never names
/// it: in a native `config export` tree the owning metadata object is carried
/// only by the file's position (`Catalogs/CorpusList/Forms/ListForm.xml`).
/// Passing it in keeps the ownership edge evidenced by the tree layout instead
/// of guessed from the document.
pub fn decode_form_metadata_source(
    document: &XmlDocument,
    source_profile: &ProfileId,
    object_path: ObjectPath,
    owner: ObjectUuid,
) -> Result<CanonicalObject, FormSourceDecodeError> {
    let root = document.root();
    if root.name().local() != "MetaDataObject" {
        return Err(FormSourceDecodeError::Structure(
            "root element is not MetaDataObject".to_owned(),
        ));
    }
    let namespaces = Namespaces::from_root(root);
    namespaces.require(root, MD_NAMESPACE, "MetaDataObject")?;
    let form = single_child(root, "MetaDataObject")?;
    namespaces.require(form, MD_NAMESPACE, "Form")?;
    if form.name().local() != FORM_FAMILY {
        return Err(FormSourceDecodeError::Structure(format!(
            "MetaDataObject holds `{}`, not `Form`",
            form.name().local()
        )));
    }
    let uuid = uuid_attribute(form)?;
    let properties = single_child(form, "Form")?;
    namespaces.require(properties, MD_NAMESPACE, "Properties")?;
    if properties.name().local() != "Properties" {
        return Err(FormSourceDecodeError::Structure(format!(
            "Form holds `{}`, not `Properties`",
            properties.name().local()
        )));
    }

    let mut fields = Vec::with_capacity(PROPERTY_SCHEMA.len());
    let mut seen = BTreeSet::new();
    for element in child_elements(properties) {
        let local = element.name().local();
        if !PROPERTY_SCHEMA.contains(&local) {
            return Err(FormSourceDecodeError::UnsupportedProperty(local.to_owned()));
        }
        namespaces.require(element, MD_NAMESPACE, local)?;
        if !seen.insert(local.to_owned()) {
            return Err(FormSourceDecodeError::Structure(format!(
                "Form property `{local}` is duplicated"
            )));
        }
        let value = match local {
            "Name" | "Comment" => CanonicalValue::text(canonical_text(&element_text(element)?)?),
            "FormType" => CanonicalValue::enum_token(
                ibcmd_core::value::EnumToken::new(&element_text(element)?)
                    .map_err(|error| FormSourceDecodeError::Core(error.to_string()))?,
            ),
            "IncludeHelpInContents" => {
                CanonicalValue::boolean(parse_bool(&element_text(element)?)?)
            }
            "Synonym" => localized_value(element, &namespaces)?,
            "UsePurposes" => use_purposes_value(element, &namespaces)?,
            _ => unreachable!("property names are checked against PROPERTY_SCHEMA"),
        };
        fields.push(
            CanonicalField::named(local, value)
                .map_err(|error| FormSourceDecodeError::Core(error.to_string()))?,
        );
    }
    for expected in PROPERTY_SCHEMA {
        if !seen.contains(*expected) {
            return Err(FormSourceDecodeError::MissingProperty(expected));
        }
    }
    // Canonical property order is the compiler's contract, not the document's.
    fields.sort_by_key(|field| {
        PROPERTY_SCHEMA
            .iter()
            .position(|name| *name == field.name().as_str())
            .expect("every accepted field is in the schema")
    });

    let anchor = CanonicalAnchor::new(
        object_path.clone(),
        PropertyPath::new(vec![
            PathSegment::name("Properties")
                .map_err(|error| FormSourceDecodeError::Core(error.to_string()))?,
        ])
        .map_err(|error| FormSourceDecodeError::Core(error.to_string()))?,
    );
    let mut parts = CanonicalObjectParts::new(
        LogicalIdentity::new(uuid, object_path),
        MetadataKind::new(FORM_FAMILY)
            .map_err(|error| FormSourceDecodeError::Core(error.to_string()))?,
        SourceProvenance::new(source_profile.clone(), anchor),
    );
    parts.owner = Some(owner);
    parts.properties = fields;
    CanonicalObject::new(parts).map_err(|error| FormSourceDecodeError::Core(error.to_string()))
}

/// Bounded prefix -> namespace-URI bindings declared on the document root.
///
/// Source-tree roots declare every prefix they use, which is why the bootstrap
/// coordinator resolves the export-manifest namespace the same way.
struct Namespaces {
    bindings: Vec<(Option<String>, String)>,
}

impl Namespaces {
    fn from_root(root: &XmlElement) -> Self {
        let mut bindings = Vec::new();
        for attribute in root.attributes() {
            if let AttributeKind::Namespace(prefix) = attribute.kind() {
                bindings.push((prefix.clone(), attribute.value().to_owned()));
            }
        }
        Self { bindings }
    }

    fn uri_of(&self, element: &XmlElement) -> Option<&str> {
        let prefix = element.name().prefix().map(str::to_owned);
        self.bindings
            .iter()
            .find(|(declared, _)| *declared == prefix)
            .map(|(_, uri)| uri.as_str())
    }

    fn require(
        &self,
        element: &XmlElement,
        namespace: &str,
        context: &str,
    ) -> Result<(), FormSourceDecodeError> {
        if self.uri_of(element) == Some(namespace) {
            return Ok(());
        }
        Err(FormSourceDecodeError::Namespace {
            context: context.to_owned(),
            expected: namespace.to_owned(),
            actual: self
                .uri_of(element)
                .map_or_else(|| "<unbound>".to_owned(), str::to_owned),
        })
    }
}

fn localized_value(
    element: &XmlElement,
    namespaces: &Namespaces,
) -> Result<CanonicalValue, FormSourceDecodeError> {
    let mut items = Vec::new();
    for item in child_elements(element) {
        namespaces.require(item, V8_NAMESPACE, "Synonym item")?;
        if item.name().local() != "item" {
            return Err(FormSourceDecodeError::Structure(format!(
                "Synonym holds `{}`, not `item`",
                item.name().local()
            )));
        }
        let children = child_elements(item);
        if children.len() != 2
            || children[0].name().local() != "lang"
            || children[1].name().local() != "content"
        {
            return Err(FormSourceDecodeError::Structure(
                "Synonym item is not exactly `lang` followed by `content`".to_owned(),
            ));
        }
        namespaces.require(children[0], V8_NAMESPACE, "Synonym lang")?;
        namespaces.require(children[1], V8_NAMESPACE, "Synonym content")?;
        let language = element_text(children[0])?;
        if language.is_empty() {
            return Err(FormSourceDecodeError::Structure(
                "Synonym item language is empty".to_owned(),
            ));
        }
        items.push(
            CanonicalValue::record(vec![
                CanonicalField::named("lang", CanonicalValue::text(canonical_text(&language)?))
                    .map_err(|error| FormSourceDecodeError::Core(error.to_string()))?,
                CanonicalField::named(
                    "content",
                    CanonicalValue::text(canonical_text(&element_text(children[1])?)?),
                )
                .map_err(|error| FormSourceDecodeError::Core(error.to_string()))?,
            ])
            .map_err(|error| FormSourceDecodeError::Core(error.to_string()))?,
        );
    }
    CanonicalValue::sequence(items).map_err(|error| FormSourceDecodeError::Core(error.to_string()))
}

fn use_purposes_value(
    element: &XmlElement,
    namespaces: &Namespaces,
) -> Result<CanonicalValue, FormSourceDecodeError> {
    let mut values = Vec::new();
    for item in child_elements(element) {
        namespaces.require(item, V8_NAMESPACE, "UsePurposes value")?;
        if item.name().local() != "Value" {
            return Err(FormSourceDecodeError::Structure(format!(
                "UsePurposes holds `{}`, not `Value`",
                item.name().local()
            )));
        }
        require_use_purpose_type(item, namespaces)?;
        values.push(CanonicalValue::enum_token(
            ibcmd_core::value::EnumToken::new(&element_text(item)?)
                .map_err(|error| FormSourceDecodeError::Core(error.to_string()))?,
        ));
    }
    CanonicalValue::sequence(values).map_err(|error| FormSourceDecodeError::Core(error.to_string()))
}

fn require_use_purpose_type(
    item: &XmlElement,
    namespaces: &Namespaces,
) -> Result<(), FormSourceDecodeError> {
    let mut declared = None;
    for attribute in item.attributes() {
        let AttributeKind::Ordinary(name) = attribute.kind() else {
            continue;
        };
        let uri = name.prefix().and_then(|prefix| {
            namespaces
                .bindings
                .iter()
                .find(|(declared, _)| declared.as_deref() == Some(prefix))
                .map(|(_, uri)| uri.as_str())
        });
        if uri != Some(XSI_NAMESPACE) || name.local() != "type" {
            return Err(FormSourceDecodeError::Structure(format!(
                "UsePurposes value carries unsupported attribute `{}`",
                name.raw()
            )));
        }
        if declared.is_some() {
            return Err(FormSourceDecodeError::Structure(
                "UsePurposes value declares `xsi:type` twice".to_owned(),
            ));
        }
        declared = Some(attribute.value().to_owned());
    }
    let declared = declared.ok_or_else(|| {
        FormSourceDecodeError::Structure("UsePurposes value has no `xsi:type`".to_owned())
    })?;
    let (prefix, local) = declared.split_once(':').ok_or_else(|| {
        FormSourceDecodeError::Structure(format!("`xsi:type` `{declared}` is not prefixed"))
    })?;
    let uri = namespaces
        .bindings
        .iter()
        .find(|(name, _)| name.as_deref() == Some(prefix))
        .map(|(_, uri)| uri.as_str());
    if uri != Some(APP_NAMESPACE) || local != USE_PURPOSE_XSI_TYPE {
        return Err(FormSourceDecodeError::Namespace {
            context: "UsePurposes xsi:type".to_owned(),
            expected: format!("{{{APP_NAMESPACE}}}{USE_PURPOSE_XSI_TYPE}"),
            actual: declared,
        });
    }
    Ok(())
}

fn child_elements(element: &XmlElement) -> Vec<&XmlElement> {
    element
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(child) => Some(child),
            _ => None,
        })
        .collect()
}

fn single_child<'a>(
    element: &'a XmlElement,
    context: &str,
) -> Result<&'a XmlElement, FormSourceDecodeError> {
    let children = child_elements(element);
    if children.len() != 1 {
        return Err(FormSourceDecodeError::Structure(format!(
            "{context} must contain exactly one element, found {}",
            children.len()
        )));
    }
    Ok(children[0])
}

fn element_text(element: &XmlElement) -> Result<String, FormSourceDecodeError> {
    let mut value = String::new();
    for node in element.children() {
        match node {
            XmlNode::Text(entry) => value.push_str(entry.value()),
            XmlNode::CData(entry) => value.push_str(entry.value()),
            XmlNode::Comment(_) => {}
            XmlNode::Element(child) => {
                return Err(FormSourceDecodeError::Structure(format!(
                    "`{}` contains nested `{}`",
                    element.name().local(),
                    child.name().local()
                )));
            }
            XmlNode::ProcessingInstruction(_) | XmlNode::DocType(_) => {
                return Err(FormSourceDecodeError::Structure(format!(
                    "`{}` contains unsupported markup",
                    element.name().local()
                )));
            }
        }
    }
    Ok(value)
}

fn parse_bool(value: &str) -> Result<bool, FormSourceDecodeError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(FormSourceDecodeError::Structure(format!(
            "`{other}` is not a boolean literal"
        ))),
    }
}

fn canonical_text(value: &str) -> Result<CanonicalText, FormSourceDecodeError> {
    CanonicalText::new(value).map_err(|error| FormSourceDecodeError::Core(error.to_string()))
}

fn uuid_attribute(element: &XmlElement) -> Result<ObjectUuid, FormSourceDecodeError> {
    let mut found = None;
    for attribute in element.attributes() {
        let AttributeKind::Ordinary(name) = attribute.kind() else {
            continue;
        };
        if name.prefix().is_some() || name.local() != "uuid" {
            return Err(FormSourceDecodeError::Structure(format!(
                "Form carries unsupported attribute `{}`",
                name.raw()
            )));
        }
        if found.is_some() {
            return Err(FormSourceDecodeError::Structure(
                "Form declares `uuid` twice".to_owned(),
            ));
        }
        found = Some(attribute.value().to_owned());
    }
    let value = found.ok_or_else(|| {
        FormSourceDecodeError::Structure("Form has no `uuid` attribute".to_owned())
    })?;
    ObjectUuid::parse(&value)
        .map_err(|error| FormSourceDecodeError::Core(format!("Form uuid `{value}`: {error}")))
}

/// Failure to project `Forms/<Name>.xml` into a canonical object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormSourceDecodeError {
    Structure(String),
    Namespace {
        context: String,
        expected: String,
        actual: String,
    },
    UnsupportedProperty(String),
    MissingProperty(&'static str),
    Core(String),
}

impl Display for FormSourceDecodeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Structure(reason) => write!(formatter, "Form source is not exact: {reason}"),
            Self::Namespace {
                context,
                expected,
                actual,
            } => write!(
                formatter,
                "Form source `{context}` is bound to `{actual}`, expected `{expected}`"
            ),
            Self::UnsupportedProperty(name) => write!(
                formatter,
                "Form property `{name}` has no evidenced base-free projection"
            ),
            Self::MissingProperty(name) => {
                write!(formatter, "Form source has no `{name}` property")
            }
            Self::Core(reason) => write!(formatter, "Form source cannot be modelled: {reason}"),
        }
    }
}

impl Error for FormSourceDecodeError {}

// ---------------------------------------------------------------------------
// canonical helpers
// ---------------------------------------------------------------------------

fn validate_coordinates(
    graph: &BootstrapGraph,
    axes: &CompileAxes,
    profile: &FormMetadataProfile,
) -> Result<(), FormMetadataBuildError> {
    if graph.profile_id() != profile.profile_id() {
        return Err(FormMetadataBuildError::ProfileMismatch {
            graph: graph.profile_id().clone(),
            codec: profile.profile_id.clone(),
        });
    }
    if axes.platform_build() != Some(&profile.platform_build) {
        return Err(FormMetadataBuildError::AxisMismatch("platform_build"));
    }
    if axes.storage_profile() != &profile.storage_profile {
        return Err(FormMetadataBuildError::AxisMismatch("storage_profile"));
    }
    if axes.compatibility_mode().is_some() || axes.container_revision().is_some() {
        return Err(FormMetadataBuildError::AxisMismatch(
            "unevidenced optional coordinate",
        ));
    }
    if !matches!(axes.xml_dialect().to_string().as_str(), "2.20" | "2.21") {
        return Err(FormMetadataBuildError::AxisMismatch("xml_dialect"));
    }
    Ok(())
}

fn require_property_schema(object: &CanonicalObject) -> Result<(), FormMetadataBuildError> {
    if object.properties().len() != PROPERTY_SCHEMA.len()
        || object
            .properties()
            .iter()
            .zip(PROPERTY_SCHEMA)
            .any(|(field, expected)| field.name().as_str() != *expected)
    {
        return Err(FormMetadataBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "canonical property schema is not exact",
        });
    }
    Ok(())
}

fn property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a CanonicalValue, FormMetadataBuildError> {
    object
        .properties()
        .iter()
        .find(|field| field.name().as_str() == name)
        .map(CanonicalField::value)
        .ok_or(FormMetadataBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "required typed property is missing",
        })
}

fn text_property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a str, FormMetadataBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Text(value) => Ok(value.as_str()),
        _ => Err(FormMetadataBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "typed property is not text",
        }),
    }
}

fn bool_property(object: &CanonicalObject, name: &str) -> Result<bool, FormMetadataBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Bool(value) => Ok(value),
        _ => Err(FormMetadataBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "typed property is not boolean",
        }),
    }
}

fn enum_property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a str, FormMetadataBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::EnumToken(value) => Ok(value.as_str()),
        _ => Err(FormMetadataBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "typed property is not an enum token",
        }),
    }
}

fn enum_sequence_property(
    object: &CanonicalObject,
    name: &str,
) -> Result<Vec<String>, FormMetadataBuildError> {
    let values =
        property(object, name)?
            .as_sequence()
            .ok_or(FormMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "enum collection is not a sequence",
            })?;
    let mut output = Vec::with_capacity(values.len());
    for value in values {
        match value.kind() {
            CanonicalValueKind::EnumToken(token) => output.push(token.as_str().to_owned()),
            _ => {
                return Err(FormMetadataBuildError::InvalidModel {
                    object: object.identity().uuid(),
                    reason: "enum collection item is not an enum token",
                });
            }
        }
    }
    Ok(output)
}

fn localized_property(
    object: &CanonicalObject,
    name: &str,
) -> Result<Vec<(String, String)>, FormMetadataBuildError> {
    let values =
        property(object, name)?
            .as_sequence()
            .ok_or(FormMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "localized property is not a sequence",
            })?;
    let mut output = Vec::with_capacity(values.len());
    let mut languages = BTreeSet::new();
    for value in values {
        let fields = value
            .as_record()
            .ok_or(FormMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "localized item is not a record",
            })?;
        if fields.len() != 2
            || fields[0].name().as_str() != "lang"
            || fields[1].name().as_str() != "content"
        {
            return Err(FormMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "localized item schema is not exact",
            });
        }
        let (CanonicalValueKind::Text(language), CanonicalValueKind::Text(content)) =
            (fields[0].value().kind(), fields[1].value().kind())
        else {
            return Err(FormMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "localized item fields are not text",
            });
        };
        if !languages.insert(language.as_str().to_owned()) {
            return Err(FormMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "localized language is duplicated",
            });
        }
        output.push((language.as_str().to_owned(), content.as_str().to_owned()));
    }
    Ok(output)
}

fn native_error(error: impl Display) -> FormMetadataBuildError {
    FormMetadataBuildError::Native(error.to_string())
}

#[derive(Debug)]
pub enum FormMetadataBuildError {
    Profile(FormMetadataProfileError),
    ProfileMismatch {
        graph: ProfileId,
        codec: ProfileId,
    },
    AxisMismatch(&'static str),
    UnknownObject(ObjectUuid),
    MissingPrimaryRoute(ObjectUuid),
    InvalidModel {
        object: ObjectUuid,
        reason: &'static str,
    },
    /// A property whose value has no retained platform bytes behind it.
    UnevidencedProperty {
        object: ObjectUuid,
        property: &'static str,
        expected: String,
        actual: String,
    },
    Native(String),
    Storage(StorageBuildError),
    Patch(StoragePatchBuildError),
}

impl Display for FormMetadataBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => Display::fmt(source, formatter),
            Self::ProfileMismatch { graph, codec } => write!(
                formatter,
                "graph profile `{graph}` differs from codec `{codec}`"
            ),
            Self::AxisMismatch(axis) => write!(formatter, "Form `{axis}` axis mismatch"),
            Self::UnknownObject(uuid) => write!(formatter, "validated graph has no object {uuid}"),
            Self::MissingPrimaryRoute(uuid) => {
                write!(formatter, "bootstrap graph has no primary row for {uuid}")
            }
            Self::InvalidModel { object, reason } => {
                write!(formatter, "Form {object} is not compilable: {reason}")
            }
            Self::UnevidencedProperty {
                object,
                property,
                expected,
                actual,
            } => write!(
                formatter,
                "Form {object} property `{property}` is `{actual}`, but only `{expected}` is \
                 evidenced by retained platform bytes"
            ),
            Self::Native(reason) => write!(formatter, "invalid Form native row: {reason}"),
            Self::Storage(source) => Display::fmt(source, formatter),
            Self::Patch(source) => Display::fmt(source, formatter),
        }
    }
}

impl Error for FormMetadataBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::Patch(source) => Some(source),
            _ => None,
        }
    }
}

impl From<FormMetadataProfileError> for FormMetadataBuildError {
    fn from(source: FormMetadataProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<StorageBuildError> for FormMetadataBuildError {
    fn from(source: StorageBuildError) -> Self {
        Self::Storage(source)
    }
}

impl From<StoragePatchBuildError> for FormMetadataBuildError {
    fn from(source: StoragePatchBuildError) -> Self {
        Self::Patch(source)
    }
}

#[cfg(test)]
mod tests {
    use ibcmd_xml::XmlReader;

    use super::*;

    /// Byte-for-byte transcription of the `Forms/ListForm.xml` document our own
    /// `cf export` produces from the bundled retained corpus
    /// `tests/fixtures/native-evidence/8.3.27.2214/dcs-form-list-settings-server-state`
    /// (storage key `c099c30b-8a08-45b0-be7d-9594366e78a5`).
    const NATIVE_FORM_SOURCE: &str = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" ",
        "xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" ",
        "xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" ",
        "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\n",
        "\t<Form uuid=\"c099c30b-8a08-45b0-be7d-9594366e78a5\">\n",
        "\t\t<Properties>\n",
        "\t\t\t<Name>ListForm</Name>\n",
        "\t\t\t<Synonym>\n",
        "\t\t\t\t<v8:item>\n",
        "\t\t\t\t\t<v8:lang>ru</v8:lang>\n",
        "\t\t\t\t\t<v8:content>ListForm</v8:content>\n",
        "\t\t\t\t</v8:item>\n",
        "\t\t\t</Synonym>\n",
        "\t\t\t<Comment/>\n",
        "\t\t\t<FormType>Managed</FormType>\n",
        "\t\t\t<IncludeHelpInContents>false</IncludeHelpInContents>\n",
        "\t\t\t<UsePurposes>\n",
        "\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">PlatformApplication</v8:Value>\n",
        "\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">",
        "MobilePlatformApplication</v8:Value>\n",
        "\t\t\t</UsePurposes>\n",
        "\t\t</Properties>\n",
        "\t</Form>\n",
        "</MetaDataObject>\n",
    );

    /// Exact plaintext of the retained `c099c30b-…` storage record, read back
    /// with `cf extract` from the same bundled corpus.  All 13 `Form` metadata
    /// records across the bundled corpora share this shape.
    const NATIVE_FORM_RECORD: &str = concat!(
        "\u{feff}{1,\r\n{0,\r\n{13,\r\n{3,\r\n",
        "{1,0,c099c30b-8a08-45b0-be7d-9594366e78a5},\"ListForm\",\r\n",
        "{1,\"ru\",\"ListForm\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},0,1,\r\n",
        "{2,\r\n",
        "{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,1},\r\n",
        "{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,2}\r\n",
        "}\r\n",
        "}\r\n",
        "},0}",
    );

    fn owner() -> ObjectUuid {
        ObjectUuid::parse("a4d6ccbb-a666-4217-ad1a-174221faba2b").unwrap()
    }

    fn object_path() -> ObjectPath {
        ObjectPath::new(vec![
            PathSegment::name("source").unwrap(),
            PathSegment::index(0),
        ])
        .unwrap()
    }

    fn decoded_source() -> CanonicalObject {
        let document = XmlReader::from_slice(NATIVE_FORM_SOURCE.as_bytes()).unwrap();
        decode_form_metadata_source(
            &document,
            &ProfileId::parse("xml-2.20").unwrap(),
            object_path(),
            owner(),
        )
        .unwrap()
    }

    #[test]
    fn native_form_source_projects_the_evidenced_cohort() {
        let object = decoded_source();
        assert_eq!(object.kind().as_str(), "Form");
        assert_eq!(object.owner(), Some(owner()));
        assert_eq!(
            object
                .properties()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            PROPERTY_SCHEMA
        );
        let ir = project_object(&object).unwrap();
        assert_eq!(ir.name, "ListForm");
        assert_eq!(ir.synonyms, vec![("ru".to_owned(), "ListForm".to_owned())]);
        assert_eq!(ir.comment, "");
        assert_eq!(ir.cohort, FormEvidencedCohort::ManagedPlatformAndMobile);
    }

    #[test]
    fn retained_platform_record_is_reproduced_byte_for_byte() {
        let profile = FormMetadataProfile::fixture();
        let ir = project_object(&decoded_source()).unwrap();
        assert_eq!(
            form_metadata_plaintext(&ir, &profile).unwrap(),
            NATIVE_FORM_RECORD.as_bytes()
        );
        let blob = encode_form_metadata_blob(&ir, &profile).unwrap();
        assert_eq!(decode_form_metadata_blob(&blob, &profile).unwrap(), ir);
    }

    #[test]
    fn properties_outside_the_evidenced_cohort_are_named_refusals() {
        for (needle, replacement, property, actual) in [
            (
                "<FormType>Managed</FormType>",
                "<FormType>Ordinary</FormType>",
                "FormType",
                "Ordinary",
            ),
            (
                "<IncludeHelpInContents>false</IncludeHelpInContents>",
                "<IncludeHelpInContents>true</IncludeHelpInContents>",
                "IncludeHelpInContents",
                "true",
            ),
            (
                "\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">\
                 MobilePlatformApplication</v8:Value>\n",
                "",
                "UsePurposes",
                "PlatformApplication",
            ),
        ] {
            let source = NATIVE_FORM_SOURCE.replace(needle, replacement);
            let document = XmlReader::from_slice(source.as_bytes()).unwrap();
            let object = decode_form_metadata_source(
                &document,
                &ProfileId::parse("xml-2.20").unwrap(),
                object_path(),
                owner(),
            )
            .unwrap();
            let error = project_object(&object).unwrap_err();
            let FormMetadataBuildError::UnevidencedProperty {
                property: reported,
                actual: reported_actual,
                ..
            } = &error
            else {
                panic!("{property} must be refused by name, got {error}");
            };
            assert_eq!(*reported, property);
            assert_eq!(reported_actual, actual);
        }
    }

    #[test]
    fn unknown_properties_are_refused_by_name() {
        let source =
            NATIVE_FORM_SOURCE.replace("<Comment/>", "<Comment/>\n\t\t\t<ExtendedPresentation/>");
        let document = XmlReader::from_slice(source.as_bytes()).unwrap();
        let error = decode_form_metadata_source(
            &document,
            &ProfileId::parse("xml-2.20").unwrap(),
            object_path(),
            owner(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            FormSourceDecodeError::UnsupportedProperty("ExtendedPresentation".to_owned())
        );
    }

    #[test]
    fn missing_properties_are_refused_by_name() {
        let source = NATIVE_FORM_SOURCE.replace("\t\t\t<Comment/>\n", "");
        let document = XmlReader::from_slice(source.as_bytes()).unwrap();
        let error = decode_form_metadata_source(
            &document,
            &ProfileId::parse("xml-2.20").unwrap(),
            object_path(),
            owner(),
        )
        .unwrap_err();
        assert_eq!(error, FormSourceDecodeError::MissingProperty("Comment"));
    }
}
