//! Чтение тел модулей (`<Слот>.bsl`) как сайдкаров дескриптора (§1.2/§4).
//!
//! Пер-видовые дескриптор-коннекторы (`formats/{edt,designer}`) читают ТОЛЬКО `.mdo`/`.xml`
//! метаданные объекта; исходный ТЕКСТ модулей лежит в отдельных файлах рядом. Для видов с
//! модуль-телами (см. [`module_slots`]) whole-config read подгружает эти файлы в
//! `MetadataObject.modules`, чтобы write-сторона могла их пере-эмитить (а `--to cf` для
//! CommonModule — эмитить тело `<uuid>.0`). §1.0-честно: файла нет → модуля в IR нет
//! (cf-сборщик тогда ГРОМКО ошибётся `BodyNotEmitted`, не собирая НЕзагружаемый `.cf`;
//! для объектных модулей cf-сборка пока честно отказывает тем же `BodyNotEmitted`).
//!
//! # Раскладка модуля рядом с дескриптором (RE: SSL/min_module + s15_subordinate-корпус)
//! * **EDT** (`DirPerObject`): дескриптор `<KindDir>/<Name>/<Name>.mdo`, модули —
//!   СИБЛИНГИ `<KindDir>/<Name>/<Слот>.bsl` (в том же каталоге объекта).
//! * **Designer** (`FilePerObject`): дескриптор `<KindDir>/<Name>.xml`, модули —
//!   `<KindDir>/<Name>/Ext/<Слот>.bsl` (под-каталог `<Name>/Ext`).
//!
//! Имя файла всегда `<Слот>.bsl`: `Module.bsl` (CommonModule), `ObjectModule.bsl` /
//! `ManagerModule.bsl` / `RecordSetModule.bsl` / `ValueManagerModule.bsl` (объектные виды).
//! CommandModule (команды) и форменный `Module.bsl` — ДРУГИЕ сайдкары (child-объект команды и
//! form-body соответственно), этим модулем НЕ обрабатываются.

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::{MetadataObject, Module, ModuleBody};

use crate::ConvertError;

/// Сигнатура 32-битного v8-контейнера, которым НАЧИНАЕТСЯ защищённый (protected) образ модуля
/// (`ff ff ff 7f` — FileHeader end-marker в little-endian). Witnessed 10/10 на защищённых
/// модулях `.fixtures/ERP` (5 CommonModule + object-модули DataProcessor'ов): каждый несёт эту
/// сигнатуру И не является валидным UTF-8; ни один ТЕКСТОВЫЙ `.bsl`/`.bin` её не несёт (текст
/// начинается с BOM `ef bb bf`, символа или CRLF). Позитивный, а не «текст не разобрался».
const PROTECTED_SIGNATURE: &[u8] = &[0xff, 0xff, 0xff, 0x7f];

