//! Base-free, profile-gated codec for `Role/Ext/Rights.xml` native bodies.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::profile::EffectiveProfile;

use super::{BodyProfileError, SelectedBodyProfile};
use crate::compiler::families::native::{
    NativeError, NativeValue, exact_list, exact_token, formatted_list, inflate, inflate_and_parse,
    inline_list, parse_without_bom, raw_deflate, required_list, required_text, required_token,
    serialize, text, token,
};

const LAYOUT_KEY: &str = "bootstrap.body.rights.layout";
const LAYOUT: &str = "rights-v1-raw-deflate-utf8-bom";
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RightsCodecProfile(SelectedBodyProfile);

impl RightsCodecProfile {
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, BodyProfileError> {
        SelectedBodyProfile::from_effective(profile, LAYOUT_KEY, LAYOUT).map(Self)
    }

    pub const fn profile_id(&self) -> &ProfileId {
        self.0.profile_id()
    }

    #[cfg(test)]
    fn fixture() -> Self {
        Self(SelectedBodyProfile::fixture("platform-8.3.27.1989"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleRightsBody {
    native: NativeValue,
    summary: RoleRightsSummary,
}

impl RoleRightsBody {
    pub fn from_model(model: &RoleRightsModel) -> Result<Self, RightsCodecError> {
        let native = native_from_model(model)?;
        let summary = validate_native(&native)?;
        Ok(Self { native, summary })
    }

    pub const fn summary(&self) -> &RoleRightsSummary {
        &self.summary
    }

    pub fn plaintext(&self) -> Result<Vec<u8>, RightsCodecError> {
        serialize(&self.native).map_err(Into::into)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoleRightsSummary {
    pub object_count: usize,
    pub restriction_template_count: usize,
    pub set_for_new_objects: bool,
    pub set_for_attributes_by_default: bool,
    pub independent_rights_of_child_objects: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleRightsModel {
    pub set_for_new_objects: bool,
    pub set_for_attributes_by_default: bool,
    pub independent_rights_of_child_objects: bool,
    pub objects: Vec<RoleObjectRightsModel>,
    pub restriction_templates: Vec<RoleRestrictionTemplateModel>,
}

impl Default for RoleRightsModel {
    fn default() -> Self {
        Self {
            set_for_new_objects: false,
            set_for_attributes_by_default: true,
            independent_rights_of_child_objects: false,
            objects: Vec::new(),
            restriction_templates: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleObjectRightsModel {
    pub reference: RoleObjectReference,
    pub rights: Vec<RoleRightModel>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleObjectReference {
    pub uuid: ObjectUuid,
    pub tail: Vec<RoleObjectReferenceField>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoleObjectReferenceField {
    Token(String),
    TypedSlot { code: i64, type_uuid: ObjectUuid },
}

impl RoleObjectReference {
    pub fn direct(uuid: ObjectUuid) -> Self {
        Self {
            uuid,
            tail: vec![
                RoleObjectReferenceField::Token("0".to_owned()),
                RoleObjectReferenceField::Token("0".to_owned()),
            ],
        }
    }

    pub fn wrapped_standard_attribute(
        owner_uuid: ObjectUuid,
        slot_code: i64,
        type_uuid: ObjectUuid,
    ) -> Self {
        Self {
            uuid: owner_uuid,
            tail: vec![
                RoleObjectReferenceField::Token("1".to_owned()),
                RoleObjectReferenceField::TypedSlot {
                    code: slot_code,
                    type_uuid,
                },
                RoleObjectReferenceField::Token("1".to_owned()),
            ],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleRightModel {
    pub uuid: ObjectUuid,
    pub value: bool,
    pub restriction: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleRestrictionTemplateModel {
    pub name: String,
    pub condition: String,
}

pub fn compile_role_rights(
    profile: &RightsCodecProfile,
    model: &RoleRightsModel,
) -> Result<Vec<u8>, RightsCodecError> {
    let _ = profile;
    raw_deflate(&RoleRightsBody::from_model(model)?.native).map_err(Into::into)
}

pub fn decode_role_rights(
    profile: &RightsCodecProfile,
    blob: &[u8],
) -> Result<RoleRightsBody, RightsCodecError> {
    let _ = profile;
    decode_evidenced_role_rights(blob)
}

pub(crate) fn decode_evidenced_role_rights(
    blob: &[u8],
) -> Result<RoleRightsBody, RightsCodecError> {
    let native = inflate_and_parse(blob).map_err(RightsCodecError::from)?;
    let summary = validate_native(&native)?;
    Ok(RoleRightsBody { native, summary })
}

/// Transitional reader for historical dump fixtures that omitted the BOM.
/// New profile-gated compilation remains strict; this exists only so the
/// legacy export adapter can consume its previously accepted cohort while it
/// delegates structural validation to this codec.
pub(crate) fn decode_compatible_role_rights(
    blob: &[u8],
) -> Result<RoleRightsBody, RightsCodecError> {
    let plain = inflate(blob)?;
    let native = if plain.starts_with(b"\xef\xbb\xbf") {
        crate::compiler::families::native::parse(&plain)?
    } else {
        parse_without_bom(&plain)?
    };
    let summary = validate_native(&native)?;
    Ok(RoleRightsBody { native, summary })
}

fn native_from_model(model: &RoleRightsModel) -> Result<NativeValue, RightsCodecError> {
    let mut object_values = Vec::with_capacity(model.objects.len() + 1);
    object_values.push(token(model.objects.len().to_string()));
    for object in &model.objects {
        let mut reference_values = vec![token("1"), token(object.reference.uuid.to_string())];
        for field in &object.reference.tail {
            reference_values.push(native_reference_field(field)?);
        }
        let reference = inline_list(reference_values);
        let rights = native_rights(&object.rights)?;
        object_values.push(formatted_list(vec![reference, rights], true, vec![1], true));
    }
    let objects = if model.objects.is_empty() {
        inline_list(object_values)
    } else {
        formatted_list(object_values, false, vec![1], true)
    };
    let mut template_values = Vec::with_capacity(model.restriction_templates.len() + 1);
    template_values.push(token(model.restriction_templates.len().to_string()));
    for template in &model.restriction_templates {
        if template.name.is_empty() {
            return Err(RightsCodecError::InvalidShape(
                "Role restriction template name is empty",
            ));
        }
        template_values.push(inline_list(vec![
            text(&template.name),
            text(&template.condition),
        ]));
    }
    let templates = inline_list(template_values);
    Ok(formatted_list(
        vec![
            token("10"),
            objects,
            templates,
            token(bool_root_token(model.set_for_new_objects, true)),
            token(bool_root_token(model.set_for_attributes_by_default, false)),
            token(bool_root_token(
                model.independent_rights_of_child_objects,
                false,
            )),
            token("4294967295"),
        ],
        false,
        vec![1, 2],
        false,
    ))
}

fn native_reference_field(
    field: &RoleObjectReferenceField,
) -> Result<NativeValue, RightsCodecError> {
    match field {
        RoleObjectReferenceField::Token(value) => {
            validate_reference_token(value, "Role object reference tail")?;
            Ok(token(value))
        }
        RoleObjectReferenceField::TypedSlot { code, type_uuid } => Ok(inline_list(vec![
            token(code.to_string()),
            token(type_uuid.to_string()),
        ])),
    }
}

fn native_rights(rights: &[RoleRightModel]) -> Result<NativeValue, RightsCodecError> {
    let restricted = rights
        .iter()
        .filter(|right| right.restriction.is_some())
        .count();
    let mut values = Vec::with_capacity(2 + rights.len() * 2 + restricted);
    if restricted == 0 {
        values.push(token("0"));
    } else {
        values.push(token("1"));
        values.push(token(rights.len().to_string()));
    }
    for right in rights {
        values.push(token(right.uuid.to_string()));
        values.push(token(if right.value { "1" } else { "0" }));
    }
    if restricted != 0 {
        values.push(token(restricted.to_string()));
        for right in rights {
            let Some(condition) = &right.restriction else {
                continue;
            };
            values.push(formatted_list(
                vec![
                    token(right.uuid.to_string()),
                    formatted_list(
                        vec![
                            token("1"),
                            inline_list(vec![token("1"), text(condition), token("0")]),
                        ],
                        false,
                        vec![1],
                        false,
                    ),
                ],
                false,
                vec![1],
                false,
            ));
        }
    }
    Ok(if restricted == 0 {
        inline_list(values)
    } else {
        let first_restriction = 3 + rights.len() * 2;
        formatted_list(values, false, vec![first_restriction], false)
    })
}

fn validate_native(root: &NativeValue) -> Result<RoleRightsSummary, RightsCodecError> {
    let fields = exact_list(root, 7, "Role Rights root")?;
    exact_token(&fields[0], "10", "Role Rights marker")?;
    let objects = required_list(&fields[1], "Role Rights object table")?;
    let object_count = parse_count(objects.first(), "Role Rights object count")?;
    if objects.len() != object_count.saturating_add(1) {
        return Err(RightsCodecError::InvalidShape(
            "Role Rights object count differs from table length",
        ));
    }
    for entry in &objects[1..] {
        let entry = exact_list(entry, 2, "Role Rights object entry")?;
        validate_object_reference(&entry[0])?;
        validate_rights_table(&entry[1])?;
    }
    let templates = required_list(&fields[2], "Role Rights restriction template table")?;
    let restriction_template_count =
        parse_count(templates.first(), "Role Rights restriction template count")?;
    if templates.len() != restriction_template_count.saturating_add(1) {
        return Err(RightsCodecError::InvalidShape(
            "Role Rights template count differs from table length",
        ));
    }
    for template in &templates[1..] {
        let template = exact_list(template, 2, "Role Rights restriction template")?;
        required_text(&template[0], "Role Rights restriction template name")?;
        required_text(&template[1], "Role Rights restriction template condition")?;
    }
    let set_for_new_objects = parse_root_bool(&fields[3], "setForNewObjects")?;
    let set_for_attributes_by_default = parse_root_bool(&fields[4], "setForAttributesByDefault")?;
    let independent_rights_of_child_objects =
        parse_root_bool(&fields[5], "independentRightsOfChildObjects")?;
    exact_token(&fields[6], "4294967295", "Role Rights trailing marker")?;
    Ok(RoleRightsSummary {
        object_count,
        restriction_template_count,
        set_for_new_objects,
        set_for_attributes_by_default,
        independent_rights_of_child_objects,
    })
}

fn validate_object_reference(value: &NativeValue) -> Result<(), RightsCodecError> {
    let fields = required_list(value, "Role Rights object reference")?;
    if fields.len() < 3 {
        return Err(RightsCodecError::InvalidShape(
            "Role Rights object reference has no typed tail",
        ));
    }
    exact_token(&fields[0], "1", "Role Rights object reference marker")?;
    let uuid = required_token(&fields[1], "Role Rights object UUID")?;
    ObjectUuid::parse(uuid).map_err(|_| RightsCodecError::InvalidUuid(uuid.to_owned()))?;
    if uuid == NIL_UUID {
        return Err(RightsCodecError::InvalidShape(
            "Role Rights object UUID is nil",
        ));
    }
    for field in &fields[2..] {
        match field {
            NativeValue::Token(value) => {
                validate_reference_token(value, "Role object reference tail")?;
            }
            NativeValue::List { .. } => {
                let slot = exact_list(field, 2, "Role standard attribute slot")?;
                required_token(&slot[0], "Role standard attribute slot code")?
                    .parse::<i64>()
                    .map_err(|_| {
                        RightsCodecError::InvalidShape("Role standard attribute slot code")
                    })?;
                let type_uuid = required_token(&slot[1], "Role standard attribute type UUID")?;
                ObjectUuid::parse(type_uuid)
                    .map_err(|_| RightsCodecError::InvalidUuid(type_uuid.to_owned()))?;
                if type_uuid == NIL_UUID {
                    return Err(RightsCodecError::InvalidShape(
                        "Role standard attribute type UUID is nil",
                    ));
                }
            }
            NativeValue::Text(_) => {
                return Err(RightsCodecError::InvalidShape(
                    "Role object reference tail contains text",
                ));
            }
        }
    }
    Ok(())
}

fn validate_rights_table(value: &NativeValue) -> Result<(), RightsCodecError> {
    let fields = required_list(value, "Role Rights values")?;
    let marker = required_token(
        fields.first().ok_or(RightsCodecError::InvalidShape(
            "Role Rights values are empty",
        ))?,
        "Role Rights values marker",
    )?;
    let (count, pair_start, restrictions_index): (usize, usize, Option<usize>) = match marker {
        "0" if (fields.len() - 1).is_multiple_of(2) => ((fields.len() - 1) / 2, 1, None),
        "1" => {
            let count = parse_count(fields.get(1), "Role Rights value count")?;
            let index = 2usize
                .checked_add(
                    count
                        .checked_mul(2)
                        .ok_or(RightsCodecError::CountOverflow)?,
                )
                .ok_or(RightsCodecError::CountOverflow)?;
            (count, 2, Some(index))
        }
        _ => {
            return Err(RightsCodecError::InvalidShape(
                "Role Rights values use an unsupported layout",
            ));
        }
    };
    let pairs_end = pair_start
        .checked_add(
            count
                .checked_mul(2)
                .ok_or(RightsCodecError::CountOverflow)?,
        )
        .ok_or(RightsCodecError::CountOverflow)?;
    if fields.len() < pairs_end {
        return Err(RightsCodecError::InvalidShape(
            "Role Rights value table is truncated",
        ));
    }
    let mut right_ids = BTreeSet::new();
    for pair in fields[pair_start..pairs_end].chunks_exact(2) {
        let uuid = required_token(&pair[0], "Role right UUID")?;
        let uuid =
            ObjectUuid::parse(uuid).map_err(|_| RightsCodecError::InvalidUuid(uuid.to_owned()))?;
        if !right_ids.insert(uuid) {
            return Err(RightsCodecError::InvalidShape(
                "Role right UUID is duplicated within an object",
            ));
        }
        parse_role_bool(&pair[1])?;
    }
    if let Some(index) = restrictions_index {
        let restriction_count = parse_count(fields.get(index), "Role restriction count")?;
        validate_restrictions(&fields[index + 1..], restriction_count, &right_ids)?;
    } else if fields.len() != pairs_end {
        return Err(RightsCodecError::InvalidShape(
            "Role Rights values contain an unexpected tail",
        ));
    }
    Ok(())
}

fn validate_restrictions(
    values: &[NativeValue],
    expected: usize,
    right_ids: &BTreeSet<ObjectUuid>,
) -> Result<(), RightsCodecError> {
    if expected == 0 {
        return if values.is_empty() {
            Ok(())
        } else {
            Err(RightsCodecError::InvalidShape(
                "Role restriction table has an unexpected empty wrapper",
            ))
        };
    }
    if values.len() == expected {
        for value in values {
            validate_restriction_pair(value, right_ids)?;
        }
        return Ok(());
    }
    if values.len() != 1 {
        return Err(RightsCodecError::InvalidShape(
            "Role restriction count differs from table length",
        ));
    }
    let wrapped = required_list(&values[0], "Role restriction wrapper")?;
    if wrapped.len() == expected {
        for value in wrapped {
            validate_restriction_pair(value, right_ids)?;
        }
        return Ok(());
    }
    if wrapped.len() == expected.saturating_mul(2) {
        for pair in wrapped.chunks_exact(2) {
            validate_restriction_parts(&pair[0], &pair[1], right_ids)?;
        }
        return Ok(());
    }
    Err(RightsCodecError::InvalidShape(
        "Role restriction count differs from wrapped table length",
    ))
}

fn validate_restriction_pair(
    value: &NativeValue,
    right_ids: &BTreeSet<ObjectUuid>,
) -> Result<(), RightsCodecError> {
    let values = exact_list(value, 2, "Role restriction entry")?;
    validate_restriction_parts(&values[0], &values[1], right_ids)
}

fn validate_restriction_parts(
    right_uuid: &NativeValue,
    payload: &NativeValue,
    right_ids: &BTreeSet<ObjectUuid>,
) -> Result<(), RightsCodecError> {
    let uuid = required_token(right_uuid, "Role restriction right UUID")?;
    let uuid =
        ObjectUuid::parse(uuid).map_err(|_| RightsCodecError::InvalidUuid(uuid.to_owned()))?;
    if !right_ids.contains(&uuid) {
        return Err(RightsCodecError::InvalidShape(
            "Role restriction refers to an absent right UUID",
        ));
    }
    required_list(payload, "Role restriction payload")?;
    Ok(())
}

fn parse_count(
    value: Option<&NativeValue>,
    field: &'static str,
) -> Result<usize, RightsCodecError> {
    required_token(value.ok_or(RightsCodecError::InvalidShape(field))?, field)?
        .parse::<usize>()
        .map_err(|_| RightsCodecError::InvalidShape(field))
}

fn parse_root_bool(value: &NativeValue, field: &'static str) -> Result<bool, RightsCodecError> {
    match required_token(value, field)? {
        "0" | "4294967295" => Ok(false),
        "1" => Ok(true),
        _ => Err(RightsCodecError::InvalidShape(field)),
    }
}

fn parse_role_bool(value: &NativeValue) -> Result<bool, RightsCodecError> {
    match required_token(value, "Role right value")? {
        "-1" | "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(RightsCodecError::InvalidShape("Role right value")),
    }
}

fn bool_root_token(value: bool, legacy_false: bool) -> &'static str {
    if value {
        "1"
    } else if legacy_false {
        "4294967295"
    } else {
        "0"
    }
}

fn validate_reference_token(value: &str, field: &'static str) -> Result<(), RightsCodecError> {
    if value.is_empty()
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || matches!(byte, b'{' | b'}' | b',' | b'"'))
    {
        return Err(RightsCodecError::InvalidShape(field));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RightsCodecError {
    Profile(BodyProfileError),
    Native(String),
    InvalidShape(&'static str),
    InvalidUuid(String),
    CountOverflow,
}

impl Display for RightsCodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => Display::fmt(source, formatter),
            Self::Native(reason) => write!(
                formatter,
                "native Role Rights codec rejected data: {reason}"
            ),
            Self::InvalidShape(field) => write!(formatter, "invalid Role Rights body: {field}"),
            Self::InvalidUuid(value) => write!(formatter, "invalid Role Rights UUID `{value}`"),
            Self::CountOverflow => formatter.write_str("Role Rights count arithmetic overflow"),
        }
    }
}

impl Error for RightsCodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            _ => None,
        }
    }
}

impl From<BodyProfileError> for RightsCodecError {
    fn from(source: BodyProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<NativeError> for RightsCodecError {
    fn from(source: NativeError) -> Self {
        Self::Native(source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::families::native::deflate_bytes;

    const MINIMAL: &[u8] = b"\xef\xbb\xbf{10,\r\n{0},\r\n{0},4294967295,1,0,4294967295}";
    const ONE_OBJECT: &[u8] = b"\xef\xbb\xbf{10,\r\n{1,\r\n{\r\n{1,78db0888-c681-48f2-8166-b202aace5148,0,0},\r\n{0,aa6448f2-be0f-42ea-ba26-1af7f52b5b65,1}\r\n}\r\n},\r\n{0},4294967295,1,0,4294967295}";

    #[test]
    fn minimal_fixture_decodes_and_reencodes_byte_exact() {
        let profile = RightsCodecProfile::fixture();
        let blob = deflate_bytes(MINIMAL).unwrap();
        let decoded = decode_role_rights(&profile, &blob).unwrap();
        assert_eq!(decoded.plaintext().unwrap(), MINIMAL);
        assert_eq!(decoded.summary().object_count, 0);
        assert_eq!(raw_deflate(&decoded.native).unwrap(), blob);
    }

    #[test]
    fn base_free_model_matches_evidenced_one_object_layout() {
        let profile = RightsCodecProfile::fixture();
        let model = RoleRightsModel {
            objects: vec![RoleObjectRightsModel {
                reference: RoleObjectReference::direct(
                    ObjectUuid::parse("78db0888-c681-48f2-8166-b202aace5148").unwrap(),
                ),
                rights: vec![RoleRightModel {
                    uuid: ObjectUuid::parse("aa6448f2-be0f-42ea-ba26-1af7f52b5b65").unwrap(),
                    value: true,
                    restriction: None,
                }],
            }],
            ..RoleRightsModel::default()
        };
        let blob = compile_role_rights(&profile, &model).unwrap();
        let decoded = decode_role_rights(&profile, &blob).unwrap();
        assert_eq!(decoded.plaintext().unwrap(), ONE_OBJECT);
        assert_eq!(decoded.summary().object_count, 1);
    }

    #[test]
    fn restrictions_roundtrip_without_a_base_blob() {
        let profile = RightsCodecProfile::fixture();
        let model = RoleRightsModel {
            objects: vec![RoleObjectRightsModel {
                reference: RoleObjectReference::direct(
                    ObjectUuid::parse("e29da7eb-5fcc-4067-82ca-ae24a1fb314a").unwrap(),
                ),
                rights: vec![RoleRightModel {
                    uuid: ObjectUuid::parse("aa6448f2-be0f-42ea-ba26-1af7f52b5b65").unwrap(),
                    value: true,
                    restriction: Some("WHERE Allowed = TRUE".to_owned()),
                }],
            }],
            restriction_templates: vec![RoleRestrictionTemplateModel {
                name: "OnlyAllowed".to_owned(),
                condition: "WHERE Allowed = TRUE".to_owned(),
            }],
            ..RoleRightsModel::default()
        };
        let first = compile_role_rights(&profile, &model).unwrap();
        let decoded = decode_role_rights(&profile, &first).unwrap();
        let second = raw_deflate(&decoded.native).unwrap();
        assert_eq!(first, second);
        assert_eq!(decoded.summary().restriction_template_count, 1);
    }

    #[test]
    fn wrapped_standard_attribute_reference_is_base_free() {
        let profile = RightsCodecProfile::fixture();
        let model = RoleRightsModel {
            objects: vec![RoleObjectRightsModel {
                reference: RoleObjectReference::wrapped_standard_attribute(
                    ObjectUuid::parse("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa").unwrap(),
                    -13,
                    ObjectUuid::parse("03f171e8-326f-41c6-9fa5-932a0b12cddf").unwrap(),
                ),
                rights: vec![RoleRightModel {
                    uuid: ObjectUuid::parse("b7bab52d-c1b1-4bd8-8276-02db08d42352").unwrap(),
                    value: false,
                    restriction: None,
                }],
            }],
            ..RoleRightsModel::default()
        };
        let blob = compile_role_rights(&profile, &model).unwrap();
        let decoded = decode_role_rights(&profile, &blob).unwrap();
        let plain = String::from_utf8(decoded.plaintext().unwrap()).unwrap();
        assert!(plain.contains(
            "{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,1,{-13,03f171e8-326f-41c6-9fa5-932a0b12cddf},1}"
        ));
    }

    #[test]
    fn duplicate_right_uuid_is_rejected_before_emission() {
        let profile = RightsCodecProfile::fixture();
        let right = RoleRightModel {
            uuid: ObjectUuid::parse("aa6448f2-be0f-42ea-ba26-1af7f52b5b65").unwrap(),
            value: true,
            restriction: None,
        };
        let model = RoleRightsModel {
            objects: vec![RoleObjectRightsModel {
                reference: RoleObjectReference::direct(
                    ObjectUuid::parse("78db0888-c681-48f2-8166-b202aace5148").unwrap(),
                ),
                rights: vec![right.clone(), right],
            }],
            ..RoleRightsModel::default()
        };
        assert!(matches!(
            compile_role_rights(&profile, &model),
            Err(RightsCodecError::InvalidShape(
                "Role right UUID is duplicated within an object"
            ))
        ));
    }
}
