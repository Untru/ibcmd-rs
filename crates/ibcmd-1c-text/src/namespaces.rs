//! The namespace declarations of the XML dialect 2.21 writes: the palette
//! colour namespace beside the style one. It moved unchanged from the root's
//! `mssql_dump/source_assets.rs`, which keeps its path by re-export; the 2.21
//! form loader reads it too.

const STYLE_NAMESPACE_DECLARATION: &str = " xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\"";
const PALETTE_NAMESPACE_DECLARATION: &str =
    " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"";

/// Dialect 2.21 declares the palette colour namespace on exactly the elements
/// that declare the style namespace, and nowhere else.
///
/// 8.5.1.1150 BSP native tree: of 6 299 start tags carrying namespace
/// declarations, every one of the 6 298 that declares `xmlns:style` also
/// declares `xmlns:pal` (`MetaDataObject`, `Form`, `document`,
/// `dcsset:settings`, `settings`, `GraphicalSchema`, `AppearanceTemplate`) and
/// none of the others does. The platform lists declarations alphabetically by
/// prefix, so `pal` goes before the first prefixed declaration that sorts after
/// it -- `style` on most roots, `sch` on a graphical schema. A tag that already
/// declares the prefix is left as it is.
pub fn declare_palette_namespace_beside_style(bytes: Vec<u8>) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return bytes;
    };
    if !text.contains(STYLE_NAMESPACE_DECLARATION) {
        return bytes;
    }
    let mut out = String::with_capacity(text.len() + 256);
    let mut cursor = 0;
    let mut search_from = 0;
    while let Some(relative) = text[search_from..].find(STYLE_NAMESPACE_DECLARATION) {
        let style_at = search_from + relative;
        search_from = style_at + STYLE_NAMESPACE_DECLARATION.len();
        let Some(tag_start) = text[..style_at].rfind('<') else {
            continue;
        };
        if tag_start < cursor || text[tag_start..style_at].contains('>') {
            continue;
        }
        let tag_end = text[style_at..]
            .find('>')
            .map_or(text.len(), |offset| style_at + offset);
        let tag = &text[tag_start..tag_end];
        if tag.contains(" xmlns:pal=") {
            continue;
        }
        // A value that declares the style namespace alone for its own `ref`
        // (a DCS font `dcscor:value`) gets no palette beside it: 23 of 23 in
        // the 8.5.1.1150 ERP УХ export, against every tag that declares more.
        if tag.matches(" xmlns:").count() == 1 {
            continue;
        }
        let mut insert_at = style_at;
        let mut scan = tag_start;
        while let Some(offset) = text[scan..style_at].find(" xmlns:") {
            let declaration = scan + offset;
            let prefix_start = declaration + " xmlns:".len();
            let prefix_end = text[prefix_start..style_at]
                .find('=')
                .map_or(style_at, |end| prefix_start + end);
            if &text[prefix_start..prefix_end] > "pal" {
                insert_at = declaration;
                break;
            }
            scan = prefix_end;
        }
        out.push_str(&text[cursor..insert_at]);
        out.push_str(PALETTE_NAMESPACE_DECLARATION);
        cursor = insert_at;
    }
    out.push_str(&text[cursor..]);
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLE: &str = " xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\"";
    const PALETTE: &str = " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"";

    fn declared(xml: &str) -> String {
        String::from_utf8(declare_palette_namespace_beside_style(
            xml.as_bytes().to_vec(),
        ))
        .unwrap()
    }

    #[test]
    fn the_palette_goes_beside_the_style_namespace_in_prefix_order() {
        // Before `style` on most roots, before `sch` on a graphical schema.
        assert_eq!(
            declared(&format!("<Form xmlns:app=\"a\"{STYLE}><a/></Form>")),
            format!("<Form xmlns:app=\"a\"{PALETTE}{STYLE}><a/></Form>")
        );
        assert_eq!(
            declared(&format!("<GraphicalSchema xmlns:sch=\"s\"{STYLE}/>")),
            format!("<GraphicalSchema{PALETTE} xmlns:sch=\"s\"{STYLE}/>")
        );
        // A tag that declares the style namespace alone, one that already
        // declares the palette, and a document without it stay as they are.
        for xml in [
            format!("<dcscor:value{STYLE}>style:X</dcscor:value>"),
            format!("<Form xmlns:app=\"a\"{PALETTE}{STYLE}/>"),
            "<Form xmlns:app=\"a\"/>".to_owned(),
        ] {
            assert_eq!(declared(&xml), xml);
        }
        // Bytes that are not UTF-8 are returned untouched.
        assert_eq!(
            declare_palette_namespace_beside_style(vec![0xff, 0xfe]),
            [0xff, 0xfe]
        );
    }
}