/// Прочитать файл модуля как ТЕКСТ или ЗАЩИЩЁННЫЙ бинарь (единый проход по байтам).
///
/// Начинается с [`PROTECTED_SIGNATURE`] → защищённый образ → [`ModuleBody::Binary`] ДОСЛОВНО
/// (§1.0: не парсим, не нормализуем). Иначе — обычный UTF-8 текст → [`ModuleBody::Text`].
/// Файл, который НЕ несёт сигнатуру И не является валидным UTF-8, — не наша форма →
/// типизированная ошибка (а не паника `read_to_string` «stream did not contain valid UTF-8»).
fn read_module_body(path: &Path) -> Result<ModuleBody, ConvertError> {
    let bytes = std::fs::read(path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    if bytes.starts_with(PROTECTED_SIGNATURE) {
        return Ok(ModuleBody::Binary(bytes));
    }
    match String::from_utf8(bytes) {
        Ok(s) => Ok(ModuleBody::Text(s)),
        Err(e) => Err(ConvertError::Io {
            path: path.display().to_string(),
            reason: format!(
                "module sidecar is neither UTF-8 text nor a protected v8 image (no \
                 `ff ff ff 7f` signature): {e}"
            ),
        }),
    }
}

/// Упорядоченные слоты модуль-сайдкаров вида (метамодель, witnessed на
/// `.fixtures/coverage/{designer,edt}/s15_subordinate` + `SSL/edt/src`). Пустой срез — вид
/// модулей-сайдкаров не несёт. Расширять ТОЛЬКО по witnessed-раскладке:
/// Bot/WebSocketClient формально несут слот `Module`, но witnessed-фикстуры модулей не
/// содержат — их подключение отложено до появления витнесса (§1.0: не фабрикуем раскладку;
/// IntegrationService ПОДКЛЮЧЁН — ERP-витнесс `IntegrationServices/ОбменСообщениями`).
/// CommonCommand несёт `CommandModule.bsl` СИБЛИНГОМ дескриптора (та же раскладка,
/// что `Module.bsl`, — witnessed SSL 63/63; НЕ путь `Commands/<Cmd>/…` дочерних команд —
/// их обрабатывает [`crate::command_module_read`]).
fn module_slots(kind: &str) -> &'static [&'static str] {
    match kind {
        // Единственный слот CommonModule.
        "CommonModule" => &["Module"],
        // Топ-левел команда: единственный слот CommandModule (witnessed SSL 63/63,
        // `CommonCommands/<Name>/CommandModule.bsl`).
        "CommonCommand" => &["CommandModule"],
        // Сервисы: единственный слот Module (witnessed SSL 13/13 WebService + 2/2 HTTPService,
        // `<Kind>s/<Name>/Module.bsl`; IntegrationService — witnessed ERP 1/1
        // `IntegrationServices/ОбменСообщениями/{Module.bsl | Ext/Module.bsl}`, cf-тело
        // `<uuid>.0` = тот же module-контейнер `{info,text}`, RE erp.cf `c512a1cd-….0`).
        "WebService" | "HTTPService" | "IntegrationService" => &["Module"],
        // Ссылочно-объектные виды: объектный + менеджерный модуль.
        "Catalog"
        | "Document"
        | "DataProcessor"
        | "Report"
        | "ChartOfCharacteristicTypes"
        | "ChartOfAccounts"
        | "ChartOfCalculationTypes"
        | "BusinessProcess"
        | "Task"
        | "ExchangePlan" => &["ObjectModule", "ManagerModule"],
        // Регистры: модуль набора записей + менеджерный.
        "InformationRegister"
        | "AccumulationRegister"
        | "AccountingRegister"
        | "CalculationRegister" => &["RecordSetModule", "ManagerModule"],
        // Константа: менеджер значения + менеджерный.
        "Constant" => &["ValueManagerModule", "ManagerModule"],
        // Менеджерный-только (DocumentJournal формально без ObjectModule).
        "Enum" | "DocumentJournal" | "FilterCriterion" | "SettingsStorage" => &["ManagerModule"],
        _ => &[],
    }
}

/// Подгрузить тела модулей (если вид их несёт и файлы существуют) в `obj.modules`.
///
/// `descriptor_path` — путь к прочитанному дескриптору (`.mdo`/`.xml`). Каждый слот вида
/// ([`module_slots`]) ищется рядом по раскладке формата. Файл отсутствует → слот пропущен
/// (модуля в IR нет; §1.0-ответственность за «нужно ли тело» — у сборщика/писателя). Ошибка
/// чтения существующего файла → [`ConvertError::Io`]. Порядок `obj.modules` детерминирован
/// порядком слотов в [`module_slots`].
///
/// ЗАЩИЩЁННЫЙ (protected) модуль: EDT кладёт бинарный образ в тот же `<Слот>.bsl` (детект по
/// сигнатуре — [`read_module_body`]); Designer — в `<Name>/Ext/<Слот>.bin` (`.bsl` там НЕТ), и
/// БЕЗ `.bin`-фолбэка такой модуль ТИХО пропадал бы из IR (баг, который этот проход чинит).
pub fn attach_module_body(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    for &slot in module_slots(kind) {
        // Обычный (текстовый) сайдкар — `<Слот>.bsl` в обоих диалектах.
        let bsl = match module_path(format, descriptor_path, slot, "bsl") {
            Some(p) => p,
            None => return Ok(()), // cf: контейнер — файловых сайдкаров нет.
        };
        // Защищённый модуль в Designer лежит как `<Name>/Ext/<Слот>.bin` (расширение .bin,
        // `.bsl` там НЕТ). В EDT защищённый образ живёт в том же `<Слот>.bsl`, что и текст
        // (различаются содержимым — детектит [`read_module_body`] по сигнатуре). Поэтому
        // `.bin`-фолбэк нужен ТОЛЬКО Designer'у и ТОЛЬКО когда `.bsl` отсутствует.
        let module_path = if bsl.is_file() {
            bsl
        } else if format == Format::Designer {
            match module_path(format, descriptor_path, slot, "bin") {
                Some(bin) if bin.is_file() => bin,
                _ => continue, // ни .bsl, ни .bin → нет модуля в IR (честно).
            }
        } else {
            continue; // EDT: нет `.bsl` → нет модуля в IR (честно).
        };
        let body = read_module_body(&module_path)?;
        obj.modules.push(Module {
            slot: slot.to_string(),
            body,
        });
    }
    Ok(())
}

