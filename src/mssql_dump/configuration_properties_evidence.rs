//! Evidence-honest decode of the Configuration `<Properties>` span the
//! compiler cannot yet decode field-by-field (`DefaultRoles` through
//! `DefaultConstantsForm`). See MINI-GATE-A-CONFIG-PROPS-01: six fields have
//! corpus-proven single-byte offsets into the raw config-body tuple text and
//! are decoded and emitted typed; everything else in the span is emitted
//! verbatim from `ibcmd_schema::configuration_properties_evidenced_default_block_policy`
//! only after a byte-range comparison proves this exact corpus's tuple
//! matches the evidenced all-default reference everywhere outside the six
//! known offsets. Any anchor miss, unrecognized digit, or byte-range
//! mismatch fails closed -- this module never emits a guess.

use std::sync::LazyLock;

use crate::module_blob::decode_base64_mime;

/// Base64 of the retained, all-default `dcs-area-style-item-uuid` config-body
/// unpacked tuple text (sha256
/// `da8070b0adfc3e71a695e5d670ab171c85a0ee7ab9aeeb2c6bd3e3ed76abb853`,
/// 6513 bytes): the deterministic `cf extract` of storage element
/// `0f7275e8-b27a-44e3-a033-d5a9ca5da59a` (the Configuration root record
/// named by `root`) from that corpus's manifest-pinned
/// `configuration.cf.b64` (sha256 `bd64046b...`). The sole fail-closed
/// comparison reference for the "unmapped" span.
const EVIDENCED_DEFAULT_REFERENCE_B64: &str = include_str!(
    "../../tests/fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid/config-body-unpacked.bin.b64"
);

static EVIDENCED_DEFAULT_REFERENCE_BYTES: LazyLock<Vec<u8>> = LazyLock::new(|| {
    decode_base64_mime(EVIDENCED_DEFAULT_REFERENCE_B64.trim())
        .expect("bundled evidenced-default config-body reference is valid base64")
});

/// Marks the boundary between the variable-length header (config uuid, Name,
/// Synonym -- already decoded correctly elsewhere) and the stable-layout
/// remainder of the tuple. Present exactly once in every evidenced corpus;
/// its position gives the constant byte SHIFT a different corpus's header
/// length induces on every later offset.
const HEADER_END_ANCHOR: &[u8] = b"},\"\",0,";

const INCLUDE_HELP_SUFFIX: &[u8] =
    b",\"\",\"\",\"\",1,\r\n{0,0},1,\r\n{0,0},1,00000000-0000-0000-0000-000000000000";
const USE_MANAGED_FORM_PREFIX: &[u8] = b"80327,\r\n{0,0},";
const INTERFACE_COMPAT_SUFFIX: &[u8] =
    b",\r\n{0,0},\r\n{28,\r\n{\r\n{\"#\",e4c53f94-e5f7-4a34-8c10-218bd811cae1,";
const MODALITY_INTERFACE_SEP: &[u8] = b",00000000-0000-0000-0000-000000000000,";
const SYNCHRONOUS_PREFIX: &[u8] = b"\r\n}\r\n},";
const SYNCHRONOUS_SUFFIX: &[u8] = b",\"\",80327,1,0,00000000-0000-0000-0000-000000000000,";

