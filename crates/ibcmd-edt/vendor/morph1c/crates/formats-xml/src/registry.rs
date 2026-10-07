//! Тип строки харнесса `(формат × вид)` (ARCHITECTURE.md §5; ORCHESTRATION «два агента
//! НЕ редактируют одни файлы»).
//!
//! # Зачем здесь
//! `formats-xml` — общий субстрат, от которого зависят И `edt`, И `designer`. Тип
//! [`FormatKind`] объявлен ЗДЕСЬ, поэтому строки всех форматов однотипны и testkit
//! сливает их в один реестр для R/X-харнесса.
//!
//! # Как собирается реестр (codegen, НЕ inventory)
//! Сам СПИСОК строк каждого формата генерируется `build.rs` этого формата из файлов
//! `src/metadata/*.rs` в ссылаемый `pub static FORMAT_KINDS: &[FormatKind]` (см.
//! `formats/<format>/build.rs`). Распределённая линкер-секция (inventory) была отвергнута:
//! в нашем графе `rlib`-ов модуль вида, на символы которого никто не ссылается,
//! ВЫРЕЗАЕТСЯ DCE вместе с саморегистрацией — вид молча исчезает из реестра (проверено).
//! Codegen-массив — обычный ссылаемый `const`, надёжен by construction.
//!
//! # Что несёт строка
//! [`FormatKind`]: формат, канонический код вида, type-erased `read`/`write` (форматные
//! ошибки → `String`, чтобы не текли в общий тип), относительный путь корпуса и
//! [`CorpusLayout`]. Этого достаточно для `roundtrip_corpus` (R) и `cross_format_equal`
//! (X) обобщённо.

use morph1c_core::ir::MetadataObject;

/// Формат-источник строки реестра.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Format {
    /// EDT (`.mdo` исходники).
    Edt,
    /// Designer-XML (`.xml` выгрузка Конфигуратора).
    Designer,
    /// Бинарный `.cf` (контейнер v8 + brace-дескрипторы). Объекты — записи ВНУТРИ
    /// единого контейнера (`CorpusLayout::Container`), а не файл-на-объект.
    Cf,
}

impl Format {
    /// Короткий стабильный код формата (для диагностики/сообщений тестов).
    pub fn code(self) -> &'static str {
        match self {
            Format::Edt => "edt",
            Format::Designer => "designer",
            Format::Cf => "cf",
        }
    }
}

