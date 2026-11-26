//! Designer-проекция вида `Sequence` + его дочернего вида `Sequence.Dimension` (зеркало
//! `core/spec/metadata/sequence*.rs`, ARCHITECTURE.md §5; child-objects substrate).
//! ТОЛЬКО размещение/кодировка — канонику держит `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ
//! спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5), включая детей. Платформенный
//! `<InternalInfo>` (Record+Manager+RecordSet) фреймится каркасом коннектора
//! (`PRODUCED_CATEGORIES`).
//!
//! РОДИТЕЛЬ — пути от `<MetaDataObject>` через `<Sequence>/<Properties>`, DENSE (6 полей,
//! сверено coverage/s7_pending 6/6). `Documents`/`RegisterRecords` — контейнеры
//! `<xr:Item xsi:type="xr:MDObjectRef">` (RefList item-style; RegisterRecords витнессится
//! ТОЛЬКО пустым `<RegisterRecords/>`). Dimension — `<ChildObjects>`+`<Properties>`
//! (props_wrapped); его `DocumentMap`/`RegisterRecordsMap` витнессятся ТОЛЬКО пустыми
//! (`<DocumentMap/>`), стиль item — по аналогии с Documents (непустой → громкая ошибка
//! стиля, не silent-drop, §1.0).

use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;

use morph1c_core::spec::metadata::sequence as sq;
use morph1c_core::spec::metadata::sequence::sequence;
use morph1c_core::spec::metadata::sequence_dimension as dim;
use morph1c_core::spec::metadata::sequence_dimension::sequence_dimension;

const fn elem(path: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path, ns: "" }
}
const fn fp(path: &'static [&'static str], codec: Codec) -> FieldProjection {
    FieldProjection { locus: elem(path), codec }
}

// Пути от корня <MetaDataObject> через <Sequence>/<Properties>.
macro_rules! p {
    ($tag:literal) => {
        &["Sequence", "Properties", $tag]
    };
}

// ===== РОДИТЕЛЬ =====
pub struct DesignerSequence;

impl LocusMap for DesignerSequence {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            sq::F_SYNONYM => fp(p!("Synonym"), Codec::LocalizedV8),
            sq::F_COMMENT => fp(p!("Comment"), Codec::PlainText),
            sq::F_MOVE_BOUNDARY_ON_POSTING => fp(p!("MoveBoundaryOnPosting"), Codec::EnumText),
            sq::F_DOCUMENTS => fp(p!("Documents"), Codec::RefList(RefListDialect::DesignerItem)),
            sq::F_REGISTER_RECORDS => fp(p!("RegisterRecords"), Codec::RefList(RefListDialect::DesignerItem)),
            sq::F_DATA_LOCK_CONTROL_MODE => fp(p!("DataLockControlMode"), Codec::EnumText),
            _ => return None,
        })
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        let cont: &'static [&'static str] = &["Sequence", "ChildObjects"];
        match collection {
            "Dimension" => Some(ChildLocus {
                container: cont,
                child_tag: "Dimension",
                props_wrapped: true,
                name_tag: "Name",
                bare_ref: false,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        seq_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // ВСЕ Designer-Sequence несут `<ChildObjects>` (сверено coverage/s7_pending 6/6 —
        // минимум самозакрывающийся `<ChildObjects/>` у объектов без измерений).
        true
    }
}

// ===== Дитя Dimension (props-wrapped; пути относительны <Properties> ребёнка) =====
pub struct DesignerDimension;
impl LocusMap for DesignerDimension {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            dim::F_SYNONYM => fp(&["Synonym"], Codec::LocalizedV8),
            dim::F_COMMENT => fp(&["Comment"], Codec::PlainText),
            dim::F_TYPE => fp(&["Type"], Codec::Type(formats_xml::TypeDialect::Designer)),
            dim::F_DOCUMENT_MAP => fp(&["DocumentMap"], Codec::RefList(RefListDialect::DesignerItem)),
            dim::F_REGISTER_RECORDS_MAP => fp(&["RegisterRecordsMap"], Codec::RefList(RefListDialect::DesignerItem)),
            _ => return None,
        })
    }
}

fn seq_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding { collection: "Dimension", child_spec: sequence_dimension(), child_map: &DesignerDimension }]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Sequence", sequence(), &DesignerSequence, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Sequence", sequence(), &DesignerSequence, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/Sequence. Дитя Dimension покрыто ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "Sequence",
    read,
    write,
    corpus_subpath: "coverage/designer/s7_pending/Sequences",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
