//! Strict offline XML codecs for modules, commands, groups and pictures.

use std::collections::BTreeMap;

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::family::FamilyId;
use ibcmd_core::model::{CanonicalObject, CanonicalObjectParts};
use ibcmd_core::value::CanonicalValue;

use super::business_objects::{
    exact_property_map, only_element_child, push_bool, push_enum, push_localized, push_text,
    require_empty, text_field,
};
use super::common::{
    MD_NAMESPACE, MetadataDecodeError, MetadataEnvelope, ResolvedNamespaces, V8_NAMESPACE,
    XR_NAMESPACE, element_text, resolve_namespaces, typed, uri_of,
};
use super::decode_metadata_envelope;
use super::language::{
    canonical_field, canonical_text, copy_object_parts, decode_to_encode, invalid_model,
    profile_version, root_version, set_unprefixed_attribute, validate_decode_profile,
};
use super::registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
use crate::{LexicalPolicy, XmlDocument, XmlElement, XmlNode, XmlWriter};

const COMMON_MODULE: &str = "CommonModule";
const COMMON_COMMAND: &str = "CommonCommand";
const COMMAND_GROUP: &str = "CommandGroup";
const COMMON_PICTURE: &str = "CommonPicture";

const COMMON_MODULE_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Global",
    "ClientManagedApplication",
    "Server",
    "ExternalConnection",
    "ClientOrdinaryApplication",
    "ServerCall",
    "Privileged",
    "ReturnValuesReuse",
];
const COMMON_COMMAND_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Group",
    "Representation",
    "ToolTip",
    "Picture",
    "Shortcut",
    "IncludeHelpInContents",
    "CommandParameterType",
    "ParameterUseMode",
    "ModifiesData",
    "OnMainServerUnavalableBehavior",
];
const COMMAND_GROUP_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Representation",
    "ToolTip",
    "Picture",
    "Category",
];
const COMMON_PICTURE_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "AvailabilityForChoice",
    "AvailabilityForAppearance",
];

pub fn register_common_module_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, COMMON_MODULE)
}

pub fn register_common_command_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, COMMON_COMMAND)
}

pub fn register_command_group_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, COMMAND_GROUP)
}

pub fn register_common_picture_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, COMMON_PICTURE)
}

fn register(
    registry: &mut MetadataRegistry,
    family: &'static str,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(CommonObjectCodec {
        family: FamilyId::parse(family).expect("common-object family id is stable"),
        name: family,
    }))
}

struct CommonObjectCodec {
    family: FamilyId,
    name: &'static str,
}

impl MetadataFamilyCodec for CommonObjectCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_common_object(document, source, path, self.name)
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_common_object(envelope, target, self.name)
    }
}

fn decode_common_object(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
    family: &str,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let generic = decode_metadata_envelope(document, source, path)?;
    if generic.root().kind().as_str() != family {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "common-object codec family differs from XML",
        ));
    }
    if !generic.descendants().is_empty()
        || generic.root().owner().is_some()
        || !generic.root().generated_types().is_empty()
        || !generic.root().references().is_empty()
        || !generic.root().assets().is_empty()
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "common object cannot contain children, generated types, references, or assets",
        ));
    }

    let uris = resolve_namespaces(document.root())?;
    if uri_of(document.root(), &uris) != Some(MD_NAMESPACE) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "common object requires the MDClasses namespace",
        ));
    }
    let object = only_element_child(document.root(), "metadata object")?;
    if !typed(object, family, Some(MD_NAMESPACE), &uris) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "common object element is not exact",
        ));
    }
    let properties = exact_properties(object, &uris)?;
    let names = match family {
        COMMON_MODULE => COMMON_MODULE_PROPERTIES,
        COMMON_COMMAND => COMMON_COMMAND_PROPERTIES,
        COMMAND_GROUP => COMMAND_GROUP_PROPERTIES,
        COMMON_PICTURE => COMMON_PICTURE_PROPERTIES,
        _ => unreachable!(),
    };
    let map = exact_property_map(properties, names, &uris)?;
    let mut parts = copy_object_parts(generic.root());
    project_properties(&mut parts, family, &map, &uris)?;
    let root = CanonicalObject::new(parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;
    MetadataEnvelope::from_parts(root, Vec::new(), document.clone())
}

fn exact_properties<'a>(
    object: &'a XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<&'a XmlElement, MetadataDecodeError> {
    let mut result = None;
    for node in object.children() {
        let XmlNode::Element(element) = node else {
            continue;
        };
        if typed(element, "Properties", Some(MD_NAMESPACE), uris)
            && result.replace(element).is_none()
        {
            continue;
        }
        return Err(MetadataDecodeError::InvalidEnvelope(
            "common object contains an unknown section",
        ));
    }
    result.ok_or(MetadataDecodeError::Missing("Properties"))
}

