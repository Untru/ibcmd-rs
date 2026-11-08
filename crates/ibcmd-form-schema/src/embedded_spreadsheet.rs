//! The embedded MOXCEL text-cell cohort, also written by the template writer.
//! Public native 8.3.27 and 8.5 records use the same row, text and format rules.

pub const ATTRIBUTES: &str = "Attributes";
pub const ATTRIBUTE: &str = "Attribute";
pub const SETTINGS: &str = "Settings";
pub const CELL: &str = "c";
pub const TYPE: &str = "type";
pub const DOCUMENT_TYPE: &str = "SpreadsheetDocument";
pub const ROW_ITEM: &str = "rowsItem";
pub const ROW: &str = "row";
pub const LANGUAGE_INFO: &str = "languageInfo";
pub const FONT: &str = "font";
pub const FORMAT: &str = "format";
pub const MERGE: &str = "merge";
pub const TEXT_FILL: &str = "Text";
pub const ABSOLUTE_FONT: &str = "Absolute";

pub fn text_cell(format: usize, localized: &str) -> String {
    format!("{{16,{format},{localized},0}}")
}

pub fn text_format(font: Option<usize>, width: Option<u32>, fill: bool) -> String {
    let mut mask = 0;
    let mut values = Vec::new();
    if let Some(font) = font {
        mask |= 1;
        values.push(font.to_string());
    }
    if let Some(width) = width {
        mask |= 128;
        values.push(width.to_string());
    }
    if fill {
        mask |= 32768;
        values.push("0".to_string());
    }
    if values.is_empty() {
        "{0}".to_string()
    } else {
        format!("{{{mask},{}}}", values.join(","))
    }
}

pub fn absolute_font(
    face: &str,
    height_tenths: i32,
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
    scale: u32,
) -> String {
    let weight = if bold { 700 } else { 400 };
    format!(
        "{{7,0,575,{height_tenths},0,0,0,{weight},{},{},{},0,0,0,0,0,{face},1,{scale}}}",
        u8::from(italic),
        u8::from(underline),
        u8::from(strikeout)
    )
}

/// Complete stored row, before the document's column table.
pub fn row(index: u32, format: usize, cells: &[(u32, String)]) -> String {
    let count = cells.len();
    let cells = cells
        .iter()
        .map(|(column, cell)| format!(",{column},{cell}"))
        .collect::<String>();
    format!(",{index},{format},{count}{cells}")
}

pub struct Document<'a> {
    pub language: &'a str,
    pub default_format: &'a str,
    pub template: bool,
    pub rows: &'a [String],
    pub columns: u32,
    pub height: u32,
    pub merges: &'a str,
    pub formats: &'a [String],
    pub fonts: &'a [String],
}

impl Document<'_> {
    pub fn encode(&self) -> String {
        let formats = self
            .formats
            .iter()
            .map(|value| format!(",{value}"))
            .collect::<String>();
        let fonts = self
            .fonts
            .iter()
            .map(|value| format!(",{value}"))
            .collect::<String>();
        format!(
            "{{0,1,\"Moxel\",{{\"#\",e603103e-a318-4edc-a014-b1c6cf94d49f,{{8,1,12,{},{},{{0}},0,\
             {{0,0}},{{0,0}},{{0,0}},{{0,0}},{{0,0}},{{0,0}},{},2,{}{},\
             {{{},0,00000000-0000-0000-0000-000000000000,0}},{},0,0,0,0,0,0,0,0,{},{{0}},{{0}},{{0}},\"\",\
             {{{{0,6,6,{{\"N\",1000}},7,{{\"N\",1000}},8,{{\"N\",1000}},9,{{\"N\",1000}},10,{{\"N\",1000}},\
             11,{{\"N\",1000}}}}}},{{0,-1,-1,-1,-1,00000000-0000-0000-0000-000000000000}},0,0,0,0,1,00000000-0000-0000-0000-000000000000,0,0,1,0,\
             {}{formats},{}{fonts},0,0,0,2,{{3,3,{{-1}}}},{{3,3,{{-3}}}},0,0,0,\"\",0,\
             {{3,0,0,100,1,1,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,\"\",0,0,0,0,0,0,0}},{{0}},0,0,0,1,0,0,0}}}}}}",
            self.language,
            self.default_format,
            u8::from(self.template),
            self.rows.len(),
            self.rows.join(""),
            self.columns,
            self.height,
            self.merges,
            self.formats.len(),
            self.fonts.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_text_row_font_and_format_records_are_complete() {
        let cell = text_cell(1, "{1,1,{\"ru\",\"Coverage\"}}");
        assert_eq!(cell, "{16,1,{1,1,{\"ru\",\"Coverage\"}},0}");
        assert_eq!(
            row(0, 0, &[(0, cell)]),
            ",0,0,1,0,{16,1,{1,1,{\"ru\",\"Coverage\"}},0}"
        );
        assert_eq!(text_format(Some(0), None, true), "{32769,0,0}");
        assert_eq!(text_format(None, Some(20), false), "{128,20}");
        assert_eq!(
            absolute_font("\"Arial\"", 100, false, false, false, false, 100),
            "{7,0,575,100,0,0,0,400,0,0,0,0,0,0,0,0,\"Arial\",1,100}"
        );
    }
}
