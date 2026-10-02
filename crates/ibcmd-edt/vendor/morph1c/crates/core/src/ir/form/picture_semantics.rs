use serde::{Deserialize, Serialize};

use crate::ir::Uuid;
use crate::ir::value::ValueScalarKind;

/// Semantic per-use picture data that the installed EDT PictureRef model lacks.
/// Unlike lexical source facets, every binding and value is serialized in the
/// canonical fingerprint. Both native and extended EDT readers create this model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormPictureSemantics {
    pub form_uuid: Uuid,
    pub records: Vec<PictureSemanticRecord>,
    /// Current nested definition bytes, bound to the declared safe native Abs path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<PictureSemanticAsset>,
    /// Names-only native explicit Point presence; CURRENT coordinates remain semantic.
    #[serde(skip)]
    pub native_point_presence: Vec<PictureSemanticBinding>,
}

impl PartialEq for FormPictureSemantics {
    fn eq(&self, other: &Self) -> bool {
        self.form_uuid == other.form_uuid
            && self.records == other.records
            && self.assets == other.assets
    }
}
impl Eq for FormPictureSemantics {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PictureSemanticRecord {
    pub binding: PictureSemanticBinding,
    pub reference: String,
    pub load_transparent: bool,
    pub pixel: Option<PictureSemanticPixel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PictureSemanticPixel {
    pub x: i64,
    pub y: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum PictureSemanticBinding {
    Chart {
        attribute: Vec<PictureChartAttributeIdentity>,
        chart_kind: String,
        path: Vec<PictureChartStep>,
        current_container_canonical: String,
    },
    Command {
        id: i64,
        name: String,
    },
    ChoiceParameter {
        path: Vec<PictureControlIdentity>,
        name: String,
        wrappers: Vec<PictureParameterIdentity>,
    },
    Control {
        path: Vec<PictureControlIdentity>,
        slot: String,
        choice: Option<PictureChoiceIdentity>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PictureControlIdentity {
    pub edge: PictureControlEdge,
    pub kind: String,
    pub id: i64,
    pub name: String,
}

/// Edges distinguish actual containment regions, including repeated synthetic
/// auto-bar ids. Array indexes never substitute for a control's typed identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PictureControlEdge {
    Root,
    Child,
    Addition,
    AutoTable,
    AutoCommandBar,
    ContextMenu,
}

/// Identity of a choice item, excluding its picture. Embedded canonical text is
/// compared to freshly serialized, fully claimed typed data; never parsed or
/// replayed as values. Equal projected choices use an explicit occurrence ordinal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PictureChoiceIdentity {
    pub presentation: Vec<(String, String)>,
    pub value_kind: ValueScalarKind,
    pub value_canonical: String,
    pub duplicate_ordinal: usize,
}

/// Current fully typed wrapper identity, excluding every picture value. Canonical
/// text is compared to regenerated values and never decoded/replayed as data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PictureParameterIdentity {
    pub presentation: Vec<(String, String)>,
    pub value_canonical: String,
    pub duplicate_ordinal: usize,
}

/// Semantic asset data; no original descriptor bytes or source-value cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PictureSemanticAsset {
    pub binding: PictureSemanticBinding,
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "edge", deny_unknown_fields)]
pub enum PictureChartAttributeIdentity {
    Attribute {
        name: String,
        id: i64,
    },
    Column {
        name: String,
        id: i64,
    },
    AdditionalColumn {
        table_path: String,
        group: usize,
        name: String,
        id: i64,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum PictureChartStep {
    Field(String),
    Item(usize),
}
