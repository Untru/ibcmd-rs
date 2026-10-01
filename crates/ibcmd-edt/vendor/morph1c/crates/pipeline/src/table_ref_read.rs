//! Enrich an ExternalDataSource's `Table` CHILDREN with their STANDALONE descriptor during
//! whole-config read, and re-emit that descriptor for an xml/edt target (§1.0/§1.6).
//! Зеркало [`crate::form_ref_read`] для коллекции `Tables/` — но для ОБОИХ диалектов
//! (у форм inline-стаб несёт EDT; у EDS-таблиц ОБА диалекта держат тело в отдельном файле).
//!
//! # Раскладка (witness ERP BaseBU — единственная непустая таблица корпуса)
//! * EDT: хост `ExternalDataSources/<EDS>/<EDS>.mdo` несёт дотированную bare-ссылку
//!   `<tables>ExternalDataSource.<EDS>.Table.<Имя></tables>` (канон-имя срезается
//!   `bare_ref_dotted`-механизмом каркаса); тело — `<EDS>/Tables/<Имя>/<Имя>.mdo`
//!   (`mdclass:Table` + inline `<tableFields>`).
//! * Designer: хост `ExternalDataSources/<EDS>.xml` несёт `<ChildObjects><Table>Имя</Table>`;
//!   тело — `ExternalDataSources/<EDS>/Tables/<Имя>.xml` (`MetaDataObject/Table` +
//!   `ChildObjects/Field`).
//!
//! # `parentDataSource` — асимметрия диалектов (зеркало `Subsystem.parentSubsystem`)
//! EDT несёт принадлежность В дескрипторе таблицы (`<parentDataSource>ExternalDataSource.
//! <EDS></parentDataSource>`); Designer — ТОЛЬКО положением сайдкара на диске. Поэтому
//! Designer-чтение СИНТЕЗИРУЕТ канон-свойство из позиции, EDT-чтение КРОСС-СВЕРЯЕТ
//! (расхождение = битый источник, §1.0 — не «тихо предпочесть одну сторону»).
//!
//! # cf
//! cf-кодировка EDS-таблиц НЕ витнессирована ни одним оракулом → cf-writer родителя даёт
//! типизированный отказ на непустых children (`formats_cf::metadata::external_data_source`);
//! witness снимать с erp.cf. Этот пасс для cf — no-op.
//!
//! # ИЗВЕСТНЫЙ ПРОБЕЛ (типизированный, НЕ тихий): Designer-InternalInfo standalone-таблицы
//! Designer-сайдкар несёт GeneratedType-имена с ПОЛНОЙ цепочкой
//! `ExternalDataSourceTable<Кат>.<EDS>.<Таблица>`, а generic-каркас
//! (`produced_types::read_designer_with_this_node`) валидирует `<Кат-имя>.<obj.name>` —
//! БЕЗ prefix-контекста родителя standalone-чтение таблицы отказывает типизированной
//! ошибкой имени (или тотальностью InternalInfo). Нужно протянуть produced-prefix через
//! `formats_designer::read_descriptor`/`write_descriptor` (edt-стороне префикс не нужен —
//! её `<producedTypes>` безымянный, там уже работает keying по `spec.entity`).

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::ir::{MetadataObject, ObjectKind};
use morph1c_core::spec::metadata::external_data_source_table::F_PARENT_DATA_SOURCE;

use crate::ConvertError;

/// Подкаталог сайдкаров таблиц возле хост-дескриптора.
const TABLES_DIR: &str = "Tables";
/// Канонический вид ребёнка-таблицы.
const TABLE_KIND: &str = "ExternalDataSource.Table";
/// ON-DISK элемент/корень standalone-дескриптора таблицы (оба диалекта).
const TABLE_ELEMENT: &str = "Table";

fn is_table_child(child: &MetadataObject) -> bool {
    child.kind.as_str() == TABLE_KIND
}

/// Канон-значение `parentDataSource` для таблицы EDS `eds_name`.
fn parent_ref(eds_name: &str) -> String {
    format!("ExternalDataSource.{eds_name}")
}

/// Путь standalone-дескриптора таблицы возле хоста: EDT — `<dir>/<stem>/Tables/<Имя>/<Имя>.mdo`
/// (хост `<dir>/<stem>/<stem>.mdo` — DirPerObject: `<dir>` УЖЕ каталог объекта, поэтому
/// сайдкары лежат в `<dir>/Tables/…`); Designer — `<dir>/<stem>/Tables/<Имя>.xml`.
fn table_descriptor_path(format: Format, descriptor_path: &Path, name: &str) -> Option<PathBuf> {
    let dir = descriptor_path.parent()?;
    match format {
        // EDT DirPerObject: дескриптор `…/ExternalDataSources/<EDS>/<EDS>.mdo` — родительский
        // каталог УЖЕ каталог объекта; `Tables/<Имя>/<Имя>.mdo` внутри него (witness ERP).
        Format::Edt => Some(dir.join(TABLES_DIR).join(name).join(format!("{name}.mdo"))),
        // Designer FilePerObject: дескриптор `…/ExternalDataSources/<EDS>.xml`; сайдкары —
        // в одноимённом каталоге `<EDS>/Tables/<Имя>.xml` (witness ERP).
        Format::Designer => {
            let stem = descriptor_path.file_stem()?;
            Some(dir.join(stem).join(TABLES_DIR).join(format!("{name}.xml")))
        }
        Format::Cf => None,
    }
}

/// Вставить свойство в bag С СОХРАНЕНИЕМ канонического порядка (спек-порядок = порядок id).
fn insert_ordered(obj: &mut MetadataObject, id: morph1c_core::ir::FieldId, value: PropertyValue) {
    let pos = obj
        .properties
        .iter()
        .position(|(fid, _)| *fid > id)
        .unwrap_or(obj.properties.len());
    obj.properties.insert(pos, (id, value));
}

