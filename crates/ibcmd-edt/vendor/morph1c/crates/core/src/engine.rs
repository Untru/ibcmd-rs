//! Spec-driven движок чтения/записи (ARCHITECTURE.md §1.4, §1.6, §2.1).
//!
//! ЕДИНСТВЕННЫЙ путь read/write для ЛЮБОГО формата. Движок walk'ает КАНОНИЧЕСКИЙ
//! [`EntitySpec`] (порядок полей = порядок эмиссии) и для каждого поля спрашивает
//! ПРОЕКЦИЮ формата: «где это поле у тебя лежит и как его декодировать/закодировать».
//! Формат отвечает только про РАЗМЕЩЕНИЕ и КОДИРОВКУ ячейки — `id`, тип значения,
//! порядок и омиссию дефолтов решает спек, не формат.
//!
//! Так §1.6 обеспечивается СТРУКТУРНО (а не дисциплиной):
//! * IR всегда ключуется `FieldSpec.id` из ОБЩЕГО спека → одинаковые ключи у всех
//!   форматов;
//! * порядок свойств = порядок полей спека → одинаковый порядок у всех форматов;
//! * декодированное значение ОБЯЗАНО совпасть с `FieldSpec.value_kind`, иначе
//!   [`EngineError::KindMismatch`] → формат не волен выбрать иной тип;
//! * дефолты опускаются по `FieldSpec.default` единообразно.
//!
//! Отсюда теорема в миниатюре: ДВЕ разные проекции ОДНОГО спека дают РАВНЫЙ IR
//! (тест `cross_projection_equality`). Рукописный пер-форматный ридер, который мог
//! бы это нарушить, в этой архитектуре просто негде разместить.
//!
//! Тотальность (§1.0/§2.3): неразобранная/нетипизированная ячейка → типизированная
//! [`EngineError`], НИКОГДА не silent-skip и не `Raw`. Опаковый бинарь проходит как
//! [`PropertyValue::Blob`] — это полное, а не зазорное поведение.

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::EntitySpec;
use crate::version::FormatVersion;

/// Ошибка движка — типизированная, без escape-hatch (§1.0). Любая нетипизируемая
/// ячейка останавливает прогон ЯВНО.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineError {
    /// Поле спека обязательно (нет `default`), но проекция его не нашла на входе.
    MissingRequired {
        /// Сущность, в которой случилась ошибка.
        entity: &'static str,
        /// Канонический id отсутствующего поля.
        field: FieldId,
        /// Каноническое имя отсутствующего поля.
        name: &'static str,
    },
    /// Декодированное значение НЕ совпало с объявленным `value_kind` спека —
    /// формат попытался выбрать иной тип (нарушение §1.6). ЯВНАЯ ошибка.
    KindMismatch {
        /// Сущность.
        entity: &'static str,
        /// Канонический id поля.
        field: FieldId,
        /// Что объявил спек.
        expected: ValueKind,
        /// Что вернула проекция.
        got: ValueKind,
    },
    /// Проекция сообщила о собственной ошибке декодирования/кодирования
    /// (нечитаемая ячейка, неизвестный slot, …) — пробрасывается как типизированная.
    Projection {
        /// Сущность.
        entity: &'static str,
        /// Канонический id поля (если применимо).
        field: Option<FieldId>,
        /// Человекочитаемая причина от проекции.
        reason: String,
    },
    /// На входе остались ячейки, не покрытые НИ ОДНИМ полем спека (§1.0: нет
    /// passthrough — неразобранное → ошибка, не `Raw`).
    UnconsumedInput {
        /// Сущность.
        entity: &'static str,
        /// Сколько ячеек осталось неразобранными.
        leftover: usize,
        /// Имена (пути) неразобранных элементов — для диагностики (какие поля добрать).
        /// Может быть пустым у проекций, не поддерживающих перечисление.
        names: Vec<String>,
    },
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::MissingRequired { entity, field, name } => {
                write!(f, "{entity}: required field {field} ({name}) missing in source")
            }
            EngineError::KindMismatch { entity, field, expected, got } => write!(
                f,
                "{entity}: field {field} value-kind mismatch: spec says {expected:?}, format produced {got:?}"
            ),
            EngineError::Projection { entity, field, reason } => match field {
                Some(fid) => write!(f, "{entity}: projection error at field {fid}: {reason}"),
                None => write!(f, "{entity}: projection error: {reason}"),
            },
            EngineError::UnconsumedInput { entity, leftover, names } => write!(
                f,
                "{entity}: {leftover} source cell(s) not covered by any spec field \
                 (no passthrough/Raw allowed — §1.0){}",
                if names.is_empty() {
                    String::new()
                } else {
                    format!(" — uncovered: [{}]", names.join(", "))
                }
            ),
        }
    }
}

