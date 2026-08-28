//! Evidence-honest decode of the Configuration `<Properties>` span the
//! compiler cannot yet decode field-by-field (`DefaultRoles` through
//! `DefaultConstantsForm`). See MINI-GATE-A-CONFIG-PROPS-01.
//!
//! The span's coordinates are *tuple field indices* of the 61-field
//! Configuration `<Properties>` tuple, not byte offsets into the config-body
//! text: a configuration whose Name, Synonym, roles or references differ in
//! length from the evidenced reference shifts every byte in the tuple, but
//! shifts no field index. Six fields carry a corpus-proven single-byte enum
//! or boolean and are emitted typed; three more carry the default report
//! forms; `UsedMobileApplicationFunctionalities` is read from its own
//! declared count elsewhere. Everything still undecoded in the span is
//! emitted verbatim from
//! `ibcmd_schema::configuration_properties_evidenced_default_block_policy`
//! only after a field-by-field comparison proves this corpus's tuple matches
//! the evidenced all-default reference in every field that carries such a
//! value. Any arity surprise, unrecognized digit, or field mismatch fails
//! closed -- this module never emits a guess.

use std::sync::LazyLock;

use super::split_1c_braced_fields;
use crate::module_blob::decode_base64_mime;

/// Base64 of the retained, all-default `dcs-area-style-item-uuid` config-body
/// unpacked tuple text (sha256
/// `da8070b0adfc3e71a695e5d670ab171c85a0ee7ab9aeeb2c6bd3e3ed76abb853`,
/// 6513 bytes): the deterministic `cf extract` of storage element
/// `0f7275e8-b27a-44e3-a033-d5a9ca5da59a` (the Configuration root record
/// named by `root`) from that corpus's manifest-pinned
/// `configuration.cf.b64` (sha256 `bd64046b...`). The sole fail-closed
/// comparison reference for the still-undecoded fields of the span.
const EVIDENCED_DEFAULT_REFERENCE_B64: &str = include_str!(
    "../../tests/fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid/config-body-unpacked.bin.b64"
);

static EVIDENCED_DEFAULT_REFERENCE_TEXT: LazyLock<String> = LazyLock::new(|| {
    let bytes = decode_base64_mime(EVIDENCED_DEFAULT_REFERENCE_B64.trim())
        .expect("bundled evidenced-default config-body reference is valid base64");
    String::from_utf8(bytes).expect("bundled evidenced-default config-body reference is UTF-8")
});

