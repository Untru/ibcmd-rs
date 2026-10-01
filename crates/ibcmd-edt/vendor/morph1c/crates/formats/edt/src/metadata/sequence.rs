//! EDT-проекция вида `Sequence` + его дочернего вида `Sequence.Dimension` (зеркало
//! `core/spec/metadata/sequence*.rs`, ARCHITECTURE.md §5; child-objects substrate).
//! ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec` (§1.6). Платформенный
//! `<producedTypes>` (recordType+managerType+recordSetType) фреймится каркасом коннектора
//! (`PRODUCED_CATEGORIES`), не тут.
//!
//! РОДИТЕЛЬ — плоские дети корня `.mdo`. EDT эмитит РАЗРЕЖЁННО; физический порядок
//! present-свойств совпадает со спек-порядком (сверено coverage/s7_pending 6/6), поэтому
//! `field_emit_order` не нужен. `documents`/`registerRecords` — RefList-сиблинги
//! (`<documents>Document.X</documents>`; registerRecords витнессится ТОЛЬКО пустым →
//! омиссия). Dimension — inline `<dimensions uuid>` (props_wrapped=false); его
//! `documentMap`/`registerRecordsMap` витнессятся ТОЛЬКО пустыми (омиссия).

use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;

use morph1c_core::spec::metadata::sequence as sq;
use morph1c_core::spec::metadata::sequence::sequence;
use morph1c_core::spec::metadata::sequence_dimension as dim;
use morph1c_core::spec::metadata::sequence_dimension::sequence_dimension;

/// Плоский локус `PropElement{path:[tag], ns:""}`.
const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}
const fn fp(tag: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: flat(tag), codec }
}

// ===== РОДИТЕЛЬ Sequence =====
pub struct EdtSequence;

impl LocusMap for EdtSequence {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            sq::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            sq::F_COMMENT => fp(&["comment"], Codec::PlainText),
            sq::F_MOVE_BOUNDARY_ON_POSTING => fp(&["moveBoundaryOnPosting"], Codec::EnumText),
            sq::F_DOCUMENTS => fp(&["documents"], Codec::RefList(RefListDialect::Edt)),
            sq::F_REGISTER_RECORDS => fp(&["registerRecords"], Codec::RefList(RefListDialect::Edt)),
            sq::F_DATA_LOCK_CONTROL_MODE => fp(&["dataLockControlMode"], Codec::EnumText),
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            "Dimension" => Some(ChildLocus {
                container: &[],
                child_tag: "dimensions",
                props_wrapped: false,
                name_tag: "name",
                bare_ref: false,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        seq_bindings()
    }
}

// ===== Дитя Dimension (inline, разрежённое) =====
pub struct EdtDimension;
impl LocusMap for EdtDimension {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            dim::F_SYNONYM => fp(&["synonym"], Codec::LocalizedKeyVal),
            dim::F_COMMENT => fp(&["comment"], Codec::PlainText),
            dim::F_TYPE => fp(&["type"], Codec::Type(formats_xml::TypeDialect::Edt)),
            dim::F_DOCUMENT_MAP => fp(&["documentMap"], Codec::RefList(RefListDialect::Edt)),
            dim::F_REGISTER_RECORDS_MAP => fp(&["registerRecordsMap"], Codec::RefList(RefListDialect::Edt)),
            _ => return None,
        })
    }
}

/// `&'static` бинды child-видов родителя (кэш на процесс).
fn seq_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding { collection: "Dimension", child_spec: sequence_dimension(), child_map: &EdtDimension }]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Sequence", sequence(), &EdtSequence, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Sequence", sequence(), &EdtSequence, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/Sequence. Дитя Dimension покрыто ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "Sequence",
    read,
    write,
    corpus_subpath: "coverage/edt/s7_pending/src/Sequences",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
