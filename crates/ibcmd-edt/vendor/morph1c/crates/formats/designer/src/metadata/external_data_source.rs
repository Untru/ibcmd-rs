//! Designer-проекция вида `ExternalDataSource` (зеркало
//! `core/spec/metadata/external_data_source.rs`, ARCHITECTURE.md §5; child-objects substrate).
//! ТОЛЬКО размещение/кодировка — канонику держит `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ
//! спека, что EDT → оба формата дают РАВНЫЙ IR (§3.5).
//!
//! # Дочерняя коллекция `Table` — bare-ссылки + ОТДЕЛЬНЫЙ сайдкар (witness ERP BaseBU)
//! Родительский `.xml` несёт `<ChildObjects><Table>Имя</Table></ChildObjects>` (bare-ссылка,
//! как формы/шаблоны); ТЕЛО таблицы — отдельный сайдкар
//! `ExternalDataSources/<EDS>/Tables/<Имя>.xml` (`MetaDataObject/Table` с InternalInfo
//! (8 категорий producedTypes) + Properties + ChildObjects/Field). Подчитку сайдкара делает
//! пайплайн-пасс `pipeline::table_ref_read` (зеркало `form_ref_read`); `parentDataSource`
//! Designer НЕ несёт (принадлежность = положение на диске) — пасс синтезирует его в IR
//! (зеркало `Subsystem.parentSubsystem`).
//!
//! [`DesignerEdsTable`] — карта standalone-сайдкара (пути `["Table","Properties",<Tag>]`,
//! derived-по-спеку + hand List-поля); [`DesignerEdsField`] — `Field`-дети
//! (`ChildObjects/Field`, single-segment пути относительно `<Properties>`).

use std::sync::OnceLock;

use formats_xml::children::ChildBinding;
use formats_xml::derive::{codec_for, projection, DeriveDialect};
use formats_xml::ref_list::RefListDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::EntitySpec;

use morph1c_core::spec::metadata::external_data_source::external_data_source;
use morph1c_core::spec::metadata::external_data_source_table::{self as tbl, external_data_source_table};
use morph1c_core::spec::metadata::external_data_source_table_field::{self as fld, external_data_source_table_field};

fn fp(path: &'static [&'static str], codec: Codec) -> Option<FieldProjection> {
    Some(FieldProjection::new(XmlLocus::PropElement { path, ns: "" }, codec))
}

// ===== РОДИТЕЛЬ ExternalDataSource =====
pub struct DesignerExternalDataSource;

impl LocusMap for DesignerExternalDataSource {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Designer, external_data_source(), field)
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            // Bare-ссылка `<Table>Имя</Table>` (witness ERP BaseBU.xml 1/1; тело — сайдкар).
            "Table" => Some(ChildLocus {
                container: &["ExternalDataSource", "ChildObjects"],
                child_tag: "Table",
                props_wrapped: false,
                name_tag: "Name",
                bare_ref: true,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        eds_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // Designer-ExternalDataSource ВСЕГДА несёт обёртку `<ChildObjects>`: без таблиц →
        // самозакрывающийся `<ChildObjects/>` (coverage 3/3), с таблицами — bare-ссылки
        // (ERP BaseBU 1/1).
        true
    }
}

/// lowerCamel → UpperCamel (первый символ; зеркало приватного `derive::upper_camel`).
fn upper_camel(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Derived-таблица standalone-сайдкара таблицы: пути `["Table","Properties",<UpperCamel>]`
/// (первый сегмент — ON-DISK элемент `Table`, а не spec.entity — потому не
/// `DeriveDialect::Designer` напрямую), кодек — общий `codec_for` (Designer-таблица кодеков).
/// List-поля residual — hand-override в [`DesignerEdsTable::lookup`].
fn table_derived(field: FieldId) -> Option<FieldProjection> {
    static TABLE: OnceLock<Vec<(FieldId, FieldProjection)>> = OnceLock::new();
    let rows = TABLE.get_or_init(|| {
        let mut rows = Vec::new();
        for fs in external_data_source_table().fields() {
            // `parentDataSource` — EDT-only: Designer выражает принадлежность положением
            // сайдкара на диске (синтез — в пайплайне, зеркало Subsystem.parentSubsystem).
            if fs.id == tbl::F_PARENT_DATA_SOURCE {
                continue;
            }
            let Some(codec) = codec_for(DeriveDialect::Designer, fs.value_kind) else {
                continue; // List-residual — hand-map в lookup.
            };
            let tag: &'static str = Box::leak(upper_camel(fs.name).into_boxed_str());
            let path: &'static [&'static str] =
                Box::leak(vec!["Table", "Properties", tag].into_boxed_slice());
            rows.push((fs.id, FieldProjection::new(XmlLocus::PropElement { path, ns: "" }, codec)));
        }
        rows
    });
    rows.iter().find(|(id, _)| *id == field).map(|(_, p)| p.clone())
}

