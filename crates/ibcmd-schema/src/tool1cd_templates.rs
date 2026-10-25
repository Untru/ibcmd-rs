//! Template enum values independently described by tool1cd and checked against Coverage.
//!
//! Source: jjzhang166/tool1cd@7c4b91cb7a7bb48a32be00b9f32afe740dada6e1,
//! MetaTree.txt: web colours 1122/1142, Windows font 1780, spreadsheet drawing
//! lines 45777/45778, graphical-schema shape 47746, HTML grammar 53128–53147.
//! These enums do not establish the MOXCEL page-break grammar: that rule is
//! separately measured from native Coverage and the existing reader.

pub struct TemplateEnumCodes;

impl TemplateEnumCodes {
    pub const OEM_FIXED_FONT: &'static str = "1";
    pub const SANDY_BROWN: u32 = 124;
    pub const SKY_BLUE: u32 = 129;
    pub const DRAWING_DASHED: usize = 2;
    pub const DRAWING_DASH_DOTTED: usize = 4;
    pub const VERTICAL_BRACKETS: i64 = 9;
    pub const VERTICAL_BRACKETS_TEXT: &'static str = "9";
    pub const SHAPE_INDEX: usize = 9;
    pub fn graphical_shape_name(code: &str) -> Option<&'static str> {
        Some(match code {
            "0" => "None",
            "9" => "VerticalBrackets",
            "10" => "Document",
            "11" => "Block",
            "12" => "HorizontalBrackets",
            _ => return None,
        })
    }
}

pub struct EmptyHtmlLayout;

impl EmptyHtmlLayout {
    pub const ARITY: usize = 3;
    pub const ATTACHMENT_COUNT: usize = 2;
    pub fn is_empty_tail(fields: &[&str]) -> bool {
        fields.len() == Self::ARITY
            && fields.get(Self::ATTACHMENT_COUNT).map(|value| value.trim()) == Some("0")
    }
}

/// Sparse AutoFont with one explicitly stored height, measured on Coverage.
pub struct AutoFontHeightLayout;
impl AutoFontHeightLayout {
    pub const ARITY: usize = 6;
    pub const MASK: usize = 2;
    pub const HEIGHT: usize = 3;
    pub const MARKER: usize = 4;
    pub const SCALE: usize = 5;
}

/// Last member of a native MOXCEL line descriptor; distinct from member 5.
pub struct MoxelLineLayout;
impl MoxelLineLayout {
    pub const TABLE_FIELD: usize = 5;
    pub const TABLE_HEADER: usize = 1;
    pub const ENTRY_WIDTH: usize = 3;
    pub const ENTRY_DESCRIPTOR: usize = 1;
    pub const GAP: usize = 7;
    pub fn matches_descriptor_profile(fields: &[&str]) -> bool {
        fields.first().map(|value| value.trim()) == Some("4")
            && fields.get(1).map(|value| value.trim()) == Some("0")
            && fields.get(2).map(|value| value.trim()) == Some("{0}")
    }
}

/// Measured optional keyed properties of graphical-schema print parameters.
pub struct FlowchartPrintKeys;
impl FlowchartPrintKeys {
    pub const PAPER: u32 = 0;
    pub const ORIENTATION: u32 = 1;
    pub const COLLATE: u32 = 3;
    pub const COPIES: u32 = 4;
    pub const PAPER_SOURCE: u32 = 15;
    pub fn admits(key: u32) -> bool {
        matches!(key, 0 | 1 | 3 | 4 | 6 | 7 | 8 | 9 | 13 | 15 | 16)
    }
    pub fn orientation_name(code: &str) -> Option<&'static str> {
        match code {
            "1" => Some("Portrait"),
            "2" => Some("Landscape"),
            _ => None,
        }
    }
    pub fn orientation_code(name: &str) -> Option<i64> {
        match name {
            "Portrait" => Some(1),
            "Landscape" => Some(2),
            _ => None,
        }
    }
}

/// Picture format admission used only to choose an inline scheme asset suffix.
pub struct TemplatePictureLayout;
impl TemplatePictureLayout {
    pub fn extension(bytes: &[u8]) -> Option<&'static str> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some("png")
        } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            Some("jpg")
        } else if bytes.starts_with(b"BM") {
            Some("bmp")
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            Some("gif")
        } else {
            None
        }
    }
}

/// Counted prefix of the native MOXCEL 8/1/12 group/page-break profile.
/// Coordinates are independently measured on public Coverage body records.
pub struct MoxelGroupBreakLayout;
impl MoxelGroupBreakLayout {
    pub const ROW_COUNT: usize = 15;
    pub const ROWS: usize = 16;
    pub const ROW_CELL_COUNT: usize = 2;
    pub const ROW_HEADER: usize = 3;
    pub const PAIR_WIDTH: usize = 2;
    pub const COLUMN_EXTRA_COUNT: usize = 2;
    pub const COLUMN_HEADER: usize = 3;
    pub const DRAWING_COUNT: usize = 1;
    pub const DRAWING_HEADER: usize = 2;
    pub const COUNT_HEADER: usize = 1;
    pub const HORIZONTAL_SENTINEL: usize = 1;
    pub const HORIZONTAL_SENTINEL_VALUE: &'static str = "-1";
    pub const MERGE_LISTS: usize = 3;
    pub fn matches(fields: &[&str]) -> bool {
        fields.first().map(|v| v.trim()) == Some("8")
            && fields.get(1).map(|v| v.trim()) == Some("1")
            && fields.get(2).map(|v| v.trim()) == Some("12")
    }
}