impl std::error::Error for EngineError {}

/// Результат движка.
pub type Result<T> = std::result::Result<T, EngineError>;

/// Что проекция вернула за одно поле при ЧТЕНИИ.
///
/// Это и есть граница §1.0/§1.6: проекция возвращает РАЗОБРАННОЕ значение, либо
/// «поля нет», либо собственную ошибку — НО НЕ «сырьё на потом». Нетипизируемое
/// она обязана либо отдать как [`PropertyValue::Blob`], либо вернуть [`Decoded::Error`].
pub enum Decoded {
    /// Поле присутствует и разобрано в типизированное значение.
    Present(PropertyValue),
    /// Поля нет на входе (движок подставит `default` или поднимет MissingRequired).
    Absent,
    /// Проекция не смогла разобрать ячейку — типизированная ошибка, не `Raw`.
    Error(String),
}

/// Проекция формата: как КОНКРЕТНЫЙ формат размещает и кодирует поля спека.
///
/// Это ЕДИНСТВЕННОЕ, что формат привносит сверх общего спека. Реализация для XML
/// знает имена тегов/атрибутов; для `.cf` — индексы slot'ов в brace-записи; для
/// СУБД — столбцы. Но НИ ОДНА реализация не решает `id`/тип/порядок — это
/// гарантирует §1.6 на уровне типов: методы получают `FieldId`/`ValueKind` ИЗ
/// спека и обязаны им соответствовать.
///
/// `Source`/`Source` — формат-специфичные «сырые» носители (brace-запись, поддерево
/// XML, строка БД); движок их не интерпретирует, лишь передаёт проекции.
pub trait Projection {
    /// Формат-специфичный источник одного объекта (напр. brace-запись / XML-узел).
    type Source;
    /// Формат-специфичный приёмник одного объекта.
    type Sink;

    /// Формат выгрузки, который эта проекция читает/пишет (для `Since`-гейтинга).
    fn format_version(&self) -> FormatVersion;

    /// Декодировать ОДНО поле из источника по его каноническому id/ожидаемому виду.
    ///
    /// Проекция локализует ячейку (тег/slot/столбец) и декодирует её. Возвращает
    /// [`Decoded`]. `expected` передаётся, чтобы проекция знала ЦЕЛЕВОЙ вид (напр.
    /// строку "true" → `Bool(true)`), но окончательную проверку вида делает движок.
    fn decode_field(&self, source: &Self::Source, field: FieldId, expected: ValueKind) -> Decoded;

    /// Закодировать и разместить ОДНО поле в приёмнике по его каноническому id.
    ///
    /// Вызывается движком для КАЖДОГО поля, доступного в целевой версии (`Since`,
    /// §1.5) — включая поля, чьё эффективное значение РАВНО дефолту. Эмитить ли
    /// дефолт — РЕШАЕТ ПРОЕКЦИЯ (F2b-1): `is_default` сообщает, что значение совпало
    /// со `spec.default` (взято из дефолта, т.к. отсутствует в bag, ЛИБО присутствует
    /// в bag но равно дефолту). Разреженная проекция (EDT) при `is_default == true`
    /// просто `Ok(())` без записи (re-sparsify); плотная (Designer) эмитит всегда.
    ///
    /// Движок при этом не страдает §1.6: READ всё равно даёт канонический разрежённый
    /// IR; WRITE лишь ВОССТАНАВЛИВАЕТ конвенцию формата (§1.1).
    fn encode_field(
        &self,
        sink: &mut Self::Sink,
        field: FieldId,
        value: &PropertyValue,
        is_default: bool,
    ) -> std::result::Result<(), String>;

    /// Сколько «сырых» ячеек источника НЕ привязано ни к одному id спека.
    ///
    /// Движок требует, чтобы это было 0 (§1.0: нет passthrough). Реализация считает
    /// ячейки, которые она НЕ сопоставила полям спека за время `decode_field`.
    /// (Для синтетических/простых проекций — обычно 0.)
    fn leftover(&self, source: &Self::Source) -> usize;

    /// Имена (пути) неразобранных ячеек для диагностики [`EngineError::UnconsumedInput`].
    /// Дефолт `&[]` — проекции без перечисления (простые/синтетические). XML-проекции
    /// переопределяют через `Element::unclaimed_names`.
    fn leftover_names(&self, _source: &Self::Source) -> Vec<String> {
        Vec::new()
    }