/// Раскладка корпуса вида в формате — как харнессу перечислять файлы и извлекать ИМЯ
/// объекта (ключ X-сопоставления EDT↔Designer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusLayout {
    /// EDT: один каталог на объект, файл внутри — `<Name>/<Name>.<ext>` (`mdo`).
    /// Имя объекта = имя каталога.
    DirPerObject {
        /// Расширение файла дескриптора внутри каталога объекта (`"mdo"`).
        ext: &'static str,
    },
    /// Designer: один файл на объект в общем каталоге — `<Name>.<ext>` (`xml`).
    /// Имя объекта = stem файла.
    FilePerObject {
        /// Расширение файла дескриптора (`"xml"`).
        ext: &'static str,
    },
    /// РЕКУРСИВНАЯ само-referential раскладка (Subsystem): объекты вида лежат НЕ только
    /// на верхнем уровне каталога, но и во ВЛОЖЕННОМ подкаталоге `nesting_dir` КАЖДОГО
    /// объекта — произвольной глубины (`<parent>/Subsystems/<child>/…`). Плоские
    /// `DirPerObject`/`FilePerObject` перечислили бы лишь верхний уровень (SSL: 3 из 87
    /// подсистем), оставив 84 вложенные НЕпокрытыми R/X. Существующие варианты НЕ трогаются
    /// (аддитивно) — на этот вариант указывает ТОЛЬКО Subsystem.
    ///
    /// Базовое перечисление НА КАЖДОМ уровне — как `DirPerObject` (`dir_per_object=true`,
    /// EDT: подкаталог `<Name>` с `<Name>/<Name>.ext`) либо `FilePerObject`
    /// (`dir_per_object=false`, Designer: файл `<Name>.ext`); из объекта `<Name>`
    /// перечислитель рекурсирует в `<Name>/<nesting_dir>` (каталог объекта = `<dir>/<Name>`
    /// в ОБОИХ режимах — у EDT это сам подкаталог объекта, у Designer — одноимённый stem'у
    /// подкаталог рядом с файлом `<Name>.ext`). Литеральные `nesting_dir`-каталоги как
    /// объекты НЕ перечисляются (у них нет одноимённого дескриптора), поэтому не путаются с
    /// объектами уровня.
    ///
    /// ИМЯ объекта (ключ X-сопоставления EDT↔Designer) = ИЕРАРХИЧЕСКИЙ путь имён-родителей
    /// и собственного имени, склеенный `/` (`СтандартныеПодсистемы/БазоваяФункциональность`).
    /// Собственное имя НЕ уникально глобально (в SSL 3 имени повторяются во вложенных
    /// ветках — напр. `БазоваяФункциональность`/`Печать`/`КонтрольРаботыПользователей`
    /// встречаются дважды), поэтому «ключ = собственное имя» дало бы КОЛЛИЗИЮ в X-мапе
    /// (`BTreeMap` схлопнул бы 87→84). Иерархический путь делает ключ уникальным И идентичным
    /// между EDT и Designer (иерархия имён общая; различается лишь физика дескриптора).
    /// `nesting_dir`-сегменты в ключ НЕ входят (они одинаковы у форматов, но избыточны).
    Nested {
        /// Расширение файла дескриптора (`"mdo"` EDT / `"xml"` Designer).
        ext: &'static str,
        /// Имя вложенного подкаталога, в который перечислитель рекурсирует (`"Subsystems"`).
        nesting_dir: &'static str,
        /// `true` → базовый режим уровня = `DirPerObject` (`<Name>/<Name>.ext`, EDT);
        /// `false` → `FilePerObject` (`<Name>.ext`, Designer).
        dir_per_object: bool,
    },
    /// SINGLETON-корень: вид присутствует в корпусе РОВНО ОДИН раз (Configuration root —
    /// N=1 в SSL), его дескриптор — ЕДИНСТВЕННЫЙ файл с ФИКСИРОВАННЫМ путём внутри
    /// `corpus_subpath`, а не `<Name>.<ext>`/`<Name>/<Name>.<ext>`. Нужен, потому что
    /// корневой файл соседствует с НЕ-его-вида файлами (Designer `cf/Configuration.xml`
    /// рядом с `ConfigDumpInfo.xml`; EDT `src/Configuration/` рядом с папками прочих видов),
    /// и `FilePerObject`/`DirPerObject` подхватили бы чужие. Имя объекта — ЗАДАНО (`name`),
    /// не выводится из файла. Перечисление: ровно `[(name, corpus_subpath/file)]`, если файл
    /// существует, иначе пусто (infra-skip).
    SingletonFile {
        /// Относительный путь дескриптора ВНУТРИ `corpus_subpath` (`"Configuration.mdo"`/
        /// `"Configuration.xml"`). Прямые слэши; нормализуются под ОС перечислителем.
        file: &'static str,
        /// Каноническое имя объекта (= ключ X-сопоставления EDT↔Designer), напр.
        /// `"Configuration"`. Не выводится из файла — корень безымянен в раскладке.
        name: &'static str,
    },
    /// cf: ЕДИНЫЙ `.cf`-контейнер (`corpus_subpath` указывает на файл, не каталог),
    /// внутри которого объекты — brace-записи с внутренним кодом записи `kind_code`.
    /// Имя объекта = идентичность записи. Перечисление/инфлейт ведёт коннектор cf
    /// (testkit делегирует ему — brace ≠ файловая раскладка XML).
    ///
    /// ⚠️ Код записи — per-record-TYPE, НЕ per-kind-уникален: один код может делиться
    /// несколькими видами (напр. код `"4"` делят CommonTemplate, CommonPicture и др.).
    /// Поэтому дискриминатор вида — ПОЛНАЯ envelope-сигнатура `(код, арность тела,
    /// арность под-записи идентичности)`: записи того же кода, но иной арности —
    /// принадлежат ДРУГИМ видам и КОРРЕКТНО отсеиваются коллектором (это не §1.0-сокрытие
    /// недоделанного ЭТОГО вида — мы лишь не присваиваем себе чужие записи).
    Container {
        /// Внутренний код brace-записи вида (`"12"` для CommonModule, `"4"` для
        /// CommonTemplate) — НЕОБХОДИМЫЙ, но не достаточный дискриминатор (см. выше).
        kind_code: &'static str,
        /// Полная арность тела `{code, ident, S0..Sn}` (= `slot_count + 2`). Различает
        /// виды-сокодеры (код `"4"`: CommonTemplate body=3 / CommonPicture body=4 / …).
        body_arity: usize,
        /// Арность под-записи идентичности `{3, …}` (= `ident_len`). Часть сигнатуры.
        ident_arity: usize,
    },
    /// Form body (под-IR L1f): тело формы — ОТДЕЛЬНЫЙ файл per-form, НЕ per-object и НЕ
    /// запись контейнера. Имя формы = имя её каталога `<FormName>`; путь к телу внутри
    /// каталога ФИКСИРОВАН (а не `<FormName>.<ext>`):
    /// * EDT: `<corpus>/<FormName>/Form.form` (`inner = "Form.form"`);
    /// * Designer: `<corpus>/<FormName>/Ext/Form.xml` (`inner = "Ext/Form.xml"`).
    ///
    /// `corpus_subpath` указывает на КАТАЛОГ форм (`CommonForms` либо `<Obj>/Forms`).
    /// Перечисление: подкаталоги `corpus_subpath`, чьё `<FormName>/<inner>` существует.
    FormBody {
        /// Относительный путь тела формы ВНУТРИ каталога формы (`"Form.form"` EDT /
        /// `"Ext/Form.xml"` Designer). Прямые слэши; нормализуются под ОС перечислителем.
        inner: &'static str,
    },
    /// Дескриптор `.mdo`/`.xml` + БИНАРНЫЙ файл-СПУТНИК (Blob, §1.0 — переносится as-is,
    /// без интерпретации внутренностей). Аналог модуль-сайдкара `Module.bsl`/`Schedule.*`,
    /// но спутник — БИНАРЬ (`Picture.png`/`Picture.svg`/`Picture.zip`/… — расширение
    /// ВАРЬИРУЕТ, поэтому спутник матчится по СТЕМУ `Picture.*`, единственный не-дескриптор).
    /// Fixture-доказано (`CommonPictures/<Name>/Picture.<ext>` — реальный PNG/SVG/ZIP).
    ///
    /// Спутник ПУТЕШЕСТВУЕТ с объектом (round-trip byte-exact И дескриптора, И бинаря):
    /// * EDT (`descriptor_dir_per_object=true`): дескриптор `<Name>/<Name>.<descriptor_ext>`,
    ///   спутник — СИБЛИНГ `<Name>/<sidecar_stem>.*` (тот же каталог объекта; `sidecar_subdir=""`).
    /// * Designer (`descriptor_dir_per_object=false`): дескриптор `<Name>.<descriptor_ext>`,
    ///   спутник — `<Name>/<sidecar_subdir>/<sidecar_stem>.*` (`sidecar_subdir="Ext/Picture"`).
    ///
    /// Имя объекта извлекается как у соответствующего дескриптор-режима (имя каталога /
    /// stem файла). `collect_objects` возвращает путь ДЕСКРИПТОРА (спутник резолвится из
    /// него + этих полей).
    BinarySidecar {
        /// Расширение файла дескриптора (`"mdo"` EDT / `"xml"` Designer).
        descriptor_ext: &'static str,
        /// `true` → дескриптор `<Name>/<Name>.<ext>` (EDT DirPerObject); `false` →
        /// `<Name>.<ext>` (Designer FilePerObject). Определяет перечисление объектов.
        descriptor_dir_per_object: bool,
        /// Подкаталог бинарного спутника ОТНОСИТЕЛЬНО каталога объекта (`""` EDT-сиблинг /
        /// `"Ext/Picture"` Designer). Прямые слэши; нормализуются под ОС.
        sidecar_subdir: &'static str,
        /// Стем бинарного файла-спутника (`"Picture"`) — расширение варьирует, поэтому
        /// матчим `<sidecar_stem>.*` (единственный файл этого стема в целевом каталоге).
        sidecar_stem: &'static str,
        /// ОПЦ. подкаталог ВСПОМОГАТЕЛЬНОГО XML-дескриптора картинки ОТНОСИТЕЛЬНО каталога
        /// объекта (`""` = нет такого файла — EDT; `"Ext"` Designer → `<Name>/Ext/Picture.xml`).
        /// Этот `ExtPicture`-файл КОНСТАНТЕН по шаблону (варьирует лишь имя бинаря), поэтому
        /// round-trip-ится byte-exact реконструкцией из имени спутника
        /// (`formats_xml::picture_sidecar::ext_picture_xml_bytes`).
        sidecar_xml_subdir: &'static str,
    },
    /// Дескриптор `.mdo`/`.xml` + ТЕКСТОВЫЙ XML-файл-СПУТНИК, который переносит СТРУКТУРНЫЕ
    /// данные объекта (не opaque-Blob) и round-trip-ится **byte-exact ОБА** — и дескриптор,
    /// и спутник (в отличие от [`CorpusLayout::BinarySidecar`], где спутник — Blob и лишь
    /// read-проверяется). Срез S2: таблица прав Role живёт в спутнике `Rights.rights`
    /// (EDT) / `Ext/Rights.xml` (Designer) — отдельный XML (ns `http://v8.1c.ru/8.2/roles`,
    /// root `<Rights>`), который парсится rights-table-кодеком (`formats_xml::rights`) в
    /// per-формат IR и регенерируется байт-в-байт (§1.0-total: любой неизвестный
    /// элемент/флаг/право → типизированная ошибка).
    ///
    /// Спутник ПУТЕШЕСТВУЕТ с объектом (round-trip byte-exact И дескриптора, И спутника):
    /// * EDT (`descriptor_dir_per_object=true`): дескриптор `<Name>/<Name>.<descriptor_ext>`,
    ///   спутник — СИБЛИНГ `<Name>/<sidecar_inner>` (напр. `sidecar_inner="Rights.rights"`).
    /// * Designer (`descriptor_dir_per_object=false`): дескриптор `<Name>.<descriptor_ext>`,
    ///   спутник — `<Name>/<sidecar_inner>` (напр. `sidecar_inner="Ext/Rights.xml"`).
    ///
    /// Имя объекта извлекается как у соответствующего дескриптор-режима (имя каталога /
    /// stem файла). `collect_objects` возвращает путь ДЕСКРИПТОРА (спутник резолвится из
    /// него + `sidecar_inner`). `sidecar_format` даёт кодеку формат-специфичный envelope
    /// (BOM/version/трейлер различаются: EDT no-BOM+trailing-CRLF, Designer BOM+version).
    TextSidecar {
        /// Расширение файла дескриптора (`"mdo"` EDT / `"xml"` Designer).
        descriptor_ext: &'static str,
        /// `true` → дескриптор `<Name>/<Name>.<ext>` (EDT DirPerObject); `false` →
        /// `<Name>.<ext>` (Designer FilePerObject). Определяет перечисление объектов.
        descriptor_dir_per_object: bool,
        /// Путь ТЕКСТОВОГО спутника ОТНОСИТЕЛЬНО каталога объекта, прямыми слэшами
        /// (`"Rights.rights"` EDT / `"Ext/Rights.xml"` Designer). Нормализуется под ОС.
        /// Фиксирован (в отличие от `<stem>.*` у бинарного спутника — тип известен).
        sidecar_inner: &'static str,
        /// Какой формат-envelope у спутника ([`SidecarFormat`]): даёт кодеку BOM/version/
        /// трейлер-различия (EDT vs Designer), чтобы каждая сторона писала СВОЙ байт-точный
        /// envelope, а тело (таблица прав) было общим → cross-format X по канон-IR таблицы.
        sidecar_format: SidecarFormat,
    },
}