// ===== Таблица (standalone-сайдкар `Tables/<Имя>.xml`, элемент `<Table>`) =====
pub struct DesignerEdsTable;
impl LocusMap for DesignerEdsTable {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        match field {
            tbl::F_KEY_FIELDS => fp(
                &["Table", "Properties", "KeyFields"],
                Codec::RefList(RefListDialect::DesignerField),
            ),
            tbl::F_INPUT_BY_STRING => fp(
                &["Table", "Properties", "InputByString"],
                Codec::RefList(RefListDialect::DesignerField),
            ),
            // item-style — как `Catalog.basedOn` (witness пуст: `<BasedOn/>` — форма одна).
            tbl::F_BASED_ON => fp(
                &["Table", "Properties", "BasedOn"],
                Codec::RefList(RefListDialect::DesignerItem),
            ),
            tbl::F_DATA_LOCK_FIELDS => fp(
                &["Table", "Properties", "DataLockFields"],
                Codec::RefList(RefListDialect::DesignerField),
            ),
            tbl::F_CHARACTERISTICS => fp(
                &["Table", "Properties", "Characteristics"],
                Codec::Characteristics(formats_xml::characteristics::CharacteristicsDialect::Designer),
            ),
            _ => table_derived(field),
        }
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            // Поля — `<ChildObjects><Field uuid><Properties>` (witness ERP 32/32).
            "Field" => Some(ChildLocus {
                container: &["Table", "ChildObjects"],
                child_tag: "Field",
                props_wrapped: true,
                name_tag: "Name",
                bare_ref: false,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        table_bindings()
    }
}

// ===== Поле таблицы (`ChildObjects/Field`) =====
pub struct DesignerEdsField;
impl LocusMap for DesignerEdsField {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        match field {
            fld::F_CHOICE_PARAMETER_LINKS => fp(
                &["ChoiceParameterLinks"],
                Codec::ChoiceParameterLinks(formats_xml::choice_param_links::LinksDialect::Designer),
            ),
            fld::F_CHOICE_PARAMETERS => fp(
                &["ChoiceParameters"],
                Codec::ChoiceParameters(formats_xml::choice_parameters::ChoiceParametersDialect::Designer),
            ),
            _ => projection(DeriveDialect::DesignerChild, external_data_source_table_field(), field),
        }
    }
}

/// `&'static` бинды child-видов родителя (кэш на процесс).
fn eds_bindings() -> &'static [ChildBinding] {
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding { collection: "Table", child_spec: external_data_source_table(), child_map: &DesignerEdsTable }]
    })
}

/// `&'static` бинды child-видов таблицы (кэш на процесс).
fn table_bindings() -> &'static [ChildBinding] {
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding { collection: "Field", child_spec: external_data_source_table_field(), child_map: &DesignerEdsField }]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("ExternalDataSource", spec(), &DesignerExternalDataSource, bytes)
        .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("ExternalDataSource", spec(), &DesignerExternalDataSource, obj)
        .map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    external_data_source()
}

/// Прочитать STANDALONE-сайдкар таблицы (`Tables/<Имя>.xml`, элемент `<Table>`) — для
/// пайплайн-пасса `table_ref_read` (per-file R byte-exact тем же спеком/картой).
pub fn read_table_descriptor(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Table", external_data_source_table(), &DesignerEdsTable, bytes)
        .map_err(|e| e.to_string())
}

/// Записать STANDALONE-сайдкар таблицы (write-зеркало [`read_table_descriptor`]).
pub fn write_table_descriptor(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Table", external_data_source_table(), &DesignerEdsTable, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/ExternalDataSource.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "ExternalDataSource",
    read,
    write,
    corpus_subpath: "coverage/designer/s5_reports_services/ExternalDataSources",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
