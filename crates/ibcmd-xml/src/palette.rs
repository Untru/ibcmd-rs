//! Palette namespace lexical policy for XML exported to older source dialects.

/// Drops only the exact palette declaration emitted by newer native writers,
/// and only when no remaining lexical content references its `pal:` prefix.
/// QName-valued attributes, element names and text all retain the declaration.
/// Content outside UTF-8 and documents with any use retain their exact bytes.
pub fn strip_unused_palette_namespace(content: Vec<u8>) -> Vec<u8> {
    const DECLARATION: &str = " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"";
    let Ok(text) = String::from_utf8(content.clone()) else {
        return content;
    };
    if !text.contains(DECLARATION) {
        return content;
    }
    let stripped = text.replace(DECLARATION, "");
    if stripped.contains("pal:") {
        return content;
    }
    stripped.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::strip_unused_palette_namespace;

    #[test]
    fn unused_palette_declaration_is_the_only_removed_lexeme() {
        let xml = b"\xef\xbb\xbf<Settings xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" keep='exact'/>\r\n";
        assert_eq!(
            strip_unused_palette_namespace(xml.to_vec()),
            b"\xef\xbb\xbf<Settings keep='exact'/>\r\n"
        );
    }

    #[test]
    fn used_palette_prefix_in_names_attribute_values_or_text_keeps_exact_bytes() {
        for usage in [
            "<pal:Color/>",
            "<Color xsi:type='pal:Color'/>",
            "<Value>pal:Color</Value>",
            "<!-- pal:Color -->",
        ] {
            let xml = format!("<Settings xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\">{usage}</Settings>").into_bytes();
            assert_eq!(strip_unused_palette_namespace(xml.clone()), xml);
        }
        let invalid_utf8 = vec![0xff, 0xfe];
        assert_eq!(
            strip_unused_palette_namespace(invalid_utf8.clone()),
            invalid_utf8
        );
    }
}
