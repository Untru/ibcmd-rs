//! Ordered, lossless XML syntax tree support.

#![forbid(unsafe_code)]

mod dialect;
pub mod metadata;
mod node;
mod reader;
pub mod source_tree;
mod writer;

pub use dialect::{
    BomRule, DetectionCandidate, DialectDescriptor, DialectDetection, DialectError,
    DialectEvidence, DialectFeature, DialectLexicalPolicy, DialectRegistry, DialectRule,
    ElementMatcher, FeatureAvailability, LexicalRules, LineEndingRule, NamespaceEvidence,
    NamespaceMatcher, ParseDialectIdError, PropertyOrderRule, RootSignature, RuleProvenance,
    XmlEncoding, bundled_dialect_registry,
};
pub use metadata::{
    MetadataDecodeError, MetadataEncodeError, MetadataEnvelope, MetadataFamilyCodec,
    MetadataRegistry, MetadataRegistryError, bundled_metadata_registry, decode_metadata_envelope,
    decode_metadata_envelope_with_dialect, register_constant_codec,
};
pub use node::{
    Attribute, AttributeKind, QName, XmlCData, XmlComment, XmlDocument, XmlElement, XmlNode,
    XmlRawNode, XmlText,
};
pub use reader::{XmlError, XmlErrorCause, XmlReader};
pub use writer::{LexicalPolicy, WriteError, XmlWriter};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ibcmd-xml");
    }

    #[test]
    fn preserve_round_trips_mixed_content_and_lexemes() {
        let input = br#"<p:r xmlns:p='urn:p' a='&quot;' xmlns:q="urn:q">x&amp;<![CDATA[y]]><!-- z --><q:e></q:e><p:x/></p:r>"#;
        let first = XmlReader::from_slice(input).unwrap();
        assert_eq!(
            XmlWriter::to_vec(&first, LexicalPolicy::Preserve).unwrap(),
            input
        );
        let second =
            XmlReader::from_slice(&XmlWriter::to_vec(&first, LexicalPolicy::Preserve).unwrap())
                .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.root().name().prefix(), Some("p"));
        assert_eq!(first.root().name().local(), "r");
        assert!(
            matches!(first.root().attributes()[0].kind(), AttributeKind::Namespace(Some(p)) if p == "p")
        );
    }

    #[test]
    fn utf8_bom_round_trips_in_preserve_and_with_root_keeps_document_shell() {
        let input = b"\xef\xbb\xbf<?xml version=\"1.0\" encoding=\"UTF-8\"?><!--before--><r a='1'>x</r><?after ok?>";
        let document = XmlReader::from_slice(input).unwrap();
        assert!(document.has_utf8_bom());
        assert_eq!(
            XmlWriter::to_vec(&document, LexicalPolicy::Preserve).unwrap(),
            input
        );
        assert_eq!(
            XmlWriter::to_vec(&document, LexicalPolicy::Normalized).unwrap(),
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><!--before--><r a=\"1\">x</r><?after ok?>"
        );

        let replacement = XmlElement::with_parts(
            QName::new("next").unwrap(),
            Vec::new(),
            vec![XmlNode::text("value")],
        );
        let replaced = document.with_root(replacement);
        assert!(replaced.has_utf8_bom());
        assert_eq!(replaced.declaration(), document.declaration());
        assert_eq!(replaced.before_root(), document.before_root());
        assert_eq!(replaced.after_root(), document.after_root());
        assert_eq!(
            XmlWriter::to_vec(&replaced, LexicalPolicy::Preserve).unwrap(),
            b"\xef\xbb\xbf<?xml version=\"1.0\" encoding=\"UTF-8\"?><!--before--><next>value</next><?after ok?>"
        );

        assert!(!XmlDocument::new(XmlElement::new(QName::new("r").unwrap())).has_utf8_bom());
    }

    #[test]
    fn reader_rejects_duplicate_and_misplaced_utf8_bom() {
        for input in [
            b"\xef\xbb\xbf\xef\xbb\xbf<r/>".as_slice(),
            b" <r>\xef\xbb\xbf</r>".as_slice(),
        ] {
            assert!(matches!(
                XmlReader::from_slice(input).unwrap_err().cause(),
                XmlErrorCause::Parser(message)
                    if message == "UTF-8 BOM is only allowed once at the beginning"
            ));
        }
    }

    #[test]
    fn normalized_escapes_and_is_reparseable() {
        let root = XmlElement::with_parts(
            QName::new("root").unwrap(),
            vec![Attribute::ordinary(QName::new("a").unwrap(), "\"'&<>")],
            vec![XmlNode::text("&<>")],
        );
        let output = XmlWriter::to_vec(&XmlDocument::new(root), LexicalPolicy::Normalized).unwrap();
        assert_eq!(
            std::str::from_utf8(&output).unwrap(),
            "<root a=\"&quot;&apos;&amp;&lt;&gt;\">&amp;&lt;&gt;</root>"
        );
        XmlReader::from_slice(&output).unwrap();
    }

    #[test]
    fn entity_spellings_are_semantic_when_normalized() {
        let input = b"<r>&amp;&#38;&#x26;</r>";
        let document = XmlReader::from_slice(input).unwrap();
        assert_eq!(
            XmlWriter::to_vec(&document, LexicalPolicy::Preserve).unwrap(),
            input
        );
        assert_eq!(
            XmlWriter::to_vec(&document, LexicalPolicy::Normalized).unwrap(),
            b"<r>&amp;&amp;&amp;</r>"
        );
    }

    #[test]
    fn preserve_is_composable_for_replaced_children() {
        let input = b"<r  a='&quot;' ><a>one&amp;</a><!-- odd --><b /></r>";
        let document = XmlReader::from_slice(input).unwrap();
        let children = vec![
            document.root().children()[0].clone(),
            document.root().children()[1].clone(),
            XmlNode::Element(XmlElement::with_parts(
                QName::new("x").unwrap(),
                vec![],
                vec![XmlNode::text("new")],
            )),
        ];
        let edited = XmlDocument::new(document.root().with_children(children));
        assert_eq!(
            XmlWriter::to_vec(&edited, LexicalPolicy::Preserve).unwrap(),
            b"<r  a='&quot;' ><a>one&amp;</a><!-- odd --><x>new</x></r>"
        );
    }

    #[test]
    fn structural_errors_have_locations() {
        for input in [
            b"<a></b>".as_slice(),
            b"<a>".as_slice(),
            b"<a x='1' x='2'/>".as_slice(),
            b"<a/><b/>".as_slice(),
            b"text<a/>".as_slice(),
        ] {
            let error = XmlReader::from_slice(input).unwrap_err();
            assert!(error.line() >= 1 && error.column() >= 1);
        }
    }

    #[test]
    fn reference_and_prolog_errors_are_positioned() {
        let error = XmlReader::from_slice(b"<r>\n&unknown;</r>").unwrap_err();
        assert_eq!(error.line(), 2);
        assert!(error.offset() >= 4);
        for input in [
            b"<!--c--><?xml version='1.0'?><r/>".as_slice(),
            b"<r><!DOCTYPE r></r>".as_slice(),
            b"<!DOCTYPE r><!DOCTYPE r><r/>".as_slice(),
            b"<r/><!DOCTYPE r>".as_slice(),
        ] {
            let error = XmlReader::from_slice(input).unwrap_err();
            assert!(error.line() >= 1 && error.column() >= 1);
        }
    }

    #[test]
    fn reader_validates_qnames_and_accepts_unicode_names() {
        for input in [
            b"<1a/>".as_slice(),
            b"<a:b:c/>".as_slice(),
            b"<a bad:name:extra='x'/>".as_slice(),
        ] {
            let error = XmlReader::from_slice(input).unwrap_err();
            assert!(matches!(error.cause(), XmlErrorCause::InvalidName(_)));
            assert_eq!(error.offset(), 0);
        }
        let nested = XmlReader::from_slice(b"<r>\n<1a/></r>").unwrap_err();
        assert!(matches!(nested.cause(), XmlErrorCause::InvalidName(_)));
        assert_eq!((nested.offset(), nested.line(), nested.column()), (4, 2, 1));

        let input = "<имя атрибут='да'/>".as_bytes();
        let document = XmlReader::from_slice(input).unwrap();
        assert_eq!(
            XmlWriter::to_vec(&document, LexicalPolicy::Preserve).unwrap(),
            input
        );
        let normalized = XmlWriter::to_vec(&document, LexicalPolicy::Normalized).unwrap();
        assert_eq!(normalized, "<имя атрибут=\"да\"/>".as_bytes());
        XmlReader::from_slice(&normalized).unwrap();

        let continuation_input = "<a\u{301} b\u{b7}c='да'/>".as_bytes();
        let continuation = XmlReader::from_slice(continuation_input).unwrap();
        assert_eq!(
            XmlWriter::to_vec(&continuation, LexicalPolicy::Preserve).unwrap(),
            continuation_input
        );
        let normalized = XmlWriter::to_vec(&continuation, LexicalPolicy::Normalized).unwrap();
        assert_eq!(normalized, "<a\u{301} b\u{b7}c=\"да\"/>".as_bytes());
        XmlReader::from_slice(&normalized).unwrap();
    }

    #[test]
    fn writer_rejects_xml_1_0_forbidden_characters() {
        let cases = [
            XmlElement::with_parts(
                QName::new("r").unwrap(),
                vec![],
                vec![XmlNode::text("bad\u{1}")],
            ),
            XmlElement::with_parts(
                QName::new("r").unwrap(),
                vec![Attribute::ordinary(QName::new("a").unwrap(), "bad\u{1}")],
                vec![],
            ),
            XmlElement::with_parts(
                QName::new("r").unwrap(),
                vec![],
                vec![XmlNode::cdata("bad\u{1}")],
            ),
            XmlElement::with_parts(
                QName::new("r").unwrap(),
                vec![],
                vec![XmlNode::comment("bad\u{1}")],
            ),
            XmlElement::with_parts(
                QName::new("r").unwrap(),
                vec![],
                vec![XmlNode::ProcessingInstruction(XmlRawNode::generated(
                    "p bad\u{1}",
                ))],
            ),
        ];
        for root in cases {
            let document = XmlDocument::new(root);
            assert!(XmlWriter::to_vec(&document, LexicalPolicy::Preserve).is_err());
            assert!(XmlWriter::to_vec(&document, LexicalPolicy::Normalized).is_err());
        }

        let valid = XmlDocument::new(XmlElement::with_parts(
            QName::new("имя").unwrap(),
            vec![Attribute::ordinary(QName::new("a").unwrap(), "да")],
            vec![XmlNode::text("✓")],
        ));
        let bytes = XmlWriter::to_vec(&valid, LexicalPolicy::Normalized).unwrap();
        XmlReader::from_slice(&bytes).unwrap();
    }

    #[test]
    fn reader_rejects_literal_and_numeric_forbidden_characters() {
        let literal = XmlReader::from_slice(b"<r>\n\x01</r>").unwrap_err();
        assert!(matches!(literal.cause(), XmlErrorCause::InvalidCharacter));
        assert_eq!(
            (literal.offset(), literal.line(), literal.column()),
            (4, 2, 1)
        );

        let numeric = XmlReader::from_slice(b"<r>\n&#1;</r>").unwrap_err();
        assert!(matches!(numeric.cause(), XmlErrorCause::InvalidCharacter));
        assert_eq!(
            (numeric.offset(), numeric.line(), numeric.column()),
            (4, 2, 1)
        );

        let attribute = XmlReader::from_slice(b"\n<r a='&#1;'/>").unwrap_err();
        assert!(matches!(attribute.cause(), XmlErrorCause::InvalidCharacter));
        assert_eq!(
            (attribute.offset(), attribute.line(), attribute.column()),
            (1, 2, 1)
        );
    }

    #[test]
    fn writer_rejects_doctype_moved_inside_an_element() {
        let parsed = XmlReader::from_slice(b"<!DOCTYPE r><r/>").unwrap();
        let moved = parsed.before_root()[0].clone();
        let document = XmlDocument::new(parsed.root().with_children(vec![moved]));
        assert!(XmlWriter::to_vec(&document, LexicalPolicy::Preserve).is_err());
        assert!(XmlWriter::to_vec(&document, LexicalPolicy::Normalized).is_err());
    }

    #[test]
    fn writer_rejects_duplicate_serialized_attributes_and_bad_prefixes() {
        let cases = [
            vec![Attribute::ordinary(
                QName::new("xmlns").unwrap(),
                "urn:evil",
            )],
            vec![Attribute::ordinary(
                QName::new("xmlns:p").unwrap(),
                "urn:evil",
            )],
            vec![
                Attribute::ordinary(QName::new("a").unwrap(), "1"),
                Attribute::ordinary(QName::new("a").unwrap(), "2"),
            ],
            vec![
                Attribute::ordinary(QName::new("xmlns").unwrap(), "one"),
                Attribute::namespace(None, "two"),
            ],
            vec![
                Attribute::ordinary(QName::new("xmlns:p").unwrap(), "one"),
                Attribute::namespace(Some("p".into()), "two"),
            ],
            vec![Attribute::namespace(Some("a:b".into()), "urn:bad")],
        ];
        for attributes in cases {
            let document = XmlDocument::new(XmlElement::with_parts(
                QName::new("r").unwrap(),
                attributes,
                vec![],
            ));
            assert!(XmlWriter::to_vec(&document, LexicalPolicy::Preserve).is_err());
            assert!(XmlWriter::to_vec(&document, LexicalPolicy::Normalized).is_err());
        }
    }

    #[test]
    fn writer_checks_thousands_of_attribute_names_with_one_set_pass() {
        const ATTRIBUTE_COUNT: usize = 10_000;
        let attributes: Vec<_> = (0..ATTRIBUTE_COUNT)
            .map(|index| {
                Attribute::ordinary(QName::new(format!("a{index}")).unwrap(), index.to_string())
            })
            .collect();
        let unique = XmlDocument::new(XmlElement::with_parts(
            QName::new("r").unwrap(),
            attributes.clone(),
            Vec::new(),
        ));
        assert!(crate::writer::validate_document(&unique).is_ok());
        assert!(XmlWriter::to_vec(&unique, LexicalPolicy::Normalized).is_ok());

        let mut duplicate_attributes = attributes;
        duplicate_attributes.push(Attribute::ordinary(
            QName::new("a5000").unwrap(),
            "duplicate",
        ));
        let duplicate = XmlDocument::new(XmlElement::with_parts(
            QName::new("r").unwrap(),
            duplicate_attributes,
            Vec::new(),
        ));
        assert_eq!(
            crate::writer::validate_document(&duplicate)
                .unwrap_err()
                .to_string(),
            "duplicate serialized attribute"
        );
    }

    #[test]
    fn reader_validates_xml_declaration_grammar() {
        let valid = b"<?xml version='1.0' encoding='UTF-8' standalone='no'?><r/>";
        let document = XmlReader::from_slice(valid).unwrap();
        assert_eq!(
            XmlWriter::to_vec(&document, LexicalPolicy::Preserve).unwrap(),
            valid
        );
        let normalized = XmlWriter::to_vec(&document, LexicalPolicy::Normalized).unwrap();
        XmlReader::from_slice(&normalized).unwrap();

        for input in [
            b"<?xml ?><r/>".as_slice(),
            b"<?xml encoding='UTF-8'?><r/>".as_slice(),
            b"<?xml standalone='yes' version='1.0'?><r/>".as_slice(),
            b"<?xml version='1.0' unknown='x'?><r/>".as_slice(),
            b"<?xml version='1.0' version='1.0'?><r/>".as_slice(),
            b"<?xml version='1.1'?><r/>".as_slice(),
            b"<?xml version='1.0' standalone='maybe'?><r/>".as_slice(),
            b"<?xml version='1.0' encoding='1UTF-8'?><r/>".as_slice(),
            b"<?xml version='1.0' encoding='ISO-8859-1'?><r/>".as_slice(),
            b"<?xml version='1.0' standalone='yes' encoding='UTF-8'?><r/>".as_slice(),
        ] {
            let error = XmlReader::from_slice(input).unwrap_err();
            assert!(matches!(
                error.cause(),
                XmlErrorCause::InvalidDeclaration(_)
            ));
            assert_eq!((error.offset(), error.line(), error.column()), (0, 1, 1));
        }
    }
}