    /// ПЕР-ФОРМАТНЫЙ дефолт поля, ПЕРЕОПРЕДЕЛЯЮЩИЙ канонический `FieldSpec.default` ТОЛЬКО
    /// в этой проекции (форм-атрибуты L1f). Дефолт `None` ⇒ используется канонический
    /// `fs.default` (поведение ВСЕХ существующих видов — байт-идентично, изменений нет).
    ///
    /// # Зачем (форм-дивергенция дефолтов)
    /// Метаданные имеют ОДИН дефолт на поле: EDT (sparse) опускает его, Designer (dense)
    /// эмитит — но дефолт ОДИН. Формы РАЗНЫЕ: EDT и Designer несут ПРОТИВОПОЛОЖНЫЕ дефолты
    /// для одного атрибута (сверено по корпусу: `autoTitle` EDT-default=false, Designer-
    /// default=true; и весь auto/save/enable-семейство). Один `fs.default` не способен
    /// восстановить КАНОНИЧЕСКОЕ значение из отсутствия в ОБОИХ форматах.
    ///
    /// # Как движок это использует (§1.6 не страдает)
    /// * READ: ячейка ОТСУТСТВУЕТ ⇒ значение = `field_default(f).unwrap_or(fs.default)`
    ///   (дефолт ИМЕННО ЭТОГО формата). Затем — как обычно: нормализация и сжатие против
    ///   КАНОНИЧЕСКОГО `fs.default` ⇒ оба формата дают РАВНЫЙ разрежённый bag (X by
    ///   construction). Пример: форма с `autoTitle=false` — EDT-absent→false, Designer
    ///   `false`→false; оба == канон-дефолт(false) ⇒ оба опускают ⇒ X-равны.
    /// * WRITE: `is_default` считается против ПЕР-ФОРМАТНОГО дефолта (а не канонического),
    ///   поэтому КАЖДЫЙ формат re-sparsify'ит против СВОЕГО дефолта (EDT опускает то, что
    ///   == EDT-дефолт; Designer — то, что == Designer-дефолт). Эффективное значение при
    ///   отсутствии в bag берётся из КАНОНИЧЕСКОГО `fs.default` (он определяет, что вообще
    ///   опущено из bag).
    ///
    /// Канонический IR остаётся ЕДИН (§1.6): это лишь конвенция sparse/dense КАЖДОГО
    /// формата, как `XmlLocus` — лишь имя тега.
    fn field_default(&self, _field: FieldId) -> Option<PropertyValue> {
        None
    }
}

/// Прочитать упорядоченный property-bag сущности по спеку через проекцию.
///
/// Walk'ает ТОЛЬКО `spec.fields` в каноническом порядке. Для каждого поля:
/// 1. проекция декодирует ячейку;
/// 2. движок проверяет `value_kind` (§1.6) — несовпадение = ошибка;
/// 3. движок применяет `FieldSpec.normalize` — внутри-видовую канонизацию (§1.6),
///    чтобы X-равенство было СТРУКТУРНЫМ (напр. сорт языков `Localized` по lang);
/// 4. отсутствие → `default` (если есть) либо `MissingRequired`;
/// 5. (нормализованное) значение, равное дефолту, в bag НЕ кладётся (§1.1 «дефолты
///    не хранятся»): IR каноничен и РАЗРЕЖЕН независимо от того, эмитил ли формат
///    дефолт явно — так sparse-EDT и dense-Designer схлопываются в ОДИН IR (§1.6).
///
/// Возвращает `Vec<(FieldId, PropertyValue)>` в порядке спека — готовый
/// property-bag для [`crate::ir::MetadataObject`] / [`crate::ir::FormItem`].
pub fn read<P: Projection>(
    spec: &EntitySpec,
    proj: &P,
    source: &P::Source,
) -> Result<Vec<(FieldId, PropertyValue)>> {
    // §1.0: ни одна ячейка источника не должна остаться вне спека.
    let leftover = proj.leftover(source);
    if leftover != 0 {
        return Err(EngineError::UnconsumedInput {
            entity: spec.entity,
            leftover,
            names: proj.leftover_names(source),
        });
    }

    let mut bag = Vec::with_capacity(spec.fields.len());
    for fs in spec.fields() {
        let value = match proj.decode_field(source, fs.id, fs.value_kind) {
            Decoded::Present(v) => v,
            // §1.1: отсутствие = дефолт. Берём ПЕР-ФОРМАТНЫЙ дефолт, если проекция его
            // задаёт (форм-дивергенция, §1.6), иначе канонический `fs.default`. Полученное
            // значение дальше нормализуется и СЖИМАЕТСЯ против КАНОНИЧЕСКОГО `fs.default`
            // (ниже), поэтому оба формата дают РАВНЫЙ разрежённый bag.
            Decoded::Absent => match proj.field_default(fs.id).or_else(|| fs.default.clone()) {
                Some(v) => v,
                None => {
                    return Err(EngineError::MissingRequired {
                        entity: spec.entity,
                        field: fs.id,
                        name: fs.name,
                    });
                }
            },
            Decoded::Error(reason) => {
                return Err(EngineError::Projection {
                    entity: spec.entity,
                    field: Some(fs.id),
                    reason,
                });
            }
        };

        // §1.6: вид значения диктует спек, не формат. Проверяем ДО нормализации —
        // нормализация канонизирует нагрузку, не меняет вид.
        let got = value.kind();
        if got != fs.value_kind {
            return Err(EngineError::KindMismatch {
                entity: spec.entity,
                field: fs.id,
                expected: fs.value_kind,
                got,
            });
        }

        // §1.6: внутри-видовая канонизация ДО дефолт-сравнения. Делает X-равенство
        // структурным: два формата с расходящейся «сырой» формой (порядок языков и
        // т.п.) сходятся к ОДНОМУ каноническому значению — не дисциплиной проекций.
        let value = fs.normalize(value);

        // §1.1: (нормализованное) значение, совпавшее с дефолтом, опускаем — IR
        // каноничен и разрежен вне зависимости от того, эмитил ли формат дефолт явно.
        if fs.is_default(&value) {
            continue;
        }

        bag.push((fs.id, value));
    }
    Ok(bag)
}

