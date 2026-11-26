//! Strict canonical codec for `DefinedType`.
//!
//! The value type list shares the typed representation used by
//! `SessionParameter`. The generated `TypeId` and independent `ValueId` stay
//! first-class canonical identities instead of being recovered from opaque XML
//! during native compilation.

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::family::FamilyId;

use super::common::{MetadataDecodeError, MetadataEnvelope};
use super::registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
use super::session_parameter::{
    TypePatternGeneratedPolicy, decode_type_pattern_family, encode_type_pattern_family,
};
use crate::XmlDocument;

const FAMILY: &str = "DefinedType";

/// Registers the exact `DefinedType` codec.
pub fn register_defined_type_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(DefinedTypeCodec {
        family: FamilyId::parse(FAMILY).expect("family id is stable"),
    }))
}

struct DefinedTypeCodec {
    family: FamilyId,
}

impl MetadataFamilyCodec for DefinedTypeCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_type_pattern_family(
            document,
            source,
            path,
            FAMILY,
            TypePatternGeneratedPolicy::DefinedType,
        )
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_type_pattern_family(
            envelope,
            target,
            FAMILY,
            TypePatternGeneratedPolicy::DefinedType,
        )
    }
}

#[cfg(test)]
mod tests {
    use ibcmd_core::value::CanonicalValueKind;

    use super::*;
    use crate::XmlReader;
    use crate::metadata::MetadataRegistry;

    const OBJECT_UUID: &str = "11111111-1111-4111-8111-111111111111";
    const TYPE_UUID: &str = "22222222-2222-4222-8222-222222222222";
    const VALUE_UUID: &str = "33333333-3333-4333-8333-333333333333";

    fn profile(version: &str) -> ProfileId {
        ProfileId::parse(&format!("xml-{version}")).unwrap()
    }

    fn fixture(version: &str, generated_name: &str, value_id: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" version=\"{version}\">\r\n\
\t<DefinedType uuid=\"{OBJECT_UUID}\">\r\n\
\t\t<InternalInfo>\r\n\
\t\t\t<xr:GeneratedType name=\"{generated_name}\" category=\"DefinedType\">\r\n\
\t\t\t\t<xr:TypeId>{TYPE_UUID}</xr:TypeId>\r\n\
\t\t\t\t<xr:ValueId>{value_id}</xr:ValueId>\r\n\
\t\t\t</xr:GeneratedType>\r\n\
\t\t</InternalInfo>\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>SafeMode</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>en</v8:lang><v8:content>Safe mode</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<Type><v8:Type>xs:boolean</v8:Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>120</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers></Type>\r\n\
\t\t</Properties>\r\n\
\t</DefinedType>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    #[test]
    fn generated_value_id_and_type_list_are_typed_for_both_dialects() {
        let mut registry = MetadataRegistry::default();
        register_defined_type_codec(&mut registry).unwrap();
        for version in ["2.20", "2.21"] {
            let input = fixture(version, "DefinedType.SafeMode", VALUE_UUID);
            let document = XmlReader::from_slice(&input).unwrap();
            let envelope = registry
                .decode(
                    &FamilyId::parse(FAMILY).unwrap(),
                    &document,
                    profile(version),
                    ObjectPath::root(),
                )
                .unwrap();
            assert_eq!(envelope.root().generated_types().len(), 1);
            assert_eq!(
                envelope.root().generated_types()[0]
                    .value_id()
                    .unwrap()
                    .to_string(),
                VALUE_UUID
            );
            assert_eq!(
                envelope.root().properties()[3]
                    .value()
                    .as_sequence()
                    .unwrap()
                    .len(),
                2
            );
            assert!(matches!(
                envelope.root().properties()[3].value().kind(),
                CanonicalValueKind::Sequence(_)
            ));
            let target = if version == "2.20" { "2.21" } else { "2.20" };
            let output = registry.encode(&envelope, &profile(target)).unwrap();
            let reparsed = XmlReader::from_slice(&output).unwrap();
            registry
                .decode(
                    &FamilyId::parse(FAMILY).unwrap(),
                    &reparsed,
                    profile(target),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn inconsistent_generated_name_and_nil_value_id_fail_closed() {
        let mut registry = MetadataRegistry::default();
        register_defined_type_codec(&mut registry).unwrap();
        for input in [
            fixture("2.20", "DefinedType.Other", VALUE_UUID),
            fixture(
                "2.20",
                "DefinedType.SafeMode",
                "00000000-0000-0000-0000-000000000000",
            ),
        ] {
            let document = XmlReader::from_slice(&input).unwrap();
            assert!(
                registry
                    .decode(
                        &FamilyId::parse(FAMILY).unwrap(),
                        &document,
                        profile("2.20"),
                        ObjectPath::root(),
                    )
                    .is_err()
            );
        }
    }
}
