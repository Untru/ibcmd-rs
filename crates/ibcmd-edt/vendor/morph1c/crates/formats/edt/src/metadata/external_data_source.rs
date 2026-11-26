//! EDT-проекция вида `ExternalDataSource` (зеркало `core/spec/metadata/external_data_source.rs`,
//! ARCHITECTURE.md §5; child-objects substrate). ТОЛЬКО размещение/кодировка ячеек — канонику
//! (id/порядок/дефолты) держит `core/spec` (§1.6).
//!
//! Карта `FieldId → (XmlLocus, Codec)` ВЫВОДИТСЯ из спека (`docs/APPROACH.md` §2.2): тег = имя
//! поля verbatim, кодек = по value_kind. Родитель сам несёт `<producedTypes>` (3 категории) —
//! каркас обрабатывает их через реестр.
//!
//! # Дочерняя коллекция `Table` — ССЫЛКИ на ОТДЕЛЬНЫЕ файлы (witness ERP BaseBU)
//! Родительский `.mdo` несёт ДОТИРОВАННЫЕ bare-ссылки
//! `<tables>ExternalDataSource.BaseBU.Table.<Имя></tables>`; ТЕЛО таблицы — отдельный файл
//! `ExternalDataSources/<EDS>/Tables/<Имя>/<Имя>.mdo` (`mdclass:Table` корень). Ссылка
//! читается bare_ref'ом (имя-стаб = дотированный текст verbatim → R byte-exact per-file);
//! НОРМАЛИЗАЦИЮ имени (срез префикса `ExternalDataSource.<EDS>.Table.`) и подчитку тела
//! делает пайплайн-пасс `pipeline::table_ref_read` (зеркало форм/подсистем; там же —
//! синтез/кросс-сверка `parentDataSource`).
//!
//! # Таблица (standalone `.mdo`) и её `Field`-дети
//! [`EdtEdsTable`] — карта standalone-файла: derive(Edt) + hand-mapped List-поля
//! (`keyFields`/`inputByString`/`basedOn`/`dataLockFields` — RefList(Edt);
//! `characteristics` — Characteristics(Edt)). Поля таблицы — INLINE `<tableFields uuid>`
//! (child-locus `Field`); [`EdtEdsField`] — derive(Edt) + choice-параметры.

use formats_xml::children::ChildBinding;
use formats_xml::derive::{projection, DeriveDialect};
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
pub struct EdtExternalDataSource;

impl LocusMap for EdtExternalDataSource {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, external_data_source(), field)
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            // Дотированные bare-ссылки `<tables>ExternalDataSource.<EDS>.Table.<Имя></tables>`
            // под корнем (witness ERP BaseBU 1/1; тело — отдельный файл, см. модульный doc).
            "Table" => Some(ChildLocus {
                container: &[],
                child_tag: "tables",
                props_wrapped: false,
                name_tag: "name",
                bare_ref: true,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        eds_bindings()
    }

    fn bare_ref_dotted(&self, collection: &str) -> bool {
        // `<tables>` несёт ПОЛНЫЙ путь `ExternalDataSource.<EDS>.Table.<Имя>` (witness ERP
        // BaseBU 1/1) — чтение срезает префикс до канон-имени, запись восстанавливает.
        collection == "Table"
    }
}

// ===== Таблица (standalone `Tables/<Имя>/<Имя>.mdo`, корень `mdclass:Table`) =====
pub struct EdtEdsTable;
impl LocusMap for EdtEdsTable {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        // List-поля не выводимы по value_kind (§2.2 residual) — hand-map:
        match field {
            tbl::F_KEY_FIELDS => fp(&["keyFields"], Codec::RefList(RefListDialect::Edt)),
            tbl::F_INPUT_BY_STRING => fp(&["inputByString"], Codec::RefList(RefListDialect::Edt)),
            tbl::F_BASED_ON => fp(&["basedOn"], Codec::RefList(RefListDialect::Edt)),
            tbl::F_DATA_LOCK_FIELDS => fp(&["dataLockFields"], Codec::RefList(RefListDialect::Edt)),
            tbl::F_CHARACTERISTICS => fp(
                &["characteristics"],
                Codec::Characteristics(formats_xml::characteristics::CharacteristicsDialect::Edt),
            ),
            _ => projection(DeriveDialect::Edt, external_data_source_table(), field),
        }
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            // Поля — INLINE `<tableFields uuid="…"><name>…` прямо под корнем таблицы
            // (witness ERP: 32/32; форма — как `<attributes>` прочих видов).
            "Field" => Some(ChildLocus {
                container: &[],
                child_tag: "tableFields",
                props_wrapped: false,
                name_tag: "name",
                bare_ref: false,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        table_bindings()
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        // STANDALONE-корень `mdclass:Table` несёт xmlns:xsi+xmlns:core (Value-xsi полей:
        // `unfilledParentValue`/`minValue`/`fillValue` xsi:type="core:…"). WITNESSED ERP
        // BaseBU/Tables/ERP_MASTER_SIVANOV_public__accumrg90126.mdo (без этого объявления
        // корневые @xmlns:xsi/@xmlns:core падали в leftover → UnconsumedInput на ВСЁМ EDS).
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
        ]
    }
}

// ===== Поле таблицы (inline `<tableFields>`) =====
pub struct EdtEdsField;
impl LocusMap for EdtEdsField {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        match field {
            fld::F_CHOICE_PARAMETER_LINKS => fp(
                &["choiceParameterLinks"],
                Codec::ChoiceParameterLinks(formats_xml::choice_param_links::LinksDialect::Edt),
            ),
            fld::F_CHOICE_PARAMETERS => fp(
                &["choiceParameters"],
                Codec::ChoiceParameters(formats_xml::choice_parameters::ChoiceParametersDialect::Edt),
            ),
            _ => projection(DeriveDialect::Edt, external_data_source_table_field(), field),
        }
    }
}

/// `&'static` бинды child-видов родителя (кэш на процесс).
fn eds_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding { collection: "Table", child_spec: external_data_source_table(), child_map: &EdtEdsTable }]
    })
}

/// `&'static` бинды child-видов таблицы (кэш на процесс).
fn table_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding { collection: "Field", child_spec: external_data_source_table_field(), child_map: &EdtEdsField }]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("ExternalDataSource", spec(), &EdtExternalDataSource, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("ExternalDataSource", spec(), &EdtExternalDataSource, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    external_data_source()
}

/// Прочитать STANDALONE-дескриптор таблицы (`Tables/<Имя>/<Имя>.mdo`, корень `mdclass:Table`) —
/// для пайплайн-пасса `table_ref_read` (per-file R byte-exact тем же спеком/картой).
pub fn read_table_descriptor(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Table", external_data_source_table(), &EdtEdsTable, bytes)
        .map_err(|e| e.to_string())
}

/// Записать STANDALONE-дескриптор таблицы (write-зеркало [`read_table_descriptor`]).
pub fn write_table_descriptor(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Table", external_data_source_table(), &EdtEdsTable, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/ExternalDataSource.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "ExternalDataSource",
    read,
    write,
    corpus_subpath: "coverage/edt/s5_reports_services/src/ExternalDataSources",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