/// Формат-envelope текстового XML-спутника ([`CorpusLayout::TextSidecar`]) — определяет
/// байтовую обёртку спутника, которую соответствующий кодек СВЕРЯЕТ и регенерирует
/// byte-exact. Тело спутника ОБЩЕЕ у форматов (сверено корпусом), различается ЛИШЬ envelope
/// (BOM / порядок ns / `version` / финальный перевод строки). Каждый вариант несёт И вид
/// спутника (какой кодек), И формат-сторону (EDT/Designer) — testkit роутит по нему.
///
/// Семейства (по кодеку):
/// * `*Rights` — таблица прав Role (`formats_xml::rights`, срез S2);
/// * `*Xdto` — рекурсивная XDTO-схема XDTOPackage (`formats_xml::xdto`, срез S4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidecarFormat {
    /// EDT `Rights.rights`: без BOM, `<Rights xmlns:xsi=… xmlns=… xsi:type="Rights">`
    /// (без `version`), ФИНАЛЬНЫЙ CRLF после `</Rights>`.
    EdtRights,
    /// Designer `Ext/Rights.xml`: с BOM, `<Rights xmlns=… xmlns:xs=… xmlns:xsi=…
    /// xsi:type="Rights" version="…">` (значение `version=` ВЕРСИОННО: 2.20 ERP / 2.21 SSL,
    /// прочее конверта идентично — см. `formats_xml::rights`), БЕЗ финального перевода строки.
    DesignerRights,
    /// EDT `Package.xdto`: без BOM, корень `<package …>` (нет `<?xml?>`-декларации),
    /// без финального перевода строки. Тело — рекурсивная XDTO-схема (сверено SSL 54/54).
    EdtXdto,
    /// Designer `Ext/Package.bin`: С BOM, далее ТО ЖЕ тело `<package …>` что EDT (тело
    /// байт-идентично; отличается лишь BOM — сверено SSL 54/54).
    DesignerXdto,
}

