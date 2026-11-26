//! Data-driven XML projection DERIVATION (`docs/APPROACH.md` §2.2): the per-field
//! `(XmlLocus, Codec)` is COMPUTED from the canonical spec by convention instead of
//! being hand-written per vid:
//!
//! * tag = `f(name, dialect)`  — EDT: field name VERBATIM (`<synonym>`); Designer:
//!   UpperCamelCase under `[<Kind>, "Properties", <Tag>]` (`<Constant><Properties><Synonym>`);
//! * codec = `f(value_kind)` — the reused value-codec table (ось-1, `docs/APPROACH.md` §1).
//!
//! This is the runtime realization of `tools/gen_projection.py`: that script's `--gate`
//! proves (design-time) the convention reproduces the hand-written maps of done vids
//! byte-identically; this module APPLIES the same convention at run time, driven by the
//! HAND-WRITTEN canonical spec the engine actually walks (so `FieldId` ordinals/names
//! match by construction — no metamodel-superset drift). A vid's connector delegates its
//! [`crate::LocusMap::lookup`] here; canon (id/order/defaults/normalization) stays in
//! `core/spec`, codec SEMANTICS stay in the shared codecs — only HOW the projection table
//! is produced changes (§1.6).
//!
//! Value-kinds whose codec is KIND-dependent (`List`/`Ref`/`Blob`/`StyleValue` — ref-lists,
//! characteristics, choice-parameters, …) are NOT convention-derivable: [`codec_for`]
//! returns `None` for them, and vids carrying such fields stay hand-written (structural
//! residual, §6). A vid is switched to derived ONLY when the byte-exact R+X gate stays
//! 100% green for it.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use morph1c_core::ir::value::ValueKind;
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::EntitySpec;

use crate::type_codec::TypeDialect;
use crate::value_codec::ValueDialect;
use crate::{Codec, FieldProjection, XmlLocus};

/// XML dialect for the derivation (tag convention + value-codec table).
///
/// EDT uses the SAME single-segment `[name]` path for a top-level object AND for a nested
/// child node (child-relative), so one variant covers both. Designer differs: a top-level
/// object's locus is a 3-segment path from `<MetaDataObject>` (`[Kind,"Properties",Tag]`),
/// but a nested child's locus is RELATIVE to that child's own `<Properties>` region — a
/// single-segment `[Tag]`. Hence [`Designer`](Self::Designer) vs [`DesignerChild`](Self::DesignerChild).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeriveDialect {
    /// EDT `.mdo` (top-level OR child): tag = field name verbatim; `BoolPresence`/
    /// `LocalizedKeyVal`/`Type(Edt)`/…
    Edt,
    /// Designer `.xml` TOP-LEVEL object: path = `[Kind, "Properties", UpperCamel(name)]`;
    /// `BoolText`/`LocalizedV8`/…
    Designer,
    /// Designer `.xml` CHILD node: path relative to the child's `<Properties>` region —
    /// single-segment `[UpperCamel(name)]`; codecs identical to [`Designer`](Self::Designer).
    DesignerChild,
}

/// Codec of a REGULAR scalar/leaf field from its canonical [`ValueKind`] + dialect
/// (the derived value_kind→codec table, mirror of `gen_projection.py`). `None` =
/// kind-dependent codec (`List`/`Ref`/`Blob`/`StyleValue`): not derivable by value_kind
/// alone → the field (and its vid) stays hand-written.
pub fn codec_for(dialect: DeriveDialect, vk: ValueKind) -> Option<Codec> {
    // Designer child shares Designer's codec table (only the PATH differs).
    let is_edt = matches!(dialect, DeriveDialect::Edt);
    Some(match vk {
        ValueKind::Localized if is_edt => Codec::LocalizedKeyVal,
        ValueKind::Localized => Codec::LocalizedV8,
        ValueKind::Str => Codec::PlainText,
        ValueKind::Bool if is_edt => Codec::BoolPresence,
        ValueKind::Bool => Codec::BoolText,
        ValueKind::Enum => Codec::EnumText,
        ValueKind::Int => Codec::IntText,
        ValueKind::Type if is_edt => Codec::Type(TypeDialect::Edt),
        ValueKind::Type => Codec::Type(TypeDialect::Designer),
        ValueKind::Value if is_edt => Codec::Value(ValueDialect::Edt),
        ValueKind::Value => Codec::Value(ValueDialect::Designer),
        // Kind-dependent codecs — not derivable from value_kind alone (structural residual).
        ValueKind::Ref | ValueKind::StyleValue | ValueKind::List | ValueKind::Blob => return None,
    })
}