/// Start (inclusive) and end (exclusive) of the fail-closed comparison
/// window, in the *base* reference's own byte coordinates -- translated by
/// the header-length SHIFT for every other corpus. Proven empirically: every
/// byte in [200, 4266) of the base reference, outside the six known-offset
/// positions, is identical across every evidenced corpus (all five
/// single-field probes, both group probes, and `dcs-form-list-settings-server-state`'s
/// differently-shaped config uuid/Name/Synonym header).
const UNMAPPED_RANGE_START: usize = 200;
const UNMAPPED_RANGE_END: usize = 4266;

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
    HeaderAnchorNotFound,
    HeaderAnchorNotUnique { count: usize },
    FieldAnchorNotFound { field: &'static str },
    FieldAnchorNotUnique { field: &'static str, count: usize },
    UnrecognizedDigit { field: &'static str, byte: u8 },
    UnmappedRangeMismatch { relative_offset: usize },
    UnmappedRangeOutOfBounds,
}

fn find_unique(
    haystack: &[u8],
    needle: &[u8],
    field: &'static str,
) -> Result<usize, ConfigurationPropertiesEvidenceError> {
    let mut matches = haystack
        .windows(needle.len().max(1))
        .enumerate()
        .filter(|(_, window)| *window == needle)
        .map(|(index, _)| index);
    let Some(first) = matches.next() else {
        return Err(ConfigurationPropertiesEvidenceError::FieldAnchorNotFound { field });
    };
    let count = 1 + matches.count();
    if count != 1 {
        return Err(ConfigurationPropertiesEvidenceError::FieldAnchorNotUnique { field, count });
    }
    Ok(first)
}

fn header_end_offset(text: &[u8]) -> Result<usize, ConfigurationPropertiesEvidenceError> {
    let mut matches = text
        .windows(HEADER_END_ANCHOR.len())
        .enumerate()
        .filter(|(_, window)| *window == HEADER_END_ANCHOR)
        .map(|(index, _)| index);
    let Some(first) = matches.next() else {
        return Err(ConfigurationPropertiesEvidenceError::HeaderAnchorNotFound);
    };
    let count = 1 + matches.count();
    if count != 1 {
        return Err(ConfigurationPropertiesEvidenceError::HeaderAnchorNotUnique { count });
    }
    Ok(first)
}

/// Reads `text[position]`, requiring exactly one ASCII byte.
fn digit_at(
    text: &[u8],
    position: usize,
    field: &'static str,
) -> Result<u8, ConfigurationPropertiesEvidenceError> {
    text.get(position)
        .copied()
        .ok_or(ConfigurationPropertiesEvidenceError::FieldAnchorNotFound { field })
}

/// Parses the six typed fields and fail-closed-verifies the remainder of the
/// Configuration Properties tuple against the evidenced all-default
/// reference. `text` is the config-body tuple's own decoded bytes (the same
/// bytes `parse_configuration_properties_from_text` already receives).
pub(crate) fn parse_configuration_properties_evidenced_default_block(
    text: &[u8],
) -> Result<ConfigurationPropertiesEvidencedFields, ConfigurationPropertiesEvidenceError> {
    let policy = ibcmd_schema::configuration_properties_evidenced_default_block_policy();

    // -- 428: IncludeHelpInContents --
    let suffix_428 = find_unique(text, INCLUDE_HELP_SUFFIX, "IncludeHelpInContents")?;
    if suffix_428 == 0 {
        return Err(ConfigurationPropertiesEvidenceError::FieldAnchorNotFound {
            field: "IncludeHelpInContents",
        });
    }
    let field_428 = suffix_428 - 1;
    let include_help_in_contents_xml = policy
        .include_help_in_contents_xml(digit_at(text, field_428, "IncludeHelpInContents")?)
        .ok_or(ConfigurationPropertiesEvidenceError::UnrecognizedDigit {
            field: "IncludeHelpInContents",
            byte: text[field_428],
        })?;

    // -- 623/625: UseManagedFormInOrdinaryApplication / UseOrdinaryFormInManagedApplication --
    let prefix_623 = find_unique(
        text,
        USE_MANAGED_FORM_PREFIX,
        "UseManagedFormInOrdinaryApplication",
    )?;
    let field_623 = prefix_623 + USE_MANAGED_FORM_PREFIX.len();
    let field_625 = field_623 + 2;
    if text.get(field_623 + 1) != Some(&b',') || text.get(field_625).is_none() {
        return Err(ConfigurationPropertiesEvidenceError::FieldAnchorNotFound {
            field: "UseOrdinaryFormInManagedApplication",
        });
    }
    let use_managed_form_in_ordinary_application_xml = policy
        .use_managed_form_in_ordinary_application_xml(digit_at(
            text,
            field_623,
            "UseManagedFormInOrdinaryApplication",
        )?)
        .ok_or(ConfigurationPropertiesEvidenceError::UnrecognizedDigit {
            field: "UseManagedFormInOrdinaryApplication",
            byte: text[field_623],
        })?;
    let use_ordinary_form_in_managed_application_xml = policy
        .use_ordinary_form_in_managed_application_xml(digit_at(
            text,
            field_625,
            "UseOrdinaryFormInManagedApplication",
        )?)
        .ok_or(ConfigurationPropertiesEvidenceError::UnrecognizedDigit {
            field: "UseOrdinaryFormInManagedApplication",
            byte: text[field_625],
        })?;

    // -- 867/906: ModalityUseMode / InterfaceCompatibilityMode --
    let suffix_906 = find_unique(text, INTERFACE_COMPAT_SUFFIX, "InterfaceCompatibilityMode")?;
    if suffix_906 == 0 {
        return Err(ConfigurationPropertiesEvidenceError::FieldAnchorNotFound {
            field: "InterfaceCompatibilityMode",
        });
    }
    let field_906 = suffix_906 - 1;
    let sep_start = field_906.checked_sub(MODALITY_INTERFACE_SEP.len()).ok_or(
        ConfigurationPropertiesEvidenceError::FieldAnchorNotFound {
            field: "ModalityUseMode",
        },
    )?;
    if &text[sep_start..field_906] != MODALITY_INTERFACE_SEP {
        return Err(ConfigurationPropertiesEvidenceError::FieldAnchorNotFound {
            field: "ModalityUseMode",
        });
    }
    let field_867 = sep_start.checked_sub(1).ok_or(
        ConfigurationPropertiesEvidenceError::FieldAnchorNotFound {
            field: "ModalityUseMode",
        },
    )?;
    let modality_use_mode_xml = policy
        .modality_use_mode_xml(digit_at(text, field_867, "ModalityUseMode")?)
        .ok_or(ConfigurationPropertiesEvidenceError::UnrecognizedDigit {
            field: "ModalityUseMode",
            byte: text[field_867],
        })?;
    let interface_compatibility_mode_xml = policy
        .interface_compatibility_mode_xml(digit_at(text, field_906, "InterfaceCompatibilityMode")?)
        .ok_or(ConfigurationPropertiesEvidenceError::UnrecognizedDigit {
            field: "InterfaceCompatibilityMode",
            byte: text[field_906],
        })?;

    // -- 2669: SynchronousPlatformExtensionAndAddInCallUseMode --
    let suffix_2669 = find_unique(
        text,
        SYNCHRONOUS_SUFFIX,
        "SynchronousPlatformExtensionAndAddInCallUseMode",
    )?;
    if suffix_2669 == 0 {
        return Err(ConfigurationPropertiesEvidenceError::FieldAnchorNotFound {
            field: "SynchronousPlatformExtensionAndAddInCallUseMode",
        });
    }
    let field_2669 = suffix_2669 - 1;
    let prefix_start = field_2669.checked_sub(SYNCHRONOUS_PREFIX.len()).ok_or(
        ConfigurationPropertiesEvidenceError::FieldAnchorNotFound {
            field: "SynchronousPlatformExtensionAndAddInCallUseMode",
        },
    )?;
    if &text[prefix_start..field_2669] != SYNCHRONOUS_PREFIX {
        return Err(ConfigurationPropertiesEvidenceError::FieldAnchorNotFound {
            field: "SynchronousPlatformExtensionAndAddInCallUseMode",
        });
    }
    let synchronous_platform_extension_and_add_in_call_use_mode_xml = policy
        .synchronous_platform_extension_and_add_in_call_use_mode_xml(digit_at(
            text,
            field_2669,
            "SynchronousPlatformExtensionAndAddInCallUseMode",
        )?)
        .ok_or(ConfigurationPropertiesEvidenceError::UnrecognizedDigit {
            field: "SynchronousPlatformExtensionAndAddInCallUseMode",
            byte: text[field_2669],
        })?;

    // -- fail-closed comparison of everything else in the span --
    verify_unmapped_range(
        text,
        &[
            field_428, field_623, field_625, field_867, field_906, field_2669,
        ],
    )?;

    Ok(ConfigurationPropertiesEvidencedFields {
        include_help_in_contents_xml,
        use_managed_form_in_ordinary_application_xml,
        use_ordinary_form_in_managed_application_xml,
        modality_use_mode_xml,
        synchronous_platform_extension_and_add_in_call_use_mode_xml,
        interface_compatibility_mode_xml,
    })
}

fn verify_unmapped_range(
    text: &[u8],
    // Positions of the six typed fields *within `text`'s own coordinate
    // space* (i.e. as returned by the anchor searches above, already
    // resolved against a possibly-shifted header) -- NOT base-reference
    // coordinates.
    known_positions_in_probe: &[usize],
) -> Result<(), ConfigurationPropertiesEvidenceError> {
    let reference = &*EVIDENCED_DEFAULT_REFERENCE_BYTES;
    let base_header_end = header_end_offset(reference)?;
    let probe_header_end = header_end_offset(text)?;
    let shift = probe_header_end as isize - base_header_end as isize;

    for base_offset in UNMAPPED_RANGE_START..UNMAPPED_RANGE_END {
        let probe_offset = base_offset as isize + shift;
        if probe_offset < 0 {
            return Err(ConfigurationPropertiesEvidenceError::UnmappedRangeOutOfBounds);
        }
        let probe_offset = probe_offset as usize;
        if known_positions_in_probe.contains(&probe_offset) {
            continue;
        }
        let (Some(reference_byte), Some(probe_byte)) =
            (reference.get(base_offset), text.get(probe_offset))
        else {
            return Err(ConfigurationPropertiesEvidenceError::UnmappedRangeOutOfBounds);
        };
        if reference_byte != probe_byte {
            return Err(
                ConfigurationPropertiesEvidenceError::UnmappedRangeMismatch {
                    relative_offset: base_offset,
                },
            );
        }
    }
    Ok(())
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
        let fields = parse_configuration_properties_evidenced_default_block(&text).unwrap();
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
    /// unmapped-range comparison still passes (this probe changes only the
    /// six known offsets, nothing else).
    #[test]
    fn single_field_probe_decodes_its_own_non_default_value() {
        let text = load(INCLUDE_HELP_IN_CONTENTS_B64);
        let fields = parse_configuration_properties_evidenced_default_block(&text).unwrap();
        assert_eq!(fields.include_help_in_contents_xml, "true");
        // The other five stay at their platform default in this probe.
        assert_eq!(fields.use_managed_form_in_ordinary_application_xml, "false");
        assert_eq!(fields.modality_use_mode_xml, "DontUse");
    }

    /// MINI-GATE-A-CONFIG-PROPS-01 gate negative: mutating a byte in the
    /// span the compiler still cannot decode field-by-field (i.e. outside
    /// all six known offsets) must fail closed with a typed error, never a
    /// silently-wrong or silently-truncated XML emission.
    #[test]
    fn mutating_an_unmapped_byte_fails_closed() {
        let mut text = load(T1_BASE_B64);
        // Byte 1000 sits well inside the fail-closed comparison window
        // (200..4266) and outside all six known offsets (428/623/625/
        // 867/906/2669) -- part of the still-unmapped
        // `UsedMobileApplicationFunctionalities` span.
        let mutated_byte = text[1000];
        text[1000] = if mutated_byte == b'0' { b'9' } else { b'0' };
        let result = parse_configuration_properties_evidenced_default_block(&text);
        assert_eq!(
            result,
            Err(
                ConfigurationPropertiesEvidenceError::UnmappedRangeMismatch {
                    relative_offset: 1000
                }
            )
        );
    }

    /// Gate negative, second flavor: mutating a byte adjacent to (but not
    /// inside) `UNMAPPED_RANGE_START..UNMAPPED_RANGE_END` is out of the
    /// verified window and must not be silently accepted as a false
    /// positive -- confirmed here by picking an offset just past the six
    /// known positions in the enum-group probe (which already carries
    /// non-default values at 867/906/2669) so the byte flip is unambiguous.
    #[test]
    fn mutating_an_unmapped_byte_in_a_non_default_probe_still_fails_closed() {
        let mut text = load(T3_ENUM_GROUP_B64);
        let probe_position = 3000;
        let mutated_byte = text[probe_position];
        text[probe_position] = if mutated_byte == b'0' { b'9' } else { b'0' };
        let result = parse_configuration_properties_evidenced_default_block(&text);
        assert_eq!(
            result,
            Err(
                ConfigurationPropertiesEvidenceError::UnmappedRangeMismatch {
                    relative_offset: probe_position
                }
            )
        );
    }

    /// MINI-GATE-A-CONFIG-PROPS-01 gate positive-mutation: mutating one of
    /// the six known-offset bytes changes exactly that field's typed XML
    /// value, correctly, without disturbing the other five or the
    /// fail-closed comparison.
    #[test]
    fn mutating_a_mapped_byte_changes_only_that_fields_xml_correctly() {
        let mut text = load(T1_BASE_B64);
        assert_eq!(text[428], b'0');
        text[428] = b'1';
        let fields = parse_configuration_properties_evidenced_default_block(&text).unwrap();
        assert_eq!(fields.include_help_in_contents_xml, "true");
        assert_eq!(fields.use_managed_form_in_ordinary_application_xml, "false");
        assert_eq!(fields.modality_use_mode_xml, "DontUse");
    }

    /// Same gate, for one of the enum-typed fields (index-based, not
    /// boolean): mutating offset 867 from `'2'` (DontUse) to `'0'` (Use)
    /// must change only `ModalityUseMode`'s XML.
    #[test]
    fn mutating_the_modality_use_mode_offset_changes_only_that_fields_xml() {
        let mut text = load(T1_BASE_B64);
        assert_eq!(text[867], b'2');
        text[867] = b'0';
        let fields = parse_configuration_properties_evidenced_default_block(&text).unwrap();
        assert_eq!(fields.modality_use_mode_xml, "Use");
        assert_eq!(
            fields.interface_compatibility_mode_xml,
            "TaxiEnableVersion8_2"
        );
        assert_eq!(
            fields.synchronous_platform_extension_and_add_in_call_use_mode_xml,
            "DontUse"
        );
    }

    /// An unrecognized digit at a known offset (never observed by any
    /// evidenced corpus) must fail closed rather than guess at an unproven
    /// enum member.
    #[test]
    fn unrecognized_digit_at_a_known_offset_fails_closed() {
        let mut text = load(T1_BASE_B64);
        text[867] = b'7';
        let result = parse_configuration_properties_evidenced_default_block(&text);
        assert_eq!(
            result,
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
        let fields = parse_configuration_properties_evidenced_default_block(&text).unwrap();
        assert_eq!(fields.modality_use_mode_xml, "Use");
        assert_eq!(fields.include_help_in_contents_xml, "false");
    }
}