/// UTF-8 BOM (as a `char`). Designer text sidecars carry it (`Ext/*.bsl` starts `EF BB BF`),
/// EDT ones do not (RE: witnessed on s6/s8 `Module.bsl` and s15 object-module sidecars,
/// edt no-BOM vs designer +BOM, CRLF both). The IR body is canonicalised (BOM-stripped) so the
/// write re-adds it per format.
const BOM: char = '\u{FEFF}';

/// Write-side mirror of [`attach_module_body`]: emit each module body `obj` carries beside its
/// just-written descriptor `descriptor_out`, in the target format's layout. TEXT bodies →
/// `<Slot>.bsl` (canonicalised: strip a leading BOM, re-emit per format — Designer prepends the
/// BOM, EDT bare). PROTECTED (binary) bodies → the opaque image VERBATIM: EDT `<Slot>.bsl`,
/// Designer `<Name>/Ext/<Slot>.bin` (never `.bsl`). Kinds without module slots and objects with
/// no module are no-ops; cf (container) never reaches here (`convert.rs` routes it to the assembler, which
/// refuses unemitted module bodies loudly). §1.0: a module in a slot the kind does not own is a
/// typed [`ConvertError::Write`], never a silent drop.
pub fn write_module_bodies(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if obj.modules.is_empty() {
        return Ok(());
    }
    let slots = module_slots(kind);
    for module in &obj.modules {
        if !slots.contains(&module.slot.as_str()) {
            return Err(ConvertError::Write {
                kind: kind.to_string(),
                object: obj.name.clone(),
                reason: format!(
                    "object carries module slot {:?} which is not a witnessed sidecar slot of \
                     this kind (known slots: {slots:?})",
                    module.slot
                ),
            });
        }
        let ext = module_sidecar_ext(format, &module.body);
        let path = match module_path(format, descriptor_out, &module.slot, ext) {
            Some(p) => p,
            None => return Ok(()), // cf: container — no file-per-object module sidecar.
        };
        crate::form_write::write_file(&path, &reencode_module_body(&module.body, format))?;
    }
    Ok(())
}

/// Canonicalise a module source (strip a leading BOM) and re-encode it for `format`'s on-disk
/// text-sidecar convention: Designer modules carry a UTF-8 BOM, EDT ones do not (cf has no
/// file-per-object module sidecar → bare). Shared by the metadata-object modules
/// ([`write_module_bodies`]) and the FORM module (`crate::form_write::write_form_bodies`) so both
/// re-emit the per-format BOM convention identically.
pub(crate) fn reencode_module_for_format(source: &str, format: Format) -> Vec<u8> {
    let canonical = source.strip_prefix(BOM).unwrap_or(source);
    match format {
        Format::Designer => {
            let mut b = String::with_capacity(canonical.len() + 3);
            b.push(BOM);
            b.push_str(canonical);
            b.into_bytes()
        }
        Format::Edt | Format::Cf => canonical.as_bytes().to_vec(),
    }
}

/// Re-encode a module BODY (text or protected binary) for `format`'s on-disk sidecar.
///
/// * `Text` → per-format BOM convention ([`reencode_module_for_format`]: Designer +BOM, EDT bare).
/// * `Binary` (protected image) → the raw bytes VERBATIM. §1.0: the image is opaque and
///   dialect-independent (edt `.bsl` == designer `.bin`, md5-identical) — no BOM, no EOL touch.
pub(crate) fn reencode_module_body(body: &ModuleBody, format: Format) -> Vec<u8> {
    match body {
        ModuleBody::Text(source) => reencode_module_for_format(source, format),
        ModuleBody::Binary(bytes) => bytes.clone(),
    }
}

/// On-disk sidecar file extension for a module body in the target format.
///
/// EDT: always `bsl` — text and the protected image share the filename, differing only in
/// content. Designer: text → `bsl` (`Ext/<Slot>.bsl`), protected → `bin` (`Ext/<Slot>.bin`,
/// witnessed layout — Designer carries protected modules under `.bin`, never `.bsl`).
fn module_sidecar_ext(format: Format, body: &ModuleBody) -> &'static str {
    match (format, body) {
        (Format::Designer, ModuleBody::Binary(_)) => "bin",
        _ => "bsl",
    }
}