impl CorpusLayout {
    /// Дескриптор-режим перечисления объектов для sidecar-раскладок
    /// ([`CorpusLayout::BinarySidecar`]/[`CorpusLayout::TextSidecar`]) — сводит их к базовому
    /// `DirPerObject`/`FilePerObject` для `collect_objects`. Для НЕ-sidecar раскладок —
    /// `None` (перечисляются своим кодом).
    pub fn descriptor_enum_mode(self) -> Option<CorpusLayout> {
        match self {
            CorpusLayout::BinarySidecar {
                descriptor_ext,
                descriptor_dir_per_object: true,
                ..
            }
            | CorpusLayout::TextSidecar {
                descriptor_ext,
                descriptor_dir_per_object: true,
                ..
            } => Some(CorpusLayout::DirPerObject {
                ext: descriptor_ext,
            }),
            CorpusLayout::BinarySidecar {
                descriptor_ext,
                descriptor_dir_per_object: false,
                ..
            }
            | CorpusLayout::TextSidecar {
                descriptor_ext,
                descriptor_dir_per_object: false,
                ..
            } => Some(CorpusLayout::FilePerObject {
                ext: descriptor_ext,
            }),
            _ => None,
        }
    }

    /// Путь текстового спутника из пути ДЕСКРИПТОРА для [`CorpusLayout::TextSidecar`]
    /// (детерминированный, в отличие от `<stem>.*`-скана бинарного спутника — тип известен):
    /// * EDT (dir-per-object): `<obj_dir>/<sidecar_inner>` (сиблинг `.mdo`);
    /// * Designer (file-per-object): `<dir>/<Name>/<sidecar_inner>` (`Name`=stem дескриптора).
    ///
    /// `None`, если раскладка не [`CorpusLayout::TextSidecar`].
    pub fn text_sidecar_path(
        self,
        descriptor_path: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        let (dir_per_object, inner) = match self {
            CorpusLayout::TextSidecar {
                descriptor_dir_per_object,
                sidecar_inner,
                ..
            } => (descriptor_dir_per_object, sidecar_inner),
            _ => return None,
        };
        let obj_dir = if dir_per_object {
            descriptor_path.parent()?.to_path_buf()
        } else {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            dir.join(stem)
        };
        let mut p = obj_dir;
        for seg in inner.split('/') {
            p = p.join(seg);
        }
        Some(p)
    }
}