/// lowerCamelCase field name → UpperCamelCase Designer tag (capitalise the first char).
fn upper_camel(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

type Table = &'static [(FieldId, FieldProjection)];

/// Build (once) and memoize the derived projection table for `(dialect, spec)`.
///
/// The `&'static` requirement on [`XmlLocus`] paths (path/ns are `&'static`) is satisfied
/// by leaking the computed table ONCE per `(dialect, spec)` — bounded (a few hundred rows
/// across all vids), one-time, mirroring how specs already `Box::leak` their fields.
///
/// Keyed by the spec's POINTER (specs are `&'static` singletons from `OnceLock`, so the
/// same vid always yields the same address and distinct vids distinct addresses) — this is
/// collision-free even when two distinct child specs share an `entity` NAME (`Method`, …).
fn table(dialect: DeriveDialect, spec: &EntitySpec) -> Table {
    static MEMO: OnceLock<Mutex<HashMap<(DeriveDialect, usize), Table>>> = OnceLock::new();
    let memo = MEMO.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (dialect, spec as *const EntitySpec as usize);
    {
        let guard = memo.lock().expect("derive memo poisoned");
        if let Some(t) = guard.get(&key) {
            return t;
        }
    }
    let mut rows: Vec<(FieldId, FieldProjection)> = Vec::new();
    for fs in spec.fields() {
        let Some(codec) = codec_for(dialect, fs.value_kind) else {
            continue;
        };
        let path: &'static [&'static str] = match dialect {
            // EDT tag = field name verbatim (`fs.name` is already `&'static str`) — same
            // single-segment path for top-level objects AND nested children.
            DeriveDialect::Edt => Box::leak(vec![fs.name].into_boxed_slice()),
            // Designer TOP-LEVEL: 3-segment path from `<MetaDataObject>`.
            DeriveDialect::Designer => {
                let tag: &'static str = Box::leak(upper_camel(fs.name).into_boxed_str());
                Box::leak(vec![spec.entity, "Properties", tag].into_boxed_slice())
            }
            // Designer CHILD: single-segment path relative to the child's `<Properties>`.
            DeriveDialect::DesignerChild => {
                let tag: &'static str = Box::leak(upper_camel(fs.name).into_boxed_str());
                Box::leak(vec![tag].into_boxed_slice())
            }
        };
        let locus = XmlLocus::PropElement { path, ns: "" };
        rows.push((fs.id, FieldProjection::new(locus, codec)));
    }
    let leaked: Table = Box::leak(rows.into_boxed_slice());
    let mut guard = memo.lock().expect("derive memo poisoned");
    // Another thread may have inserted meanwhile; keep the first (both are equal).
    guard.entry(key).or_insert(leaked)
}

/// Derived projection of `field` in `spec` for `dialect` (data-driven, §2.2).
///
/// Returns `None` when the field is not in `spec` OR its value_kind is not
/// convention-derivable ([`codec_for`] `None`) — identical `None`-semantics to a
/// hand-written map that omits the field (engine treats it as `Absent`/skip).
pub fn projection(
    dialect: DeriveDialect,
    spec: &EntitySpec,
    field: FieldId,
) -> Option<FieldProjection> {
    table(dialect, spec)
        .iter()
        .find(|(id, _)| *id == field)
        .map(|(_, fp)| fp.clone())
}