/// Путь к `<slot>.<ext>` рядом с дескриптором по раскладке формата (см. модульный docstring).
/// `ext` — `"bsl"` (обычный текстовый сайдкар, оба диалекта) или `"bin"` (Designer-раскладка
/// защищённого образа). `None` для форматов без файловой раскладки модуля (cf — контейнер).
fn module_path(format: Format, descriptor_path: &Path, slot: &str, ext: &str) -> Option<PathBuf> {
    let file_name = format!("{slot}.{ext}");
    match format {
        // EDT: дескриптор `<obj-dir>/<Name>.mdo` → модуль `<obj-dir>/<Слот>.<ext>`.
        Format::Edt => {
            let obj_dir = descriptor_path.parent()?;
            Some(obj_dir.join(file_name))
        }
        // Designer: дескриптор `<dir>/<Name>.xml` → модуль `<dir>/<Name>/Ext/<Слот>.<ext>`.
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            Some(dir.join(stem).join("Ext").join(file_name))
        }
        Format::Cf => None,
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::{ObjectKind, Uuid};

    fn obj(kind: &str) -> MetadataObject {
        MetadataObject::new(ObjectKind::new(kind), "X", Uuid([0; 16]))
    }

    #[test]
    fn edt_module_path_is_sibling() {
        let p = Path::new("/root/CommonModules/Foo/Foo.mdo");
        let m = module_path(Format::Edt, p, "Module", "bsl").unwrap();
        assert!(
            m.ends_with(Path::new("CommonModules/Foo/Module.bsl")),
            "got {m:?}"
        );
        let p = Path::new("/root/Catalogs/Спр/Спр.mdo");
        let m = module_path(Format::Edt, p, "ObjectModule", "bsl").unwrap();
        assert!(
            m.ends_with(Path::new("Catalogs/Спр/ObjectModule.bsl")),
            "got {m:?}"
        );
    }

    #[test]
    fn designer_module_path_is_ext_subdir() {
        let p = Path::new("/root/CommonModules/Foo.xml");
        let m = module_path(Format::Designer, p, "Module", "bsl").unwrap();
        assert!(
            m.ends_with(Path::new("CommonModules/Foo/Ext/Module.bsl")),
            "got {m:?}"
        );
        let p = Path::new("/root/InformationRegisters/Рег.xml");
        let m = module_path(Format::Designer, p, "RecordSetModule", "bsl").unwrap();
        assert!(
            m.ends_with(Path::new(
                "InformationRegisters/Рег/Ext/RecordSetModule.bsl"
            )),
            "got {m:?}"
        );
    }

    /// Метамодельная таблица слотов: пер-вид точный упорядоченный список.
    #[test]
    fn slot_table_matches_metamodel() {
        assert_eq!(module_slots("CommonModule"), ["Module"]);
        assert_eq!(module_slots("CommonCommand"), ["CommandModule"]);
        assert_eq!(module_slots("WebService"), ["Module"]);
        assert_eq!(module_slots("HTTPService"), ["Module"]);
        assert_eq!(module_slots("IntegrationService"), ["Module"]);
        for k in [
            "Catalog",
            "Document",
            "DataProcessor",
            "Report",
            "ChartOfCharacteristicTypes",
            "ChartOfAccounts",
            "ChartOfCalculationTypes",
            "BusinessProcess",
            "Task",
            "ExchangePlan",
        ] {
            assert_eq!(module_slots(k), ["ObjectModule", "ManagerModule"], "{k}");
        }
        for k in [
            "InformationRegister",
            "AccumulationRegister",
            "AccountingRegister",
            "CalculationRegister",
        ] {
            assert_eq!(module_slots(k), ["RecordSetModule", "ManagerModule"], "{k}");
        }
        assert_eq!(
            module_slots("Constant"),
            ["ValueManagerModule", "ManagerModule"]
        );
        for k in [
            "Enum",
            "DocumentJournal",
            "FilterCriterion",
            "SettingsStorage",
        ] {
            assert_eq!(module_slots(k), ["ManagerModule"], "{k}");
        }
        // Виды без witnessed-модулей — пусто (в т.ч. сервисы без витнесса и чужие сайдкары).
        // (IntegrationService ВЫВЕДЕН из этого списка: его слот Module witnessed на ERP —
        // `IntegrationServices/ОбменСообщениями` + cf-тело `c512a1cd-….0`.)
        for k in ["Role", "Subsystem", "Bot", "WebSocketClient", "CommonForm"] {
            assert!(module_slots(k).is_empty(), "{k}");
        }
    }

    #[test]
    fn non_module_kind_is_noop() {
        let mut o = obj("Role");
        // A path that does not exist — must still be a no-op (kind has no module slots).
        attach_module_body(Format::Edt, "Role", Path::new("/nope/C/C.mdo"), &mut o).unwrap();
        assert!(o.modules.is_empty());
    }

    #[test]
    fn missing_module_files_is_noop() {
        let mut o = obj("Constant");
        // Module kind, but no files on disk → honestly no modules in IR.
        attach_module_body(Format::Edt, "Constant", Path::new("/nope/C/C.mdo"), &mut o).unwrap();
        assert!(o.modules.is_empty());
    }

    /// attach по обоим диалектам + write byte-exact (пер-слот), на temp-дереве.
    #[test]
    fn attach_and_write_roundtrip_both_dialects() {
        let base = std::env::temp_dir().join(format!("morph1c-module-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);

        // Витнесс-содержимое: designer несёт BOM, edt — нет; CRLF оба (s15-конвенция).
        let object_src = "// Модуль объекта\r\nПроцедура О() КонецПроцедуры\r\n";
        let manager_src = "// Модуль менеджера\r\nПроцедура М() КонецПроцедуры\r\n";

        // EDT: <obj-dir>/<Слот>.bsl рядом с .mdo.
        let edt_dir = base.join("edt/Catalogs/Спр");
        std::fs::create_dir_all(&edt_dir).unwrap();
        let edt_mdo = edt_dir.join("Спр.mdo");
        crate::fsio::write(&edt_mdo, b"<mdo/>").unwrap();
        crate::fsio::write(edt_dir.join("ObjectModule.bsl"), object_src.as_bytes()).unwrap();
        crate::fsio::write(edt_dir.join("ManagerModule.bsl"), manager_src.as_bytes()).unwrap();

        // Designer: <dir>/<Name>/Ext/<Слот>.bsl рядом с <dir>/<Name>.xml, +BOM.
        let dsn_dir = base.join("designer/Catalogs");
        let dsn_ext = dsn_dir.join("Спр/Ext");
        std::fs::create_dir_all(&dsn_ext).unwrap();
        let dsn_xml = dsn_dir.join("Спр.xml");
        crate::fsio::write(&dsn_xml, b"<xml/>").unwrap();
        crate::fsio::write(
            dsn_ext.join("ObjectModule.bsl"),
            format!("{BOM}{object_src}"),
        )
        .unwrap();
        crate::fsio::write(
            dsn_ext.join("ManagerModule.bsl"),
            format!("{BOM}{manager_src}"),
        )
        .unwrap();

        // attach: оба диалекта дают те же слоты в порядке module_slots.
        let mut edt_obj = obj("Catalog");
        attach_module_body(Format::Edt, "Catalog", &edt_mdo, &mut edt_obj).unwrap();
        assert_eq!(
            edt_obj
                .modules
                .iter()
                .map(|m| m.slot.as_str())
                .collect::<Vec<_>>(),
            ["ObjectModule", "ManagerModule"]
        );
        assert_eq!(edt_obj.modules[0].as_text().unwrap(), object_src);

        let mut dsn_obj = obj("Catalog");
        attach_module_body(Format::Designer, "Catalog", &dsn_xml, &mut dsn_obj).unwrap();
        assert_eq!(
            dsn_obj
                .modules
                .iter()
                .map(|m| m.slot.as_str())
                .collect::<Vec<_>>(),
            ["ObjectModule", "ManagerModule"]
        );

        // write byte-exact в СВОЙ диалект (пер-слот)…
        let edt_out = base.join("out-edt/Catalogs/Спр");
        std::fs::create_dir_all(&edt_out).unwrap();
        let edt_out_mdo = edt_out.join("Спр.mdo");
        crate::fsio::write(&edt_out_mdo, b"<mdo/>").unwrap();
        write_module_bodies(Format::Edt, "Catalog", &edt_out_mdo, &edt_obj).unwrap();
        for slot in ["ObjectModule", "ManagerModule"] {
            let orig = std::fs::read(edt_dir.join(format!("{slot}.bsl"))).unwrap();
            let out = std::fs::read(edt_out.join(format!("{slot}.bsl"))).unwrap();
            assert_eq!(orig, out, "edt→edt {slot} not byte-exact");
        }
        let dsn_out = base.join("out-dsn/Catalogs");
        std::fs::create_dir_all(&dsn_out).unwrap();
        let dsn_out_xml = dsn_out.join("Спр.xml");
        crate::fsio::write(&dsn_out_xml, b"<xml/>").unwrap();
        write_module_bodies(Format::Designer, "Catalog", &dsn_out_xml, &dsn_obj).unwrap();
        for slot in ["ObjectModule", "ManagerModule"] {
            let orig = std::fs::read(dsn_ext.join(format!("{slot}.bsl"))).unwrap();
            let out = std::fs::read(dsn_out.join(format!("Спр/Ext/{slot}.bsl"))).unwrap();
            assert_eq!(orig, out, "designer→designer {slot} not byte-exact");
        }

        // …и КРОСС-диалектно (BOM-конвенция цели): designer-IR → edt-байты и наоборот.
        let cross_edt = base.join("cross-edt/Catalogs/Спр");
        std::fs::create_dir_all(&cross_edt).unwrap();
        let cross_edt_mdo = cross_edt.join("Спр.mdo");
        crate::fsio::write(&cross_edt_mdo, b"<mdo/>").unwrap();
        write_module_bodies(Format::Edt, "Catalog", &cross_edt_mdo, &dsn_obj).unwrap();
        assert_eq!(
            std::fs::read(cross_edt.join("ObjectModule.bsl")).unwrap(),
            object_src.as_bytes(),
            "designer-IR → edt must strip the BOM"
        );
        let cross_dsn = base.join("cross-dsn/Catalogs");
        std::fs::create_dir_all(&cross_dsn).unwrap();
        let cross_dsn_xml = cross_dsn.join("Спр.xml");
        crate::fsio::write(&cross_dsn_xml, b"<xml/>").unwrap();
        write_module_bodies(Format::Designer, "Catalog", &cross_dsn_xml, &edt_obj).unwrap();
        assert_eq!(
            std::fs::read(cross_dsn.join("Спр/Ext/ObjectModule.bsl")).unwrap(),
            format!("{BOM}{object_src}").as_bytes(),
            "edt-IR → designer must add the BOM"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Одиночные слоты: Constant (ValueManagerModule) и регистр (RecordSetModule) attach'атся,
    /// даже когда второй слот (ManagerModule) отсутствует на диске.
    #[test]
    fn partial_slot_presence_attaches_only_existing() {
        let base =
            std::env::temp_dir().join(format!("morph1c-module-partial-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dir = base.join("Constants/Конст");
        std::fs::create_dir_all(&dir).unwrap();
        let mdo = dir.join("Конст.mdo");
        crate::fsio::write(&mdo, b"<mdo/>").unwrap();
        crate::fsio::write(dir.join("ValueManagerModule.bsl"), "// vm\r\n").unwrap();

        let mut o = obj("Constant");
        attach_module_body(Format::Edt, "Constant", &mdo, &mut o).unwrap();
        assert_eq!(
            o.modules
                .iter()
                .map(|m| m.slot.as_str())
                .collect::<Vec<_>>(),
            ["ValueManagerModule"]
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// §1.0: модуль в слоте, которого вид не несёт → громкая типизированная ошибка записи.
    #[test]
    fn write_unknown_slot_is_loud_error() {
        let mut o = obj("Enum");
        o.modules.push(Module::text("ObjectModule", "// x\r\n"));
        let err =
            write_module_bodies(Format::Edt, "Enum", Path::new("/nope/E/E.mdo"), &o).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ObjectModule"), "got: {msg}");
    }

    /// Витнесс-префикс защищённого образа 1С (32-бит v8 FileHeader `ff ff ff 7f 00 02 …`) —
    /// РЕАЛЬНЫЕ первые 16 байт всех 10 защищённых модулей ERP (сверено). Достаточно для проверки
    /// детектора и verbatim-записи (сам образ опаков; write — passthrough). Полный read→body→
    /// write на НАСТОЯЩЕМ `.bsl` из фикстур — в `protected_common_module_real_fixture_roundtrip`
    /// (gated на ERP).
    fn protected_prefix() -> Vec<u8> {
        vec![
            0xff, 0xff, 0xff, 0x7f, 0x00, 0x02, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0xde, 0xad, 0xbe, 0xef,
        ]
    }

    /// Детектор: сигнатура `ff ff ff 7f` → Binary ДОСЛОВНО; UTF-8 → Text; невалидный UTF-8 без
    /// сигнатуры → типизированная ошибка (не паника).
    #[test]
    fn read_module_body_detects_kind() {
        let base = std::env::temp_dir().join(format!("morph1c-detect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();

        // защищённый → Binary(верно, байты сохранены дословно)
        let prot = base.join("Module.bsl");
        let bytes = protected_prefix();
        crate::fsio::write(&prot, &bytes).unwrap();
        assert_eq!(read_module_body(&prot).unwrap(), ModuleBody::Binary(bytes));

        // обычный текст → Text
        let txt = base.join("Text.bsl");
        crate::fsio::write(&txt, "// Модуль\r\nПроцедура О() КонецПроцедуры\r\n".as_bytes()).unwrap();
        assert_eq!(
            read_module_body(&txt).unwrap(),
            ModuleBody::Text("// Модуль\r\nПроцедура О() КонецПроцедуры\r\n".into())
        );

        // невалидный UTF-8 БЕЗ сигнатуры → ошибка (0xff в середине, не 4-байтовая сигнатура)
        let bad = base.join("Bad.bsl");
        crate::fsio::write(&bad, &[0x41u8, 0xff, 0x00]).unwrap();
        assert!(read_module_body(&bad).is_err());

        let _ = std::fs::remove_dir_all(&base);
    }

    /// EDT: защищённый `.bsl` → Binary → edt-write ДОСЛОВНО в `<Слот>.bsl`; кросс-write в Designer
    /// кладёт `<Name>/Ext/<Слот>.bin` (НЕ `.bsl`, БЕЗ BOM) ДОСЛОВНО.
    #[test]
    fn edt_protected_attach_and_write_both_dialects_verbatim() {
        let base = std::env::temp_dir().join(format!("morph1c-prot-edt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let img = protected_prefix();

        // EDT-раскладка: <obj-dir>/<Слот>.bsl рядом с .mdo.
        let edt_dir = base.join("edt/DataProcessors/Обр");
        std::fs::create_dir_all(&edt_dir).unwrap();
        let edt_mdo = edt_dir.join("Обр.mdo");
        crate::fsio::write(&edt_mdo, b"<mdo/>").unwrap();
        crate::fsio::write(edt_dir.join("ObjectModule.bsl"), &img).unwrap();

        // attach → ровно один Binary-модуль (ManagerModule отсутствует).
        let mut o = obj("DataProcessor");
        attach_module_body(Format::Edt, "DataProcessor", &edt_mdo, &mut o).unwrap();
        assert_eq!(o.modules.len(), 1);
        assert_eq!(o.modules[0].slot, "ObjectModule");
        assert_eq!(o.modules[0].body, ModuleBody::Binary(img.clone()));
        assert!(o.modules[0].as_text().is_none());

        // edt-write: тот же `<Слот>.bsl`, байт-в-байт.
        let edt_out = base.join("out-edt/DataProcessors/Обр");
        std::fs::create_dir_all(&edt_out).unwrap();
        let edt_out_mdo = edt_out.join("Обр.mdo");
        crate::fsio::write(&edt_out_mdo, b"<mdo/>").unwrap();
        write_module_bodies(Format::Edt, "DataProcessor", &edt_out_mdo, &o).unwrap();
        assert_eq!(
            std::fs::read(edt_out.join("ObjectModule.bsl")).unwrap(),
            img,
            "edt protected → .bsl verbatim"
        );

        // designer-write (кросс-диалект): `<Name>/Ext/<Слот>.bin`, БЕЗ BOM, verbatim; `.bsl` НЕТ.
        let dsn_out = base.join("out-dsn/DataProcessors");
        std::fs::create_dir_all(&dsn_out).unwrap();
        let dsn_out_xml = dsn_out.join("Обр.xml");
        crate::fsio::write(&dsn_out_xml, b"<xml/>").unwrap();
        write_module_bodies(Format::Designer, "DataProcessor", &dsn_out_xml, &o).unwrap();
        assert_eq!(
            std::fs::read(dsn_out.join("Обр/Ext/ObjectModule.bin")).unwrap(),
            img,
            "designer protected → Ext/*.bin verbatim (no BOM)"
        );
        assert!(
            !dsn_out.join("Обр/Ext/ObjectModule.bsl").exists(),
            "protected designer module must NOT be written as .bsl"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Designer: защищённый образ лежит как `<Name>/Ext/<Слот>.bin` (а `.bsl` НЕТ) — attach
    /// должен подхватить его через `.bin`-фолбэк (иначе модуль ТИХО пропадал бы), Binary
    /// дословно; кросс-write в EDT кладёт `<Слот>.bsl` verbatim.
    #[test]
    fn designer_protected_bin_fallback_attach_and_cross_write() {
        let base = std::env::temp_dir().join(format!("morph1c-prot-dsn-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let img = protected_prefix();

        // Designer-раскладка: <dir>/<Name>.xml + <dir>/<Name>/Ext/<Слот>.bin (НЕТ .bsl).
        let dsn_dir = base.join("designer/CommonModules");
        let dsn_ext = dsn_dir.join("ЗащМод/Ext");
        std::fs::create_dir_all(&dsn_ext).unwrap();
        let dsn_xml = dsn_dir.join("ЗащМод.xml");
        crate::fsio::write(&dsn_xml, b"<xml/>").unwrap();
        crate::fsio::write(dsn_ext.join("Module.bin"), &img).unwrap();

        let mut o = obj("CommonModule");
        attach_module_body(Format::Designer, "CommonModule", &dsn_xml, &mut o).unwrap();
        assert_eq!(o.modules.len(), 1, "the .bin fallback must attach the module");
        assert_eq!(o.modules[0].slot, "Module");
        assert_eq!(o.modules[0].body, ModuleBody::Binary(img.clone()));

        // designer-write back: `<Name>/Ext/Module.bin` verbatim.
        let dsn_out = base.join("out-dsn/CommonModules");
        std::fs::create_dir_all(&dsn_out).unwrap();
        let dsn_out_xml = dsn_out.join("ЗащМод.xml");
        crate::fsio::write(&dsn_out_xml, b"<xml/>").unwrap();
        write_module_bodies(Format::Designer, "CommonModule", &dsn_out_xml, &o).unwrap();
        assert_eq!(
            std::fs::read(dsn_out.join("ЗащМод/Ext/Module.bin")).unwrap(),
            img
        );

        // cross-write to EDT: `<Слот>.bsl` verbatim.
        let edt_out = base.join("out-edt/CommonModules/ЗащМод");
        std::fs::create_dir_all(&edt_out).unwrap();
        let edt_out_mdo = edt_out.join("ЗащМод.mdo");
        crate::fsio::write(&edt_out_mdo, b"<mdo/>").unwrap();
        write_module_bodies(Format::Edt, "CommonModule", &edt_out_mdo, &o).unwrap();
        assert_eq!(std::fs::read(edt_out.join("Module.bsl")).unwrap(), img);

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Fixtures-gated (§3.2): на РЕАЛЬНОМ защищённом модуле ERP — самом маленьком (CommonModule
    /// `ЕдиныйНалоговыйСчетИнтеграцияПовтИсп`, 2827 байт) — полный цикл: (1) edt `.bsl` и
    /// designer `.bin` БАЙТ-ИДЕНТИЧНЫ (единый диалект-независимый канон); (2) каждый читается в
    /// Binary-body ДОСЛОВНО через `read_module_body`; (3) attach на edt-дескрипторе даёт один
    /// Binary-модуль; (4) запись в оба диалекта воспроизводит исходные байты (edt `<Слот>.bsl`,
    /// designer `<Name>/Ext/<Слот>.bin`). InfraSkip, если фикстуры ERP не принесены.
    #[test]
    fn protected_common_module_real_fixture_roundtrip() {
        let root = morph1c_testkit::fixtures_root();
        let edt_bsl = root
            .join("ERP/edt/src/CommonModules/ЕдиныйНалоговыйСчетИнтеграцияПовтИсп/Module.bsl");
        let dsn_bin = root.join(
            "ERP/designer_8.3.27/CommonModules/ЕдиныйНалоговыйСчетИнтеграцияПовтИсп/Ext/Module.bin",
        );
        if !edt_bsl.is_file() || !dsn_bin.is_file() {
            eprintln!("InfraSkip: ERP protected-module fixtures absent");
            return;
        }
        let edt_bytes = std::fs::read(&edt_bsl).unwrap();
        let dsn_bytes = std::fs::read(&dsn_bin).unwrap();
        assert!(
            edt_bytes.starts_with(PROTECTED_SIGNATURE),
            "the fixture .bsl must be a protected v8 image"
        );
        assert_eq!(
            edt_bytes, dsn_bytes,
            "edt .bsl and designer .bin are byte-identical (dialect-independent canon)"
        );

        // (2) read_module_body on the real .bsl → Binary verbatim.
        assert_eq!(
            read_module_body(&edt_bsl).unwrap(),
            ModuleBody::Binary(edt_bytes.clone())
        );

        // (3) attach on a temp edt tree mirroring the fixture layout.
        let base = std::env::temp_dir().join(format!("morph1c-prot-real-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let edt_dir = base.join("edt/CommonModules/ЗМ");
        std::fs::create_dir_all(&edt_dir).unwrap();
        let mdo = edt_dir.join("ЗМ.mdo");
        crate::fsio::write(&mdo, b"<mdo/>").unwrap();
        crate::fsio::write(edt_dir.join("Module.bsl"), &edt_bytes).unwrap();
        let mut o = obj("CommonModule");
        attach_module_body(Format::Edt, "CommonModule", &mdo, &mut o).unwrap();
        assert_eq!(o.modules.len(), 1);
        assert_eq!(o.modules[0].body, ModuleBody::Binary(edt_bytes.clone()));

        // (4) write both dialects → verbatim.
        let edt_out = base.join("out-edt/CommonModules/ЗМ");
        std::fs::create_dir_all(&edt_out).unwrap();
        let edt_out_mdo = edt_out.join("ЗМ.mdo");
        crate::fsio::write(&edt_out_mdo, b"<mdo/>").unwrap();
        write_module_bodies(Format::Edt, "CommonModule", &edt_out_mdo, &o).unwrap();
        assert_eq!(std::fs::read(edt_out.join("Module.bsl")).unwrap(), edt_bytes);

        let dsn_out = base.join("out-dsn/CommonModules");
        std::fs::create_dir_all(&dsn_out).unwrap();
        let dsn_out_xml = dsn_out.join("ЗМ.xml");
        crate::fsio::write(&dsn_out_xml, b"<xml/>").unwrap();
        write_module_bodies(Format::Designer, "CommonModule", &dsn_out_xml, &o).unwrap();
        assert_eq!(
            std::fs::read(dsn_out.join("ЗМ/Ext/Module.bin")).unwrap(),
            edt_bytes
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
