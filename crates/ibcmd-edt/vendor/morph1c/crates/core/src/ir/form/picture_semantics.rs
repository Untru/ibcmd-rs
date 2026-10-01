use serde::{Deserialize, Serialize};

use crate::ir::Uuid;
use crate::ir::value::ValueScalarKind;

/// Semantic per-use picture data that the installed EDT PictureRef model lacks.
/// Unlike lexical source facets, every binding and value is serialized in the
/// canonical fingerprint. Both native and extended EDT readers create this model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormPictureSemantics {
    pub form_uuid: Uuid,
    pub records: Vec<PictureSemanticRecord>,
}

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
    Command {
        id: i64,
        name: String,
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