/// The reference's own Configuration `<Properties>` tuple, split into its
/// declared fields the same way every probe's is.
static EVIDENCED_DEFAULT_REFERENCE_FIELDS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let text: &'static str = &EVIDENCED_DEFAULT_REFERENCE_TEXT;
    let start = text
        .find("{68,")
        .expect("the evidenced reference opens its Properties tuple with `{68,`");
    split_1c_braced_fields(text, start)
        .expect("the evidenced reference's Properties tuple is well-formed")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConfigurationPropertiesEvidencedFields {
    pub(crate) include_help_in_contents_xml: &'static str,
    pub(crate) use_managed_form_in_ordinary_application_xml: &'static str,
    pub(crate) use_ordinary_form_in_managed_application_xml: &'static str,
    pub(crate) modality_use_mode_xml: &'static str,
    pub(crate) synchronous_platform_extension_and_add_in_call_use_mode_xml: &'static str,
    pub(crate) interface_compatibility_mode_xml: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfigurationPropertiesEvidenceError {
    /// The tuple does not have the reference's field count at all, so none of
    /// the proven field indices mean anything here. Not my case -- the caller
    /// keeps its existing per-field behaviour.
    UnexpectedTupleArity { found: usize },
    /// A field the reference spells as a single byte is not a single byte
    /// here. Again a shape mismatch, not a content disagreement.
    UnexpectedFieldShape { field: &'static str },
    /// A field this module cannot decode is not even written in the same
    /// syntactic class as the reference's (a bare scalar where the reference
    /// spells a uuid, and so on). That is a different tuple dialect, not a
    /// configuration that disagrees -- not my case either.
    UnexpectedFieldClass { field: usize },
    /// A single byte at a proven coordinate that no evidenced corpus has ever
    /// shown. Fail closed rather than guess at an unobserved enum member.
    UnrecognizedDigit { field: &'static str, byte: u8 },
    /// A field this module cannot decode disagrees with the evidenced
    /// all-default reference, so the verbatim segments covering it are no
    /// longer proven for this corpus. Fail closed.
    UnprovenFieldMismatch { field: usize },
}

/// The syntactic class a Properties tuple field is written in. Two tuples
/// that spell the same field in different classes are different dialects of
/// the record, not two configurations that disagree about a value -- the
/// flat/SQL-sourced shapes this crate also reads write a bare `0` where the
/// CF container writes a uuid or a nested group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldClass {
    Uuid,
    Group,
    Quoted,
    Scalar,
}

fn field_class(field: &str) -> FieldClass {
    if field.starts_with('{') {
        FieldClass::Group
    } else if field.starts_with('"') {
        FieldClass::Quoted
    } else if field.len() == 36
        && field.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
    {
        FieldClass::Uuid
    } else {
        FieldClass::Scalar
    }
}

fn field_at<'a>(
    fields: &'a [&'a str],
    index: usize,
    name: &'static str,
) -> Result<&'a str, ConfigurationPropertiesEvidenceError> {
    fields
        .get(index)
        .map(|field| field.trim())
        .ok_or(ConfigurationPropertiesEvidenceError::UnexpectedFieldShape { field: name })
}

/// Reads the single ASCII byte a proven coordinate spells, then maps it
/// through the policy's evidenced value table.
fn typed_field(
    fields: &[&str],
    index: usize,
    name: &'static str,
    map: impl Fn(u8) -> Option<&'static str>,
) -> Result<&'static str, ConfigurationPropertiesEvidenceError> {
    let field = field_at(fields, index, name)?;
    let bytes = field.as_bytes();
    if bytes.len() != 1 {
        return Err(ConfigurationPropertiesEvidenceError::UnexpectedFieldShape { field: name });
    }
    map(bytes[0]).ok_or(ConfigurationPropertiesEvidenceError::UnrecognizedDigit {
        field: name,
        byte: bytes[0],
    })
}

