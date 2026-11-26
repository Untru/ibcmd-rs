//! Local source-only EDT PaletteColor projection over existing RGB/ref codec.
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::{
    ir::{FieldId, MetadataObject},
    spec::metadata::palette_color::*,
};
pub struct EdtPaletteColor;
impl LocusMap for EdtPaletteColor {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        let (path, codec): (&'static [&'static str], _) = match field {
            F_SYNONYM => (&["synonym"], Codec::LocalizedKeyVal),
            F_COMMENT => (&["comment"], Codec::PlainText),
            F_COLOR => (
                &["color"],
                Codec::MetadataColor(formats_xml::metadata_color::Dialect::Edt),
            ),
            _ => return None,
        };
        Some(FieldProjection::new(
            XmlLocus::PropElement { path, ns: "" },
            codec,
        ))
    }
    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
        ]
    }
}
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("PaletteColor", palette_color(), &EdtPaletteColor, bytes)
        .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("PaletteColor", palette_color(), &EdtPaletteColor, obj)
        .map_err(|e| e.to_string())
}
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "PaletteColor",
    read,
    write,
    corpus_subpath: "local/edt/src/PaletteColors",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