/// Type-erased ридер: байты дескриптора → канонический IR (ошибка строкой).
pub type ReadFn = fn(&[u8]) -> Result<MetadataObject, String>;
/// Type-erased райтер: канонический IR → байты дескриптора (byte-exact), ошибка строкой.
pub type WriteFn = fn(&MetadataObject) -> Result<Vec<u8>, String>;

/// Одна строка реестра харнесса: всё, что нужно R+X для `(формат × вид)`.
///
/// КОНСТАНТНА (fn-указатели + `&'static str`), поэтому годна для ссылаемого `static`
/// массива, который генерирует `build.rs` формата.
#[derive(Debug, Clone, Copy)]
pub struct FormatKind {
    /// Формат-источник.
    pub format: Format,
    /// Канонический код вида (`"CommonModule"`, …) — ТОТ ЖЕ, что в `core`-реестре.
    pub kind: &'static str,
    /// Прочитать дескриптор этого вида/формата в IR.
    pub read: ReadFn,
    /// Записать IR этого вида/формата в байты (byte-exact).
    pub write: WriteFn,
    /// Относительный (от `.fixtures`) путь к корпусу этого вида/формата, напр.
    /// `"SSL/edt/src/CommonModules"`. Корпус gitignore'нут → отсутствие = инфра-skip.
    pub corpus_subpath: &'static str,
    /// Как перечислять файлы корпуса и извлекать имя объекта.
    pub layout: CorpusLayout,
}