/// Записать упорядоченный property-bag сущности по спеку через проекцию.
///
/// Walk'ает ТОЛЬКО `spec.fields` в каноническом порядке (детерминированный вывод,
/// §2.4). Для каждого поля:
/// 1. опускает поле, недоступное в целевой версии формата (`Since`-гейт, §1.5);
/// 2. эффективное значение = значение из bag, иначе — `default` спека; поле без
///    дефолта, отсутствующее в bag, пропускается (нечего восстанавливать);
/// 3. проверяет `value_kind` (§1.6) — несовпадение = ошибка, не молчаливая запись;
/// 4. передаёт значение проекции ВМЕСТЕ с флагом `is_default` — РЕШЕНИЕ об эмиссии
///    дефолта принимает ПРОЕКЦИЯ (F2b-1): разрежённый формат (EDT) дефолт опустит,
///    плотный (Designer) эмитит всегда. Движок свой безусловный default-skip снял.
///
/// §1.6 не страдает: [`read`] всё равно даёт канонический разрежённый IR; запись
/// лишь восстанавливает конвенцию формата (§1.1). Симметрия с [`read`] (§2.1):
/// один спек → и read, и write одного формата.
pub fn write<P: Projection>(
    spec: &EntitySpec,
    proj: &P,
    bag: &[(FieldId, PropertyValue)],
    sink: &mut P::Sink,
) -> Result<()> {
    let target = proj.format_version();
    for fs in spec.fields() {
        // §1.5: поле, введённое версией новее цели, не эмитится.
        if let Some(since) = fs.since {
            if !since.available_in(target) {
                continue;
            }
        }

        // Эффективное значение: bag → иначе spec.default. Поле без дефолта и без
        // значения в bag эмитить нечем — пропускаем (для каноничного bag из `read`
        // обязательные поля всегда присутствуют, иначе read поднял MissingRequired).
        let provided = bag.iter().find(|(k, _)| *k == fs.id).map(|(_, v)| v);
        let value = match (provided, fs.default.as_ref()) {
            (Some(v), _) => v,
            (None, Some(d)) => d,
            (None, None) => continue,
        };

        // §1.6: запись тоже сверяет вид — формат не пишет «что попало».
        let got = value.kind();
        if got != fs.value_kind {
            return Err(EngineError::KindMismatch {
                entity: spec.entity,
                field: fs.id,
                expected: fs.value_kind,
                got,
            });
        }

        // F2b-1: эмитить ли дефолт — решает проекция. Сообщаем ей, совпало ли значение с
        // дефолтом ЭТОГО формата: пер-форматный `field_default` (форм-дивергенция, §1.6),
        // иначе канонический `fs.default`. Так КАЖДЫЙ формат re-sparsify'ит против СВОЕГО
        // дефолта (EDT опускает то, что == EDT-дефолт; Designer — то, что == Designer-
        // дефолт). Для всех существующих видов `field_default==None` ⇒ поведение прежнее.
        let is_default = match proj.field_default(fs.id) {
            Some(fd) => *value == fd,
            None => fs.is_default(value),
        };

        proj.encode_field(sink, fs.id, value, is_default)
            .map_err(|reason| EngineError::Projection {
                entity: spec.entity,
                field: Some(fs.id),
                reason,
            })?;
    }
    Ok(())
}