/// Прочитать standalone-дескрипторы таблиц каждого Table-ребёнка и ТРАНСПЛАНТИРОВАТЬ их
/// содержимое (uuid + свойства + Field-дети + producedTypes) на bare-ref-стаб — IR из
/// обоих диалектов становится ИДЕНТИЧНЫМ (§1.6). cf — no-op.
///
/// §1.0-STRICT: заявленная таблица БЕЗ файла-сайдкара — ЖЁСТКАЯ ошибка (uuid/поля таблицы
/// живут только там; молчаливый стаб = потеря). Дескриптор с чужим `<name>`/чужим
/// `parentDataSource` — битый источник, тоже ошибка.
pub fn attach_table_descriptors(
    format: Format,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf || obj.kind.as_str() != "ExternalDataSource" {
        return Ok(());
    }
    let eds_name = obj.name.clone();
    for child in obj.children.iter_mut() {
        if !is_table_child(child) {
            continue;
        }
        let path = match table_descriptor_path(format, descriptor_path, &child.name) {
            Some(p) => p,
            None => continue, // defensive: нет parent/stem — нечего подчитывать.
        };
        if !path.is_file() {
            return Err(ConvertError::Read {
                kind: TABLE_KIND.to_string(),
                object: child.name.clone(),
                reason: format!(
                    "the host declares a Table child but its standalone descriptor {} is \
                     missing (§1.0 — the table's uuid/fields live ONLY in that file)",
                    path.display()
                ),
            });
        }
        let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        let mut desc = match format {
            Format::Edt => formats_edt::metadata::external_data_source::read_table_descriptor(&bytes),
            Format::Designer => {
                // derivable-имена GeneratedType таблицы несут цепочку `<Кат>.<EDS>.<Т>` —
                // префикс (имя источника) известен только здесь (позиция сайдкара).
                formats_xml::produced_types::with_produced_name_prefix(&eds_name, || {
                    formats_designer::metadata::external_data_source::read_table_descriptor(&bytes)
                })
            }
            Format::Cf => unreachable!("cf returned above"),
        }
        .map_err(|e| ConvertError::Read {
            kind: TABLE_KIND.to_string(),
            object: child.name.clone(),
            reason: format!("{} : {e}", path.display()),
        })?;
        // §1.0 membership: собственное имя дескриптора == заявленная ссылка.
        if desc.name != child.name {
            return Err(ConvertError::Read {
                kind: TABLE_KIND.to_string(),
                object: child.name.clone(),
                reason: format!(
                    "standalone table descriptor {} declares name {:?} != declared ref {:?} (§1.0)",
                    path.display(),
                    desc.name,
                    child.name
                ),
            });
        }
        // `parentDataSource`: EDT кросс-сверка / Designer синтез (см. модульный doc).
        let want = parent_ref(&eds_name);
        match desc.get(F_PARENT_DATA_SOURCE) {
            Some(PropertyValue::Str(s)) if *s == want => {}
            Some(PropertyValue::Str(s)) => {
                return Err(ConvertError::Read {
                    kind: TABLE_KIND.to_string(),
                    object: child.name.clone(),
                    reason: format!(
                        "table descriptor declares parentDataSource {s:?} but it sits under \
                         {want:?} on disk (§1.0 — the two identities disagree)"
                    ),
                });
            }
            Some(other) => {
                return Err(ConvertError::Read {
                    kind: TABLE_KIND.to_string(),
                    object: child.name.clone(),
                    reason: format!("parentDataSource is not a string: {:?}", other.kind()),
                });
            }
            // Designer не несёт свойства → синтез из позиции на диске (канон = EDT-форма).
            None => insert_ordered(&mut desc, F_PARENT_DATA_SOURCE, PropertyValue::Str(want)),
        }
        // Трансплантация: bare-ref-стаб уже несёт правильные kind/name; остальное — из файла.
        desc.kind = ObjectKind::new(TABLE_KIND);
        *child = desc;
    }
    Ok(())
}

/// Write-зеркало [`attach_table_descriptors`]: для xml/edt-цели эмитировать standalone-
/// дескриптор каждой таблицы возле только что записанного хоста. Без него хост несёт
/// bare-ссылку в никуда (платформенная загрузка падает «Файл объекта не существует»).
/// cf — no-op (cf-запись непустого EDS отказывает типизированно в cf-коннекторе).
pub fn write_table_descriptors(
    format: Format,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf || obj.kind.as_str() != "ExternalDataSource" {
        return Ok(());
    }
    for child in &obj.children {
        if !is_table_child(child) {
            continue;
        }
        let path = match table_descriptor_path(format, descriptor_out, &child.name) {
            Some(p) => p,
            None => continue,
        };
        // Тот же спек/карта, что на чтении → сайдкар round-trip'ится byte-exact.
        // `write_descriptor` сверяет `obj.kind == element` → клон со standalone-видом `Table`.
        let mut table = child.clone();
        table.kind = ObjectKind::new(TABLE_ELEMENT);
        let bytes = match format {
            Format::Edt => {
                formats_edt::metadata::external_data_source::write_table_descriptor(&table)
            }
            Format::Designer => {
                formats_xml::produced_types::with_produced_name_prefix(&obj.name, || {
                    formats_designer::metadata::external_data_source::write_table_descriptor(&table)
                })
            }
            Format::Cf => unreachable!("cf returned above"),
        }
        .map_err(|e| ConvertError::Write {
            kind: TABLE_KIND.to_string(),
            object: child.name.clone(),
            reason: e.to_string(),
        })?;
        crate::form_write::write_file(&path, &bytes)?;
    }
    Ok(())
}
