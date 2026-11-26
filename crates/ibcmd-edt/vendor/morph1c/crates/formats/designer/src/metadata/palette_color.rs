//! Local source-only native XML PaletteColor projection.
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::{
    ir::{FieldId, MetadataObject},
    spec::metadata::palette_color::*,
};
pub struct DesignerPaletteColor;
impl LocusMap for DesignerPaletteColor {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        let (path, codec): (&'static [&'static str], _) = match field {
            F_SYNONYM => (
                &["PaletteColor", "Properties", "Synonym"],
                Codec::LocalizedV8,
            ),
            F_COMMENT => (&["PaletteColor", "Properties", "Comment"], Codec::PlainText),
            F_COLOR => (
                &["PaletteColor", "Properties", "Color"],
                Codec::MetadataColor(formats_xml::metadata_color::Dialect::Designer),
            ),
            _ => return None,
        };
        Some(FieldProjection::new(
            XmlLocus::PropElement { path, ns: "" },
            codec,
        ))
    }
}
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    let doc = formats_xml::parse(bytes).map_err(|e| e.to_string())?;
    if doc
        .root
        .attr("version")
        .is_none_or(|version| version.value != "2.21")
    {
        return Err("PaletteColor requires witnessed XML 2.21 source profile".into());
    }
    crate::read_descriptor(
        "PaletteColor",
        palette_color(),
        &DesignerPaletteColor,
        bytes,
    )
    .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    if morph1c_core::version::current_roundtrip_target()
        .is_none_or(|version| version.to_string() != "2.21")
    {
        return Err("PaletteColor requires explicit XML 2.21 target profile".into());
    }
    crate::write_descriptor("PaletteColor", palette_color(), &DesignerPaletteColor, obj)
        .map_err(|e| e.to_string())
}
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "PaletteColor",
    read,
    write,
    corpus_subpath: "local/designer/PaletteColors",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
