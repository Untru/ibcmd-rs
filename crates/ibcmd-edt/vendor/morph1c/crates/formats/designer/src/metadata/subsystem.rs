//! Designer-проекция вида `Subsystem` + дочерней self-reference коллекции
//! `Subsystem.SubsystemRef` (зеркало `core/spec/metadata/subsystem.rs`, ARCHITECTURE.md §5).
//! ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec` (§1.6). Это ИНОЙ
//! синтаксис того же спека, что и EDT, поэтому оба формата дают РАВНЫЙ IR (§3.5).
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от КОРНЯ `<MetaDataObject>` через
//! `<Subsystem>/<Properties>`:
//! * `synonym` → `<Synonym>` с `v8:item/(v8:lang, v8:content)` (`LocalizedV8`);
//! * `comment` → `<Comment>текст</Comment>` (`PlainText`; пустой → `<Comment/>`);
//! * `includeHelpInContents` → `<IncludeHelpInContents>true|false</…>` (`BoolText`);
//! * `includeInCommandInterface` → `<IncludeInCommandInterface>true|false</…>` (`BoolText`);
//! * `useOneCommand` → `<UseOneCommand>true|false</…>` (`BoolText`);
//! * `explanation` → `<Explanation>` с `v8:item/...` (`LocalizedV8`; пустой → `<Explanation/>`);
//! * `picture` → `<Picture><xr:Ref>ref</xr:Ref><xr:LoadTransparent>…</…></Picture>`
//!   (`PictureRef`; пустой → `<Picture/>`);
//! * `content` → `<Content><xr:Item xsi:type="xr:MDObjectRef">Path</xr:Item>…</Content>`
//!   (`RefList(DesignerItem)`; пустой → `<Content/>`).
//!
//! Дочерняя коллекция `Subsystem` (вложенные подсистемы) — BARE-REF `<ChildObjects>
//! <Subsystem>Имя</Subsystem>…` (сиблинг `<Properties>` под `<Subsystem>`; `bare_ref`).
//!
//! Property-теги — UpperCamelCase БЕЗ префикса (дефолтный MDClasses-ns, `ns=""`);
//! внутренности (`v8:item`/`xr:Ref`/`xr:Item`) несут v8/xr — это разбирает кодек.

use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::subsystem::{
    F_COMMENT, F_CONTENT, F_EXPLANATION, F_INCLUDE_HELP_IN_CONTENTS, F_INCLUDE_IN_COMMAND_INTERFACE,
    F_PICTURE, F_SYNONYM, F_USE_ONE_COMMAND,
};
use morph1c_core::spec::metadata::subsystem_subsystem_ref::subsystem_subsystem_ref;

/// `PropElement{path, ns}` (хелпер таблицы).
const fn elem(tag: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns }
}

const P_SYNONYM: &[&str] = &["Subsystem", "Properties", "Synonym"];
const P_COMMENT: &[&str] = &["Subsystem", "Properties", "Comment"];
const P_INCLUDE_HELP: &[&str] = &["Subsystem", "Properties", "IncludeHelpInContents"];
const P_INCLUDE_IN_CI: &[&str] = &["Subsystem", "Properties", "IncludeInCommandInterface"];
const P_USE_ONE_COMMAND: &[&str] = &["Subsystem", "Properties", "UseOneCommand"];
const P_EXPLANATION: &[&str] = &["Subsystem", "Properties", "Explanation"];
const P_PICTURE: &[&str] = &["Subsystem", "Properties", "Picture"];
const P_CONTENT: &[&str] = &["Subsystem", "Properties", "Content"];

/// Карта проекции Designer для `Subsystem`.
pub struct DesignerSubsystem;

impl DesignerSubsystem {
    /// Строка проекции для поля (или `None`, если поле не из этого вида).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(elem(P_SYNONYM, ""), Codec::LocalizedV8)
        } else if field == F_COMMENT {
            FieldProjection::new(elem(P_COMMENT, ""), Codec::PlainText)
        } else if field == F_INCLUDE_HELP_IN_CONTENTS {
            FieldProjection::new(elem(P_INCLUDE_HELP, ""), Codec::BoolText)
        } else if field == F_INCLUDE_IN_COMMAND_INTERFACE {
            FieldProjection::new(elem(P_INCLUDE_IN_CI, ""), Codec::BoolText)
        } else if field == F_USE_ONE_COMMAND {
            FieldProjection::new(elem(P_USE_ONE_COMMAND, ""), Codec::BoolText)
        } else if field == F_EXPLANATION {
            FieldProjection::new(elem(P_EXPLANATION, ""), Codec::LocalizedV8)
        } else if field == F_PICTURE {
            FieldProjection::new(elem(P_PICTURE, ""), Codec::PictureRef(formats_xml::picture::PictureDialect::Designer))
        } else if field == F_CONTENT {
            FieldProjection::new(elem(P_CONTENT, ""), Codec::RefList(RefListDialect::DesignerItem))
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for DesignerSubsystem {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            // Вложенные подсистемы — BARE-REF `<ChildObjects><Subsystem>Имя</Subsystem>…`
            // (навигация от `<MetaDataObject>` → `<Subsystem>` → `<ChildObjects>`).
            "Subsystem" => Some(ChildLocus {
                container: &["Subsystem", "ChildObjects"],
                child_tag: "Subsystem",
                props_wrapped: false,
                name_tag: "Name",
                bare_ref: true,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        subsystem_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // ВСЕ Designer-Subsystem несут `<ChildObjects>` (пустой → самозакрывающийся
        // `<ChildObjects/>` при нуле вложенных подсистем — сверено по SSL top-level).
        true
    }
}

/// Бинды дочерних коллекций `Subsystem` (self-reference: bare-ref). Кэш на процесс.
fn subsystem_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding {
            collection: "Subsystem",
            child_spec: subsystem_subsystem_ref(),
            child_map: &DesignerSubsystemRef,
        }]
    })
}

/// Designer-проекция child-вида `Subsystem.SubsystemRef` (bare-ref: полей нет). БЕЗ
/// `HARNESS_ENTRY` (покрыт транзитивно через родителя).
pub struct DesignerSubsystemRef;

impl LocusMap for DesignerSubsystemRef {
    fn lookup(&self, _field: FieldId) -> Option<FieldProjection> {
        None
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::subsystem::subsystem;

/// R-read для харнесса: `.xml`-байты Subsystem → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Subsystem", subsystem(), &DesignerSubsystem, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR Subsystem → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Subsystem", subsystem(), &DesignerSubsystem, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/Subsystem. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "Subsystem",
    read,
    write,
    corpus_subpath: "coverage/designer/s4_common/Subsystems",
    // РЕКУРСИВНАЯ раскладка: подсистемы вложены (`<parent>/Subsystems/<child>.xml`), поэтому
    // плоский `FilePerObject` перечислил бы лишь 3 верхних из 87. `Nested` спускается в
    // подкаталог `Subsystems` каждого объекта → все 87 round-trip-ятся (R+X).
    layout: formats_xml::CorpusLayout::Nested {
        ext: "xml",
        nesting_dir: "Subsystems",
        dir_per_object: false,
    },
};
