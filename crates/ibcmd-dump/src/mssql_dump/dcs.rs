//! The data-composition (DCS) normalizer is the `ibcmd-form-body` crate's
//! (`dcs.rs`); `mssql_dump`'s modules and the root's template exporters reach
//! its items by their old names and paths through these re-exports, at their
//! old visibility. The tests that decode a template through the root's
//! `compiler::bodies::dcs` stay here.

// The tests below read `mssql_dump`'s names, as when this file held the code.
#[cfg(test)]
use super::{BTreeMap, MetadataSourceFailureClass, ProfileId};

pub(super) use ibcmd_form_body::ANY_IB_REF_TYPE_ID;
pub(crate) use ibcmd_form_body::normalize_data_composition_schema_template_documents_with_profiles;
// The form reader, in the crate, reads the rest there; the root's tests read
// these by their old paths.
#[cfg(test)]
pub(crate) use ibcmd_form_body::{
    FormListSettingsChildKind, FormListSettingsChildTransliteration,
    transliterate_form_list_settings_child_document,
};

// The DCS type index is the `ibcmd-form-body` crate's (`FormParseContext`
// holds it); it is built there and in `compiler::bodies::dcs`.
pub(crate) use ibcmd_form_body::{DcsTypeIndex, DcsTypeResolution};

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn decode_base64_fixture(encoded: &str) -> Vec<u8> {
        let mut output = Vec::new();
        let mut quartet = [0u8; 4];
        let mut length = 0usize;
        for byte in encoded.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
            quartet[length] = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => 64,
                _ => panic!("invalid fixture base64 byte {byte}"),
            };
            length += 1;
            if length == 4 {
                output.push((quartet[0] << 2) | (quartet[1] >> 4));
                if quartet[2] != 64 {
                    output.push((quartet[1] << 4) | (quartet[2] >> 2));
                }
                if quartet[3] != 64 {
                    output.push((quartet[2] << 6) | quartet[3]);
                }
                length = 0;
            }
        }
        assert_eq!(length, 0, "fixture base64 must contain complete quartets");
        output
    }

    #[test]
    fn platform_8_3_27_xml_2_20_dcs_body_exports_byte_exact() {
        let packed = include_bytes!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/dcs-core/raw/\
             f4db0f6c-34f4-4449-995d-6265516e5fa8.0.deflate"
        );
        let expected = include_bytes!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/dcs-core/native/Reports/\
             DcsCorpus/Templates/MainSchema/Ext/Template.xml"
        );

        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            packed,
        )
        .expect("platform-attested DCS body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("platform-attested DCS body must be exportable through the live codec");

        assert_eq!(actual, expected);
    }

    /// A real production `DataSetQuery` schema -- the shape no closed typed
    /// cohort describes and that the general storage-to-source
    /// transliteration exists for.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `20db535c-d9a7-4a81-98b5-06295e8f518d.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `5df7cb6cb94efb1c7ef3e0ea1405553bd2cdf2895cc35ceacc31e7bf9130319f`;
    /// the expectation is the platform's own
    /// `DataProcessors/УправлениеВыгрузкамиВБидзаар/Templates/УсловияОтбораНоменклатуры/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `1d04899ee7fe61cddaa3efb51892d91fcea94db481db34e3ecaf0a2bfa3fc6b7`.
    #[test]
    fn platform_query_data_set_body_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-query-data-set/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-query-data-set/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "5df7cb6cb94efb1c7ef3e0ea1405553bd2cdf2895cc35ceacc31e7bf9130319f"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "1d04899ee7fe61cddaa3efb51892d91fcea94db481db34e3ecaf0a2bfa3fc6b7"
        );

        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a production DataSetQuery schema must export through the live codec");

        assert_eq!(actual, expected);
    }

    /// The empty `<dcsset:settings .../>` variant: its inline fragment used to
    /// be spliced as `.../ xmlns:dcsset="..."...>`, which is not well-formed,
    /// so our own analyzer rejected our own output.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `15f6a89d-53a1-47f7-967e-973c5966caf8.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `a7e61a3156c7a2fa3302f7f77277d7f1a64abda10eb0519292bf4a1829a6f169`;
    /// the expectation is the platform's own
    /// `Reports/РезультатыТестирования/Templates/Макет/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `c1b1771520a0c8153e3c2f8d8381fa90f02c78338dc9e936eaef68caf2dffe67`.
    #[test]
    fn platform_empty_inline_settings_body_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-empty-inline-settings/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-empty-inline-settings/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "a7e61a3156c7a2fa3302f7f77277d7f1a64abda10eb0519292bf4a1829a6f169"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "c1b1771520a0c8153e3c2f8d8381fa90f02c78338dc9e936eaef68caf2dffe67"
        );

        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("an empty settings variant must export through the live codec");

        assert_eq!(actual, expected);
    }

    /// The terminal `SchemaFile` carrying a real report's area template, with
    /// its table cell's appearance held in the envelope's side table and
    /// selected by ordinal.
    ///
    /// Storage writes `<dcsat:appIndex>0</dcsat:appIndex>` inside the cell and
    /// a `<appearance xmlns="...area-template" xsi:type="TableCellAppearance">`
    /// child of `SchemaFile` itself; the source document writes that child's
    /// items inline as `<dcsat:appearance>` -- no ordinal, no discriminator,
    /// and no declaration, because `dcsat` is already bound by the
    /// `AreaTemplate` element the fragment sits under. This is the join and
    /// the inherited-prefix rule in their simplest form.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `2bf9c338-d31e-467e-8281-d1d6a3a6b2e2.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `f26cb51838fbdd3d1ab60381ce694d7cecb61e034793bd4c11aaf48dd149b734`;
    /// the expectation is the platform's own
    /// `Reports/ДлительностьОтложенногоОбновления/Templates/ОсновнаяСхемаКомпоновкиДанных/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `8cb4cddc8ecc053d74cf349ba3d4f78fe2d6f080ea966a9a3488184b2a2360a2`.
    #[test]
    fn platform_terminal_area_template_appearance_index_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-appearance-index/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-appearance-index/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "f26cb51838fbdd3d1ab60381ce694d7cecb61e034793bd4c11aaf48dd149b734"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "8cb4cddc8ecc053d74cf349ba3d4f78fe2d6f080ea966a9a3488184b2a2360a2"
        );

        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        let terminal = body.documents()[body.documents().len() - 1];
        let terminal = std::str::from_utf8(terminal).expect("the terminal document is UTF-8");
        assert!(
            terminal.contains("<dcsat:appIndex>0</dcsat:appIndex>")
                && terminal.contains(
                    "<appearance xmlns=\"http://v8.1c.ru/8.1/data-composition-system/\
                     area-template\" xsi:type=\"TableCellAppearance\">"
                ),
            "the fixture is the out-of-line appearance shape this exercises"
        );

        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a template-carrying terminal must export through the live codec");

        assert_eq!(actual, expected);
        let actual = String::from_utf8(actual).expect("the source document is UTF-8");
        assert!(
            !actual.contains("appIndex") && actual.contains("<dcsat:appearance>"),
            "the ordinal is resolved away and the appearance written inline"
        );
    }

    /// The same join where the appearance carries typed values, so both prefix
    /// rules the source direction applies inside it are visible at once.
    ///
    /// The storage `<value xmlns:d4p1="http://v8.1c.ru/8.1/data/ui" ...>` binds
    /// a generated prefix to a namespace the source root does declare, so the
    /// declaration is dropped and the value moves onto `v8ui`; a generated
    /// prefix bound to one the root does *not* declare is renumbered by the
    /// element's depth in the target document, not the storage one. Two
    /// templates select two different side-table entries by ordinal, and the
    /// schema's `fieldTemplate`/`groupTemplate` children ride along in
    /// document order.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `cc43c5a3-5e72-4f37-9e1a-016c70f94d85.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `fc95f8e0bb82f09f15a741c2e5bd2a6755448a3c7abe17b9815ebe0d7c76bdd2`;
    /// the expectation is the platform's own
    /// `InformationRegisters/МестаПримененияЭлементовКонструктораВидовПродукцииИС/Templates/МакетНастроекМестПрименения/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `d2f420275b235890de29152125610f97bc995114e98c62a834bd7e54201efbde`.
    #[test]
    fn platform_terminal_area_template_style_prefixes_export_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-style-prefixes/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-style-prefixes/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "fc95f8e0bb82f09f15a741c2e5bd2a6755448a3c7abe17b9815ebe0d7c76bdd2"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "d2f420275b235890de29152125610f97bc995114e98c62a834bd7e54201efbde"
        );

        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        // The style item this configuration's Settings document names by uuid.
        let object_refs = BTreeMap::from([(
            "fa2a9ef2-00a1-44f4-a82c-6c7288dd62dc".to_string(),
            "StyleItem.ВажнаяНадписьШрифт".to_string(),
        )]);
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &object_refs,
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("two ordinals into the side table must export through the live codec");

        assert_eq!(actual, expected);
        let rendered = String::from_utf8(actual).expect("the source document is UTF-8");
        assert!(
            rendered.contains("<dcscor:value xsi:type=\"v8ui:VerticalAlign\">Bottom"),
            "a generated prefix bound to a root-declared namespace collapses \
             onto the root's own and loses its declaration"
        );
        assert!(
            rendered
                .contains("<dcscor:value xsi:type=\"dcscor:DataCompositionTextPlacementType\">"),
            "an unprefixed xsi:type picks up the prefix of the default \
             namespace it resolved through"
        );
        assert_eq!(
            rendered.matches("<dcsat:appearance>").count(),
            2,
            "both ordinals resolved into their own inline appearance"
        );
    }

    /// Everything about the side table that the platform's own bytes do not
    /// account for is refused rather than guessed.
    ///
    /// The join is by ordinal and nothing else, so an ordinal that names no
    /// entry has no appearance to inline; an entry no ordinal names would be
    /// dropped, since the source document has no side table to put it in, and
    /// dropping stored content is exactly the silent wrongness this direction
    /// exists to avoid; and an entry whose discriminator is not the evidenced
    /// `TableCellAppearance` is a shape whose inline spelling nothing here has
    /// seen. All three keep the fragment out of the export entirely.
    #[test]
    fn terminal_side_table_shapes_without_evidence_fail_closed() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-appearance-index/raw-packed.bin.b64"
        )));
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        let documents = body.documents();
        let terminal = std::str::from_utf8(documents[documents.len() - 1])
            .expect("the terminal document is UTF-8");

        for (what, mutated) in [
            (
                "an ordinal past the end of the side table",
                terminal.replace(
                    "<dcsat:appIndex>0</dcsat:appIndex>",
                    "<dcsat:appIndex>1</dcsat:appIndex>",
                ),
            ),
            (
                "a side-table entry no table cell selects",
                terminal.replace("\t\t\t\t\t\t<dcsat:appIndex>0</dcsat:appIndex>\r\n", ""),
            ),
            (
                "a discriminator beyond TableCellAppearance",
                terminal.replace(
                    "xsi:type=\"TableCellAppearance\"",
                    "xsi:type=\"FutureAppearance\"",
                ),
            ),
        ] {
            assert_ne!(mutated, terminal, "{what}: the mutation must bite");
            let mut mutated_documents = documents.clone();
            let last = mutated_documents.len() - 1;
            mutated_documents[last] = mutated.as_bytes();
            let error = normalize_data_composition_schema_template_documents_with_profiles(
                &mutated_documents,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .err()
            .unwrap_or_else(|| panic!("{what} must not export"));
            assert_eq!(
                error.class(),
                MetadataSourceFailureClass::Unsupported,
                "{what}: the refusal is `no evidence describes this`"
            );
        }
    }

    /// A `{data/ui}Font`'s `ref` attribute naming a configuration `StyleItem`
    /// by storage uuid, in both places this configuration writes one: inside
    /// the terminal document's inlined appearance and inside a standalone
    /// `Settings` document's own.
    ///
    /// The platform pre-declares `xmlns:style` on the very element carrying
    /// the reference in the first position and inherits the settings root's
    /// declaration in the second, and writes `ref="style:<Name>"` in both.
    /// A uuid the object-reference index cannot name is refused rather than
    /// written in its `0:<uuid>` storage spelling, which the platform never
    /// produces.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `e0ef648e-3e13-4aba-bd7f-206fbb42572d.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `31b6cd7d91e8314f2f3614ba4daa94b1007d092459eb78dbcd0f344984679084`;
    /// the expectation is the platform's own
    /// `InformationRegisters/ДоступныеЗначенияЭлементовКонструктораВидовПродукцииИС/Templates/МакетНастроекДоступныхЗначений/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `0d1894d8624dd00fd566afcc37d6816cf71dd223f4124f1cbcff97502c200082`.
    #[test]
    fn platform_terminal_area_template_style_item_ref_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-style-item-ref/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-style-item-ref/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "31b6cd7d91e8314f2f3614ba4daa94b1007d092459eb78dbcd0f344984679084"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "0d1894d8624dd00fd566afcc37d6816cf71dd223f4124f1cbcff97502c200082"
        );

        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        // The two style items this template's documents name by uuid: one on
        // a `Font`'s `ref` attribute, one as a `Color` value's character data.
        let object_refs = BTreeMap::from([
            (
                "fa2a9ef2-00a1-44f4-a82c-6c7288dd62dc".to_string(),
                "StyleItem.ВажнаяНадписьШрифт".to_string(),
            ),
            (
                "fe63db47-3ece-4518-b6a2-0b6499f60a61".to_string(),
                "StyleItem.ТекстЗапрещеннойЯчейкиЦвет".to_string(),
            ),
        ]);
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &object_refs,
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a named style item must export through the live codec");

        assert_eq!(actual, expected);
        let rendered = String::from_utf8(actual).expect("the source document is UTF-8");
        assert_eq!(
            rendered.matches("ref=\"style:ВажнаяНадписьШрифт\"").count(),
            2,
            "both the inlined appearance and the Settings document name it"
        );
        assert!(
            !rendered.contains("0:fa2a9ef2"),
            "the storage uuid spelling never reaches the source document"
        );

        // Without the name, both positions refuse rather than fall back to the
        // storage spelling.
        let error = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect_err("an unnamed style item has no source spelling");
        assert_eq!(error.class(), MetadataSourceFailureClass::Unsupported);
    }

    /// Four external Settings documents in one envelope -- twice the count the
    /// clean-room corpus could exhibit, and the shape the attested-range gate
    /// refused outright. The header declares the count and the framing formula
    /// is uniform in it; this pins that reading it beats enumerating it, on
    /// the platform's own bytes.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `542a53f5-56c5-4ca9-90d1-d689532c69fd.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `90db75a46813f9f36e8abcc32499cf8bf5591af8b7c09770a11456569428e11f`;
    /// the expectation is the platform's own
    /// `Reports/ПродлениеСрокаДействияЭлектронныхПодписей/Templates/Макет/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `afaa6a8bc76bb34a3d5bf4650c60b0a7e740625fd690ec8da039675578eecbc5`.
    #[test]
    fn platform_multi_variant_settings_body_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-multi-variant-settings/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-multi-variant-settings/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "90db75a46813f9f36e8abcc32499cf8bf5591af8b7c09770a11456569428e11f"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "afaa6a8bc76bb34a3d5bf4650c60b0a7e740625fd690ec8da039675578eecbc5"
        );

        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        assert_eq!(
            body.documents().len(),
            6,
            "the header declares four Settings documents between the two SchemaFiles"
        );
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a four-variant envelope must export through the live codec");

        assert_eq!(actual, expected);
    }

    /// `AnyIBRef` is a protocol identifier outside the generated-type index,
    /// so it reaches the transliteration through the same unconditional seed
    /// `data_composition_type_id_xml` applies on the typed path -- and it is
    /// spelled `<v8:TypeSet>`, not `<v8:Type>`.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `cd6770d5-cd3c-47af-bc46-c865993adf63.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `4b70aeb7032569c77a2508fc932e0d27c03704191193f4e946922001f3adc9e4`;
    /// the expectation is the platform's own
    /// `Reports/МестаИспользованияСсылок/Templates/ОсновнаяСхемаКомпоновкиДанных/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `98d644bc92ee7fb5b100277478f90d075ab22bea2b358e6e2e86bbb34c07d6f9`.
    #[test]
    fn platform_any_ib_ref_type_set_body_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-any-ib-ref-type-set/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-any-ib-ref-type-set/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "4b70aeb7032569c77a2508fc932e0d27c03704191193f4e946922001f3adc9e4"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "98d644bc92ee7fb5b100277478f90d075ab22bea2b358e6e2e86bbb34c07d6f9"
        );

        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("an AnyIBRef valueType must export through the live codec");

        assert_eq!(actual, expected);
        assert!(
            String::from_utf8_lossy(&actual).contains(":AnyIBRef</v8:TypeSet>"),
            "a reference family is spelled as a TypeSet"
        );
    }

    /// The index-driven half of the same gap: `DocumentRef` is a
    /// `DcsTypeResolution::TypeSet` entry the type index does carry, which the
    /// transliteration had no map to read it from and so refused by name.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `3c305f89-e453-46c7-987b-f01dc964efdb.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `d0e430b3ab056b79ca70bd3c810bf0a82c006fb2d637bc41d9217a6b15f9b5fe`;
    /// the expectation is the platform's own
    /// `Reports/ДвиженияДокумента/Templates/ПустаяСхемаКомпоновкиДанных/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `4dcc90d8002faeabc911551b2ce388148feaab71b1d3283cd0deb01b85b31944`.
    #[test]
    fn platform_document_ref_type_set_body_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-document-ref-type-set/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-document-ref-type-set/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "d0e430b3ab056b79ca70bd3c810bf0a82c006fb2d637bc41d9217a6b15f9b5fe"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "4dcc90d8002faeabc911551b2ce388148feaab71b1d3283cd0deb01b85b31944"
        );

        let mut type_index = DcsTypeIndex::new();
        type_index.insert(
            "38bfd075-3e63-4aaa-a93e-94521380d579".to_owned(),
            DcsTypeResolution::TypeSet {
                qname: "cfg:DocumentRef".to_owned(),
            },
        );
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &type_index,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a DocumentRef valueType must export through the live codec");

        assert_eq!(actual, expected);

        // Without the index entry the uuid resolves to nothing at all, and
        // that must still fail closed rather than be spelled either way.
        let refused = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &DcsTypeIndex::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        );
        assert!(
            refused.is_err(),
            "an unresolvable TypeId must not be guessed as either spelling"
        );
    }

    #[test]
    fn platform_data_parameters_source_owned_body_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-data-parameters-source-owned/raw/",
            "f4db0f6c-34f4-4449-995d-6265516e5fa8.0.deflate.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-data-parameters-source-owned/native-template.xml.b64"
        )));

        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested source-owned DCS body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect(
            "platform-attested source-owned DCS body must be exportable through the live codec",
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_multi_variant_envelope_materializes_settings_positionally_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-multi-variant-envelope/raw/",
            "f4db0f6c-34f4-4449-995d-6265516e5fa8.0.deflate.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-multi-variant-envelope/native-template.xml.b64"
        )));

        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested multi-variant body must decode");
        let documents = body.documents();
        let source_profile = ProfileId::parse("provider:mssql-legacy").unwrap();
        let target_profile = ProfileId::parse("xml-2.20").unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &documents,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &source_profile,
            &target_profile,
        )
        .expect("platform-attested multi-variant DCS body must be exportable");

        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_type_id_reference_body_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-typeid-reference/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-typeid-reference/native-template.xml.b64"
        )));
        let mut type_index = BTreeMap::new();
        type_index.insert(
            "488c0ffa-ef24-480c-a420-3bd2736317f9".to_owned(),
            DcsTypeResolution::Type {
                qname: "cfg:CatalogRef.FilterProbe".to_owned(),
            },
        );
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let documents = body.documents();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &documents,
            &type_index,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_query_union_link_body_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link/native-template.xml.b64"
        )));
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    /// Gate test for the query-union-link fallback's `reference_types.is_empty()`
    /// bug (DCS-LEGACY-REMOVAL-01 phase-1 finding, candidate a): the inner-schema
    /// typed parser rejects any DataSetQuery/DataSetUnion/dataSetLink shape (it
    /// only admits DataSetObject), and this fixture's own type_index -- the
    /// same evidenced `CatalogRef.FilterProbe` construction
    /// `platform_type_id_reference_body_exports_byte_exact_through_common_codec`
    /// already proves, built the same way the real route's
    /// `build_metadata_type_indexes_from_texts` would produce it -- is
    /// non-empty. Before the fix this combination (inner-schema parse
    /// fails + `reference_types` non-empty) skipped the query-union-link
    /// fallback entirely and hard failed; the fix removes the accidental
    /// gate so the fallback is always attempted on inner-schema-parse
    /// failure, independent of `reference_types`. This reuses the base
    /// `dcs-query-union-link` corpus's own genuine bytes (its shape is
    /// admitted by the query-union-link parser) with an injected type_index
    /// entry the corpus itself does not need, isolating the gate-condition
    /// fix from the query-union-link parser's own unrelated fixed-shape
    /// strictness -- see the module doc comment on this test for why the
    /// evidenced `dcs-query-union-link-typeid` corpus cannot be used here.
    #[test]
    fn platform_query_union_link_exports_byte_exact_with_non_empty_type_index() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link/native-template.xml.b64"
        )));
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        // A type_index entry this corpus's own content never references:
        // before the fix, its mere non-emptiness alone was enough to skip
        // the query-union-link fallback and hard fail; after the fix,
        // export must still succeed byte-exactly because the fallback is
        // attempted regardless of what (or whether) `reference_types`
        // resolves.
        let mut type_index = BTreeMap::new();
        type_index.insert(
            "488c0ffa-ef24-480c-a420-3bd2736317f9".to_owned(),
            DcsTypeResolution::Type {
                qname: "cfg:CatalogRef.FilterProbe".to_owned(),
            },
        );
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &type_index,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("query-union-link fallback must still be attempted with a non-empty type_index");
        assert_eq!(actual, expected);
    }

    /// `dcs-query-union-link-typeid` (evidenced fixture, immutable manifest,
    /// DCS-QUERY-SECOND-FIELD-01): `QueryRows` carries a second, typed field
    /// (`Owner`) transplanting the exact evidenced current-config TypeId
    /// construction `dcs-typeid-reference`'s DataSetObject field already
    /// proved. `DcsQueryUnionLinkPolicy::query_children()`/`parse_query`
    /// (ibcmd-schema/ibcmd-xml) now admit exactly two evidenced
    /// `DataSetQuery` shapes, selected by child count: the original
    /// single-field cohort, or this cohort's five-child shape. With a
    /// real, non-empty `type_index` (built the same way the live route
    /// would), the full common codec now exports this corpus byte-exact.
    #[test]
    fn platform_query_union_link_typeid_body_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link-typeid/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link-typeid/native-template.xml.b64"
        )));
        // manifest.json: retained.packed_body.sha256 / retained.native_template.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "f51f756a382994936bcf748a62d665cf9a53cfc05153e75401d3df6e8b4ee3ca"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "5ce6f74897ce0428f2898fcf54ed542dcd23af6aafdbd0f331a0fde1e18bb3be"
        );
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        // The same evidenced TypeId -> qname construction
        // `platform_type_id_reference_body_exports_byte_exact_through_common_codec`
        // already proves, built the same way the real route's
        // `build_metadata_type_indexes_from_texts` would produce it.
        let mut type_index = BTreeMap::new();
        type_index.insert(
            "488c0ffa-ef24-480c-a420-3bd2736317f9".to_owned(),
            DcsTypeResolution::Type {
                qname: "cfg:CatalogRef.FilterProbe".to_owned(),
            },
        );
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &type_index,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("two-field DataSetQuery must now export through the common codec");
        assert_eq!(actual, expected);
    }

    /// A third field is outside both closed typed cohorts (which admit exactly
    /// two evidenced `DataSetQuery` child-lists, by count), so it reaches the
    /// general storage-to-source transliteration instead -- and is exported,
    /// not skipped. What the typed parsers refuse is still refused *by them*;
    /// the document is then accounted for from its own bytes. The
    /// transliterated result must reproduce the document itself: same
    /// `DataSetQuery`, all three fields, in order.
    #[test]
    fn query_union_link_third_field_transliterates_instead_of_failing_closed() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link-typeid/raw-packed.bin.b64"
        )));
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let documents = body.documents();
        let primary = std::str::from_utf8(documents[0]).unwrap();
        // Duplicate the evidenced typed `Owner` field so `QueryRows` carries
        // three fields (name, field, field, field, dataSource, query --
        // six children), a cardinality neither evidenced list admits.
        // Insert the duplicate right after the existing typed field's own
        // closing `</field>` (located via its unique TypeId uuid).
        let type_id_marker = "488c0ffa-ef24-480c-a420-3bd2736317f9</TypeId>";
        let type_id_at = primary.find(type_id_marker).expect("Owner TypeId marker");
        let insertion = type_id_at
            + primary[type_id_at..]
                .find("</field>")
                .expect("Owner field closing tag")
            + "</field>".len();
        let extra_field = "\r\n\t\t\t<field xsi:type=\"DataSetFieldField\"><dataPath>Owner</dataPath><field>Owner</field><valueType><TypeId xmlns=\"http://v8.1c.ru/8.1/data/core\">488c0ffa-ef24-480c-a420-3bd2736317f9</TypeId></valueType></field>";
        let three_field_primary = format!(
            "{}{}{}",
            &primary[..insertion],
            extra_field,
            &primary[insertion..]
        );
        let terminal = std::str::from_utf8(documents[2]).unwrap();
        let settings = std::str::from_utf8(documents[1]).unwrap();
        let three_field_documents: [&[u8]; 3] = [
            three_field_primary.as_bytes(),
            settings.as_bytes(),
            terminal.as_bytes(),
        ];

        let mut type_index = BTreeMap::new();
        type_index.insert(
            "488c0ffa-ef24-480c-a420-3bd2736317f9".to_owned(),
            DcsTypeResolution::Type {
                qname: "cfg:CatalogRef.FilterProbe".to_owned(),
            },
        );
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &three_field_documents,
            &type_index,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a three-field DataSetQuery must transliterate, not be dropped");
        let actual = String::from_utf8(actual).expect("source XML is UTF-8");
        // The corpus carries the top-level `dataSet` query plus the one nested
        // in the `DataSetUnion` item, and both survive the rewrite.
        assert_eq!(actual.matches("xsi:type=\"DataSetQuery\"").count(), 2);
        assert_eq!(actual.matches("<dataPath>Owner</dataPath>").count(), 2);
        assert!(
            actual.contains("<v8:Type xmlns:d5p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">d5p1:CatalogRef.FilterProbe</v8:Type>"),
            "the resolved TypeId keeps its evidenced source spelling: {actual}"
        );
    }

    #[test]
    fn platform_link_parameter_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-parameter/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-parameter/native-template.xml.b64"
        )));
        // manifest.json: retained.packed_body.sha256 / retained.native_template.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "5211a2ac9fa02d3351686f48963445e0062e2d174327fcaf47026a6f11a6b9ae"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "381e86721884c63c9f99dcde21f1cd78cca07b4644714bf635e954b1f59fc698"
        );
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        // Proves the existing route already transparently threads the new
        // optional `dataSetLink` fields through the shared codec: no
        // per-field wiring was added here, this call is unchanged from the
        // base `dcs-query-union-link` corpus's own equivalent test.
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_link_expressions_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-expressions/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-expressions/native-template.xml.b64"
        )));
        // manifest.json: retained.packed_body.sha256 / retained.native_template.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "c78d5cbf882eec93cec27480f200f1dad7b0d98c5938e8c2d115c4c1f4b46ce3"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "e80cc9492ab93cabff9799fb14e7e4c6fafff0d96129acba19ba53d4aa4faf54"
        );
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        // Same transparency proof as the link-parameter cohort, for the
        // fuller six-plus-three-field state (including the platform's own
        // canonical reordering of the three newest fields).
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_style_free_area_template_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template/native-template.xml.b64"
        )));
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_area_appearance_web_color_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-appearance-web-color/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-appearance-web-color/native-template.xml.b64"
        )));
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_multi_cell_appearance_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/native-template.xml.b64"
        )));
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_area_style_color_reference_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-color-reference/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-color-reference/native-template.xml.b64"
        )));
        // manifest.json: retained.packed_body.sha256 / retained.native_template.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "1e6c10a050235b9ecd42b1b7bdcdb3df5b148bde0c42cb442ba7bd16722cdf9b"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "4269ac193b76bb88ecaaf65a5b4ef9ed12a31cdcf1d36d8ac429de68cf10f970"
        );
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        // The standard/built-in style-reference form needs no resolver
        // (`object_refs` empty, exactly like every other pre-existing
        // corpus): the named lexical spelling is authenticated on both
        // directions without any configuration-object lookup.
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_area_style_item_uuid_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-item-uuid/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-item-uuid/native-template.xml.b64"
        )));
        // manifest.json: retained.packed_body.sha256 / retained.native_template.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "680d04d34a12c54be75ac69a5c20ff82d2136736e11998b070c77d7abbbe3235"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "98f1857d3424198275cc35834a6635c28623568aae8d01a95cb5e220f91b818f"
        );
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        // The custom StyleItem form's raw uuid storage wire form requires a
        // resolver: `object_refs` here stands in for what a real dump
        // session's own object-reference scan would already have found
        // (see `data_composition_style_item_name`'s own established
        // "StyleItem.<Name>" convention, reused verbatim, not invented).
        let mut object_refs = BTreeMap::new();
        object_refs.insert(
            "4a9d8536-ff59-4a90-a1cf-646d241dc53c".to_string(),
            "StyleItem.CorpusAccent".to_string(),
        );
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &object_refs,
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);

        // Without the resolver entry the export must fail closed instead of
        // silently dropping the AreaTemplate appearance or emitting the raw
        // `0:<uuid>` storage spelling, which the platform never writes.
        //
        // This is where the resolver gate lives now. The envelope admits this
        // terminal document either way -- its frame is decidable without
        // naming the style item, which is what lets real configurations'
        // area templates be transliterated at all -- so the typed coordinate
        // refuses for want of the name and the fragment transliteration,
        // reached next, refuses for exactly the same reason rather than
        // guessing one. See ibcmd-compiler's
        // `compiler::bodies::dcs::tests::area_style_item_uuid_strict_decode_admits_the_frame_without_a_resolver`.
        for resolver in [
            BTreeMap::new(),
            BTreeMap::from([(
                "00000000-0000-0000-0000-000000000000".to_string(),
                "StyleItem.SomeOtherStyleItem".to_string(),
            )]),
        ] {
            let error = normalize_data_composition_schema_template_documents_with_profiles(
                &body.documents(),
                &BTreeMap::new(),
                &resolver,
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .expect_err("an unresolvable style reference has no source spelling");
            assert_eq!(
                error.class(),
                MetadataSourceFailureClass::Unsupported,
                "the refusal is `no evidence names this`, not a broken invariant"
            );
            assert!(
                format!("{error}").contains("StyleItem resolution"),
                "the refusal names the missing resolution: {error}"
            );
        }
    }

    #[test]
    fn platform_parameter_scalar_types_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-parameter-scalar-types/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-parameter-scalar-types/native-template.xml.b64"
        )));
        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_output_parameters_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-output-parameters/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-output-parameters/native-template.xml.b64"
        )));
        // manifest.json: rounds.packed_body_sha256 / rounds.native_template_sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "1bab2e8e93c491f33d094473d8456e7877c1673d45656cca9d4729ea40c82fd7"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "bc27a20de1bb75a83b3727ac457db04791cdd092c64e2e31e5b58ebecf296ddb"
        );

        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();

        // `canonicalize_data_composition_settings_document` now routes
        // outputParameters through the same shared `ibcmd-xml` codec
        // (`DcsSettingsBuilder::output_parameters`, `emit_dcs_settings_children_parts`,
        // `rewrite_dcs_settings_children`) that selection/filter/order/
        // conditionalAppearance already used, the same way the terminal
        // AreaTemplate side table achieves byte-accuracy for TextColor/
        // Details. The storage "Title" -> source "Заголовок" lexical
        // canonicalization this codec already performed is now actually
        // exercised on this path, so the export is byte-exact.
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_8_3_27_xml_2_20_root_auto_selection_exports_byte_exact() {
        let packed = include_bytes!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/dcs-selection-auto/raw/\
             f4db0f6c-34f4-4449-995d-6265516e5fa8.0.deflate"
        );
        let expected = include_bytes!(
            "../../../../tests/fixtures/native-evidence/8.3.27.2214/dcs-selection-auto/native/Reports/\
             DcsCorpus/Templates/MainSchema/Ext/Template.xml"
        );

        let body = ibcmd_compiler::compiler::bodies::dcs::decode_compatible_dcs(
            ibcmd_compiler::compiler::bodies::dcs::DcsTemplateKind::Schema,
            packed,
        )
        .expect("platform-attested root Auto selection body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("platform-attested root Auto selection must be exportable through the live codec");

        assert_eq!(actual, expected);
    }
}