/// Parses the six typed fields and fail-closed-verifies every field of the
/// Configuration `<Properties>` tuple this module still cannot decode against
/// the evidenced all-default reference. `fields` is that tuple already split
/// into its declared fields (`configuration_root_property_fields`).
pub(crate) fn parse_configuration_properties_evidenced_default_block(
    fields: &[&str],
) -> Result<ConfigurationPropertiesEvidencedFields, ConfigurationPropertiesEvidenceError> {
    let policy = ibcmd_schema::configuration_properties_evidenced_default_block_policy();
    let reference = &*EVIDENCED_DEFAULT_REFERENCE_FIELDS;
    if fields.len() != reference.len() {
        return Err(ConfigurationPropertiesEvidenceError::UnexpectedTupleArity {
            found: fields.len(),
        });
    }

    let include_help_in_contents_xml = typed_field(
        fields,
        policy.include_help_in_contents_tuple_field(),
        "IncludeHelpInContents",
        |digit| policy.include_help_in_contents_xml(digit),
    )?;
    let use_managed_form_in_ordinary_application_xml = typed_field(
        fields,
        policy.use_managed_form_in_ordinary_application_tuple_field(),
        "UseManagedFormInOrdinaryApplication",
        |digit| policy.use_managed_form_in_ordinary_application_xml(digit),
    )?;
    let use_ordinary_form_in_managed_application_xml = typed_field(
        fields,
        policy.use_ordinary_form_in_managed_application_tuple_field(),
        "UseOrdinaryFormInManagedApplication",
        |digit| policy.use_ordinary_form_in_managed_application_xml(digit),
    )?;
    let modality_use_mode_xml = typed_field(
        fields,
        policy.modality_use_mode_tuple_field(),
        "ModalityUseMode",
        |digit| policy.modality_use_mode_xml(digit),
    )?;
    let interface_compatibility_mode_xml = typed_field(
        fields,
        policy.interface_compatibility_mode_tuple_field(),
        "InterfaceCompatibilityMode",
        |digit| policy.interface_compatibility_mode_xml(digit),
    )?;
    let synchronous_platform_extension_and_add_in_call_use_mode_xml = typed_field(
        fields,
        policy.synchronous_platform_extension_and_add_in_call_use_mode_tuple_field(),
        "SynchronousPlatformExtensionAndAddInCallUseMode",
        |digit| policy.synchronous_platform_extension_and_add_in_call_use_mode_xml(digit),
    )?;

    for &index in policy.unproven_tuple_fields() {
        let (Some(ours), Some(theirs)) = (fields.get(index), reference.get(index)) else {
            return Err(ConfigurationPropertiesEvidenceError::UnexpectedTupleArity {
                found: fields.len(),
            });
        };
        let (ours, theirs) = (ours.trim(), theirs.trim());
        if ours == theirs {
            continue;
        }
        if field_class(ours) != field_class(theirs) {
            return Err(ConfigurationPropertiesEvidenceError::UnexpectedFieldClass {
                field: index,
            });
        }
        return Err(ConfigurationPropertiesEvidenceError::UnprovenFieldMismatch { field: index });
    }

    Ok(ConfigurationPropertiesEvidencedFields {
        include_help_in_contents_xml,
        use_managed_form_in_ordinary_application_xml,
        use_ordinary_form_in_managed_application_xml,
        modality_use_mode_xml,
        synchronous_platform_extension_and_add_in_call_use_mode_xml,
        interface_compatibility_mode_xml,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const T1_BASE_B64: &str = include_str!(
        "../../tests/fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid/config-body-unpacked.bin.b64"
    );
    const T3_ENUM_GROUP_B64: &str = include_str!(
        "../../tests/fixtures/native-evidence/8.3.27.2214/configuration-properties-enum-group/config-body-unpacked.bin.b64"
    );
    const INCLUDE_HELP_IN_CONTENTS_B64: &str = include_str!(
        "../../tests/fixtures/native-evidence/8.3.27.2214/configuration-property-include-help-in-contents/config-body-unpacked.bin.b64"
    );
    const MODALITY_USE_MODE_B64: &str = include_str!(
        "../../tests/fixtures/native-evidence/8.3.27.2214/configuration-property-modality-use-mode/config-body-unpacked.bin.b64"
    );

    fn load(b64: &str) -> Vec<u8> {
        decode_base64_mime(b64.trim()).unwrap()
    }

    /// The production entry point takes the Properties tuple already split
    /// into its declared fields; these fixtures are the whole config body, so
    /// split them the same way `configuration_root_property_fields` does.
    fn fields_of(text: &[u8]) -> Vec<&str> {
        let text = std::str::from_utf8(text).expect("every evidenced fixture is UTF-8");
        let start = text
            .find("{68,")
            .expect("every evidenced config body opens its Properties tuple with `{68,`");
        split_1c_braced_fields(text, start).expect("the Properties tuple is well-formed")
    }

    fn parse(
        text: &[u8],
    ) -> Result<ConfigurationPropertiesEvidencedFields, ConfigurationPropertiesEvidenceError> {
        parse_configuration_properties_evidenced_default_block(&fields_of(text))
    }

    /// Byte ranges of the 61 top-level fields of the config-body tuple's
    /// Configuration `<Properties>` container, in `text`'s own coordinates.
    fn properties_tuple_field_ranges(text: &[u8]) -> Vec<(usize, usize)> {
        let start = text
            .windows(4)
            .position(|window| window == b"{68,")
            .expect("every evidenced config body opens its Properties tuple with `{68,`");
        let mut ranges = Vec::new();
        let mut depth = 1usize;
        let mut quoted = false;
        let mut field_start = start + 1;
        let mut index = start + 1;
        while index < text.len() {
            match text[index] {
                b'"' if quoted && text.get(index + 1) == Some(&b'"') => index += 2,
                b'"' => {
                    quoted = !quoted;
                    index += 1;
                }
                _ if quoted => index += 1,
                b'{' => {
                    depth += 1;
                    index += 1;
                }
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        ranges.push((field_start, index));
                        return ranges;
                    }
                    index += 1;
                }
                b',' if depth == 1 => {
                    ranges.push((field_start, index));
                    field_start = index + 1;
                    index += 1;
                }
                _ => index += 1,
            }
        }
        panic!("the Properties tuple is never left unterminated in an evidenced corpus");
    }

    /// REVERSE-GATE-R2-CONFIG-PROJECTION-01: the load direction addresses the
    /// six proven coordinates by *tuple field index*, not by absolute byte
    /// offset. This pins that translation against the very bytes the offsets
    /// were proven on: each declared field must be exactly the single byte at
    /// the declared offset.
    #[test]
    fn evidenced_byte_offsets_land_in_the_declared_tuple_fields() {
        let policy = ibcmd_schema::configuration_properties_evidenced_default_block_policy();
        let text = load(T1_BASE_B64);
        let ranges = properties_tuple_field_ranges(&text);
        assert_eq!(ranges.len(), 61);
        for (offset, field) in [
            (428, policy.include_help_in_contents_tuple_field()),
            (
                623,
                policy.use_managed_form_in_ordinary_application_tuple_field(),
            ),
            (
                625,
                policy.use_ordinary_form_in_managed_application_tuple_field(),
            ),
            (867, policy.modality_use_mode_tuple_field()),
            (906, policy.interface_compatibility_mode_tuple_field()),
            (
                2669,
                policy.synchronous_platform_extension_and_add_in_call_use_mode_tuple_field(),
            ),
        ] {
            let (start, end) = ranges[field];
            assert_eq!(
                (start, end),
                (offset, offset + 1),
                "tuple field {field} is not the single byte at offset {offset}"
            );
        }
    }

    /// One table, one fact: every one of the 61 Properties tuple fields is
    /// accounted for exactly once -- decoded from its own coordinate, proven
    /// by identity with the evidenced reference, or observed to drive no
    /// `<Properties>` output at all. A field that fell through all three
    /// would be emitted from a segment nothing proves.
    #[test]
    fn every_properties_tuple_field_is_accounted_for_exactly_once() {
        let policy = ibcmd_schema::configuration_properties_evidenced_default_block_policy();
        // Fields the readers in `refs.rs` decode themselves: the record
        // header (Name/Synonym/Comment), NamePrefix, DefaultRunMode, the five
        // localized properties, DefaultStyle's sibling DefaultLanguage,
        // Vendor/Version/UpdateCatalogAddress, the four settings storages,
        // ConfigurationExtensionCompatibilityMode, the two ordinary-form
        // booleans, the three default report forms, UsePurposes, the three
        // enum bytes, DefaultRoles, CompatibilityMode, the mobile
        // functionalities and AllowedIncomingShareRequestTypes.
        let decoded = [
            1usize, 2, 3, 4, 5, 6, 7, 8, 10, 13, 14, 15, 16, 22, 23, 24, 25, 26, 28, 29, 30, 31,
            32, 33, 36, 38, 39, 41, 43, 53, 59,
        ];
        let mut seen = vec![0usize; 61];
        for index in decoded
            .iter()
            .copied()
            .chain(policy.unproven_tuple_fields().iter().copied())
            .chain(
                policy
                    .tuple_fields_without_properties_output()
                    .iter()
                    .copied(),
            )
        {
            seen[index] += 1;
        }
        assert!(
            seen.iter().all(|count| *count == 1),
            "fields covered twice or not at all: {:?}",
            seen.iter()
                .enumerate()
                .filter(|(_, count)| **count != 1)
                .collect::<Vec<_>>()
        );
    }

    /// The compiler emits the all-default
    /// `<UsedMobileApplicationFunctionalities>` block by turning it into the
    /// numeric IDs the platform's own tuple marks as used. Those IDs come
    /// from the bundled reference, so pin them there.
    #[test]
    fn evidenced_reference_marks_exactly_the_declared_mobile_ids() {
        let policy = ibcmd_schema::configuration_properties_evidenced_default_block_policy();
        let text = load(T1_BASE_B64);
        let ranges = properties_tuple_field_ranges(&text);
        let (start, end) = ranges[53];
        let field = std::str::from_utf8(&text[start..end])
            .unwrap()
            .replace("\r\n", "");
        let mut used = Vec::new();
        for entry in field.trim_start_matches("{2,38,").split("},") {
            let entry = entry.trim_start_matches('{').trim_end_matches('}');
            let Some((id, flag)) = entry.split_once(',') else {
                continue;
            };
            if flag.trim() == "1" {
                used.push(id.trim().parse::<u32>().unwrap());
            }
        }
        assert_eq!(
            used,
            policy.used_mobile_application_functionalities_default_tuple_ids()
        );
    }

    #[test]
    fn all_default_base_corpus_decodes_to_the_platform_default_values() {
        let text = load(T1_BASE_B64);
        let fields = parse(&text).unwrap();
        assert_eq!(fields.include_help_in_contents_xml, "false");
        assert_eq!(fields.use_managed_form_in_ordinary_application_xml, "false");
        assert_eq!(fields.use_ordinary_form_in_managed_application_xml, "false");
        assert_eq!(fields.modality_use_mode_xml, "DontUse");
        assert_eq!(
            fields.synchronous_platform_extension_and_add_in_call_use_mode_xml,
            "DontUse"
        );
        assert_eq!(
            fields.interface_compatibility_mode_xml,
            "TaxiEnableVersion8_2"
        );
    }

    /// Positive per-field case: a single-field probe corpus's own typed
    /// field reflects its known non-default value, and the fail-closed
    /// unproven-field comparison still passes (this probe changes only the
    /// six known coordinates, nothing else).
    #[test]
    fn single_field_probe_decodes_its_own_non_default_value() {
        let text = load(INCLUDE_HELP_IN_CONTENTS_B64);
        let fields = parse(&text).unwrap();
        assert_eq!(fields.include_help_in_contents_xml, "true");
        // The other five stay at their platform default in this probe.
        assert_eq!(fields.use_managed_form_in_ordinary_application_xml, "false");
        assert_eq!(fields.modality_use_mode_xml, "DontUse");
    }

    /// MINI-GATE-A-CONFIG-PROPS-01 gate negative: mutating a byte in a field
    /// the module still cannot decode must fail closed with a typed error,
    /// never a silently-wrong or silently-truncated XML emission. Byte 355
    /// sits inside tuple field 11, a nil-uuid field no reader touches and
    /// that the verbatim segments therefore stand or fall with.
    #[test]
    fn mutating_an_unproven_field_fails_closed() {
        let mut text = load(T1_BASE_B64);
        let mutated_byte = text[355];
        text[355] = if mutated_byte == b'0' { b'9' } else { b'0' };
        assert_eq!(
            parse(&text),
            Err(ConfigurationPropertiesEvidenceError::UnprovenFieldMismatch { field: 11 })
        );
    }

    /// Gate negative, second flavor: the same must hold in a corpus that
    /// already carries non-default values at the six known coordinates, so
    /// the mismatch cannot be confused with one of them. Byte 2700 is inside
    /// tuple field 46, another nil-uuid field no reader decodes.
    #[test]
    fn mutating_an_unproven_field_in_a_non_default_probe_still_fails_closed() {
        let mut text = load(T3_ENUM_GROUP_B64);
        let mutated_byte = text[2700];
        text[2700] = if mutated_byte == b'0' { b'9' } else { b'0' };
        assert_eq!(
            parse(&text),
            Err(ConfigurationPropertiesEvidenceError::UnprovenFieldMismatch { field: 46 })
        );
    }

    /// MINI-GATE-A-CONFIG-PROPS-01 gate positive-mutation: mutating one of
    /// the six known-coordinate bytes changes exactly that field's typed XML
    /// value, correctly, without disturbing the other five or the
    /// fail-closed comparison.
    #[test]
    fn mutating_a_mapped_byte_changes_only_that_fields_xml_correctly() {
        let mut text = load(T1_BASE_B64);
        assert_eq!(text[428], b'0');
        text[428] = b'1';
        let fields = parse(&text).unwrap();
        assert_eq!(fields.include_help_in_contents_xml, "true");
        assert_eq!(fields.use_managed_form_in_ordinary_application_xml, "false");
        assert_eq!(fields.modality_use_mode_xml, "DontUse");
    }

    /// Same gate, for one of the enum-typed fields (index-based, not
    /// boolean): mutating tuple field 36 from `'2'` (DontUse) through `'1'`
    /// (UseWithWarnings, the value «1С:Управление торговлей 11.5.27.75»
    /// carries) to `'0'` (Use) must change only `ModalityUseMode`'s XML.
    #[test]
    fn mutating_the_modality_use_mode_field_changes_only_that_fields_xml() {
        for (digit, expected) in [(b'0', "Use"), (b'1', "UseWithWarnings")] {
            let mut text = load(T1_BASE_B64);
            assert_eq!(text[867], b'2');
            text[867] = digit;
            let fields = parse(&text).unwrap();
            assert_eq!(fields.modality_use_mode_xml, expected);
            assert_eq!(
                fields.interface_compatibility_mode_xml,
                "TaxiEnableVersion8_2"
            );
            assert_eq!(
                fields.synchronous_platform_extension_and_add_in_call_use_mode_xml,
                "DontUse"
            );
        }
    }

    /// An unrecognized digit at a known coordinate (never observed by any
    /// evidenced corpus) must fail closed rather than guess at an unproven
    /// enum member.
    #[test]
    fn unrecognized_digit_at_a_known_coordinate_fails_closed() {
        let mut text = load(T1_BASE_B64);
        text[867] = b'7';
        assert_eq!(
            parse(&text),
            Err(ConfigurationPropertiesEvidenceError::UnrecognizedDigit {
                field: "ModalityUseMode",
                byte: b'7',
            })
        );
    }

    /// Independent single-field probe for the enum convention: confirms
    /// `ModalityUseMode`'s own corpus (not just the group probe) decodes
    /// correctly and in isolation from the other five fields. The
    /// header-length-shift case itself (a corpus whose Configuration
    /// Name/Synonym differ in length from the base reference, like
    /// `dcs-form-list-settings-server-state`) is exercised end-to-end in
    /// `tests/cf_export.rs`, which has that corpus's own retained CF; no
    /// module-level bundled fixture with a differently-shaped header exists
    /// here.
    #[test]
    fn modality_use_mode_single_field_probe_decodes_in_isolation() {
        let text = load(MODALITY_USE_MODE_B64);
        let fields = parse(&text).unwrap();
        assert_eq!(fields.modality_use_mode_xml, "Use");
        assert_eq!(fields.include_help_in_contents_xml, "false");
    }

    /// A tuple with a different field count is not this module's case: the
    /// proven coordinates are field indices, and they mean nothing in a tuple
    /// that does not have the reference's shape.
    #[test]
    fn a_differently_sized_tuple_is_not_my_case() {
        let text = load(T1_BASE_B64);
        let mut fields = fields_of(&text);
        fields.pop();
        assert_eq!(
            parse_configuration_properties_evidenced_default_block(&fields),
            Err(ConfigurationPropertiesEvidenceError::UnexpectedTupleArity { found: 60 })
        );
    }
}