fn project_properties(
    parts: &mut CanonicalObjectParts,
    family: &str,
    map: &BTreeMap<&str, &XmlElement>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    push_text(parts, map, "Comment")?;
    match family {
        COMMON_MODULE => {
            for name in [
                "Global",
                "ClientManagedApplication",
                "Server",
                "ExternalConnection",
                "ClientOrdinaryApplication",
                "ServerCall",
                "Privileged",
            ] {
                push_bool(parts, map, name)?;
            }
            push_checked_enum(
                parts,
                map,
                "ReturnValuesReuse",
                &["DontUse", "DuringRequest", "DuringSession"],
            )?;
        }
        COMMON_COMMAND => {
            push_text(parts, map, "Group")?;
            push_checked_enum(
                parts,
                map,
                "Representation",
                &["Text", "Picture", "PictureAndText", "Auto"],
            )?;
            push_localized(parts, map, "ToolTip", uris)?;
            push_picture(parts, map["Picture"], uris)?;
            require_empty(map["Shortcut"], "Shortcut")?;
            push_bool(parts, map, "IncludeHelpInContents")?;
            push_parameter_type(parts, map["CommandParameterType"], uris)?;
            push_checked_enum(parts, map, "ParameterUseMode", &["Single", "Multiple"])?;
            push_bool(parts, map, "ModifiesData")?;
            push_checked_enum(parts, map, "OnMainServerUnavalableBehavior", &["Auto"])?;
        }
        COMMAND_GROUP => {
            push_checked_enum(
                parts,
                map,
                "Representation",
                &["Text", "Picture", "PictureAndText", "Auto"],
            )?;
            push_localized(parts, map, "ToolTip", uris)?;
            push_picture(parts, map["Picture"], uris)?;
            push_checked_enum(
                parts,
                map,
                "Category",
                &[
                    "NavigationPanel",
                    "FormNavigationPanel",
                    "ActionsPanel",
                    "FormCommandBar",
                ],
            )?;
        }
        COMMON_PICTURE => {
            push_bool(parts, map, "AvailabilityForChoice")?;
            push_bool(parts, map, "AvailabilityForAppearance")?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn push_checked_enum(
    parts: &mut CanonicalObjectParts,
    map: &BTreeMap<&str, &XmlElement>,
    name: &str,
    allowed: &[&str],
) -> Result<(), MetadataDecodeError> {
    let value = text_field(map, name)?;
    if !allowed.contains(&value.as_str()) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "common-object enum has no evidenced native mapping",
        ));
    }
    push_enum(parts, map, name)
}

fn push_picture(
    parts: &mut CanonicalObjectParts,
    picture: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let children = picture
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(element) => Some(element),
            _ => None,
        })
        .collect::<Vec<_>>();
    let (reference, load_transparent) = if children.is_empty() {
        require_empty(picture, "Picture")?;
        (String::new(), false)
    } else {
        if children.len() != 2
            || !typed(children[0], "Ref", Some(XR_NAMESPACE), uris)
            || !typed(children[1], "LoadTransparent", Some(XR_NAMESPACE), uris)
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Picture must contain exact xr:Ref and xr:LoadTransparent fields",
            ));
        }
        let reference = element_text(children[0])?.ok_or(MetadataDecodeError::InvalidEnvelope(
            "Picture reference must contain text only",
        ))?;
        if reference.is_empty() {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "non-empty Picture has an empty reference",
            ));
        }
        let load_transparent = match element_text(children[1])?.as_deref() {
            Some("true") => true,
            Some("false") => false,
            _ => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "Picture LoadTransparent is not canonical boolean",
                ));
            }
        };
        (reference, load_transparent)
    };
    parts.properties.push(canonical_field(
        "PictureReference",
        CanonicalValue::text(canonical_text(&reference)?),
    )?);
    parts.properties.push(canonical_field(
        "PictureLoadTransparent",
        CanonicalValue::boolean(load_transparent),
    )?);
    Ok(())
}

fn push_parameter_type(
    parts: &mut CanonicalObjectParts,
    property: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let children = property
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(element) => Some(element),
            _ => None,
        })
        .collect::<Vec<_>>();
    let value = if children.is_empty() {
        require_empty(property, "CommandParameterType")?;
        String::new()
    } else {
        if children.len() != 1 || !typed(children[0], "TypeSet", Some(V8_NAMESPACE), uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "CommandParameterType must contain one v8:TypeSet",
            ));
        }
        let value = element_text(children[0])?.ok_or(MetadataDecodeError::InvalidEnvelope(
            "CommandParameterType TypeSet must contain text only",
        ))?;
        if !value.starts_with("cfg:DefinedType.") {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "CommandParameterType is not a cfg:DefinedType reference",
            ));
        }
        value
    };
    parts.properties.push(canonical_field(
        "CommandParameterType",
        CanonicalValue::text(canonical_text(&value)?),
    )?);
    Ok(())
}

fn encode_common_object(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
    family: &str,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path().clone();
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    if envelope.root().kind().as_str() != family {
        return Err(invalid_model(&path, "kind"));
    }
    let source_profile = envelope.root().provenance().source_profile().clone();
    validate_decode_profile(envelope.source_document(), &source_profile, &path)
        .map_err(decode_to_encode)?;
    let source = decode_common_object(
        envelope.source_document(),
        source_profile,
        path.clone(),
        family,
    )
    .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "common-object semantic mutation is not implemented",
        ));
    }
    let root = if root_version(envelope.source_document().root()).map_err(decode_to_encode)?
        == target_version
    {
        envelope.source_document().root().clone()
    } else {
        set_unprefixed_attribute(
            envelope.source_document().root(),
            "version",
            target_version,
            &path,
        )?
    };
    XmlWriter::to_vec(
        &envelope.source_document().with_root(root),
        LexicalPolicy::Preserve,
    )
    .map_err(|error| MetadataEncodeError::Xml(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::XmlReader;

    fn decode(family: &str, properties: &str) -> Result<MetadataEnvelope, MetadataDecodeError> {
        let xml = format!(
            "<MetaDataObject xmlns='{MD_NAMESPACE}' xmlns:v8='{V8_NAMESPACE}' xmlns:xr='{XR_NAMESPACE}' version='2.20'><{family} uuid='11111111-1111-4111-8111-111111111111'><Properties>{properties}</Properties></{family}></MetaDataObject>"
        );
        let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
        decode_common_object(
            &document,
            ProfileId::parse("xml-2.20").unwrap(),
            ObjectPath::root(),
            family,
        )
    }

    #[test]
    fn all_four_common_objects_project_exact_typed_fields() {
        let module = decode(
            COMMON_MODULE,
            "<Name>M</Name><Synonym/><Comment/><Global>false</Global><ClientManagedApplication>true</ClientManagedApplication><Server>true</Server><ExternalConnection>false</ExternalConnection><ClientOrdinaryApplication>false</ClientOrdinaryApplication><ServerCall>true</ServerCall><Privileged>false</Privileged><ReturnValuesReuse>DuringRequest</ReturnValuesReuse>",
        )
        .unwrap();
        assert_eq!(
            module.root().properties().len(),
            COMMON_MODULE_PROPERTIES.len()
        );

        let command = decode(
            COMMON_COMMAND,
            "<Name>C</Name><Synonym/><Comment/><Group>NavigationPanelOrdinary</Group><Representation>Auto</Representation><ToolTip/><Picture/><Shortcut/><IncludeHelpInContents>false</IncludeHelpInContents><CommandParameterType/><ParameterUseMode>Single</ParameterUseMode><ModifiesData>false</ModifiesData><OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>",
        )
        .unwrap();
        assert_eq!(
            command.root().properties().len(),
            COMMON_COMMAND_PROPERTIES.len()
        );

        let group = decode(
            COMMAND_GROUP,
            "<Name>G</Name><Synonym/><Comment/><Representation>Picture</Representation><ToolTip/><Picture><xr:Ref>StdPicture.Print</xr:Ref><xr:LoadTransparent>true</xr:LoadTransparent></Picture><Category>FormCommandBar</Category>",
        )
        .unwrap();
        assert_eq!(
            group.root().properties().len(),
            COMMAND_GROUP_PROPERTIES.len() + 1
        );

        let picture = decode(
            COMMON_PICTURE,
            "<Name>P</Name><Synonym/><Comment/><AvailabilityForChoice>false</AvailabilityForChoice><AvailabilityForAppearance>true</AvailabilityForAppearance>",
        )
        .unwrap();
        assert_eq!(
            picture.root().properties().len(),
            COMMON_PICTURE_PROPERTIES.len()
        );
    }

    #[test]
    fn unknown_property_and_unsupported_parameter_type_fail_closed() {
        assert!(decode(COMMON_PICTURE, "<Name>P</Name><Synonym/><Comment/><AvailabilityForChoice>false</AvailabilityForChoice><AvailabilityForAppearance>false</AvailabilityForAppearance><Future/>").is_err());
        assert!(decode(COMMON_COMMAND, "<Name>C</Name><Synonym/><Comment/><Group>NavigationPanelOrdinary</Group><Representation>Auto</Representation><ToolTip/><Picture/><Shortcut/><IncludeHelpInContents>false</IncludeHelpInContents><CommandParameterType><v8:TypeSet>xs:string</v8:TypeSet></CommandParameterType><ParameterUseMode>Single</ParameterUseMode><ModifiesData>false</ModifiesData><OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>").is_err());
    }
}
