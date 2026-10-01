//! Чтение/запись тел МОДУЛЕЙ КОМАНД (`CommandModule.bsl`) как сайдкаров дескриптора владельца
//! (§1.2/§4). Сиблинг [`crate::module_read`] — «дескриптор-read метаданных не трогает тело;
//! whole-config-конвейер присоединяет его отдельным проходом».
//!
//! Дескриптор КОМАНДЫ — НЕ отдельный файл: команда владельца (`Catalog.Command`,
//! `Report.Command`, …) читается пер-видовым коннектором как ДИТЯ владельца (полный
//! sub-object в `obj.children`). Отдельным файлом лежит только ТЕКСТ модуля команды.
//!
//! # Раскладка модуля команды рядом с дескриптором владельца (RE: coverage/s15_subordinate,
//! 17 команд × оба формата)
//! * **EDT** (`DirPerObject`): дескриптор `<Kind>s/<Owner>/<Owner>.mdo`, модуль —
//!   `<Kind>s/<Owner>/Commands/<Cmd>/CommandModule.bsl` (БЕЗ BOM, CRLF).
//! * **Designer** (`FilePerObject`): дескриптор `<Kind>s/<Owner>.xml`, модуль —
//!   `<Kind>s/<Owner>/Commands/<Cmd>/Ext/CommandModule.bsl` (С BOM, CRLF).
//!
//! BOM-конвенция ИДЕНТИЧНА `Module.bsl` → пере-кодирование делит
//! [`crate::module_read::reencode_module_for_format`] (один код на обе стороны).
//!
//! Модуль кладётся в `modules` САМОГО ДИТЯ-КОМАНДЫ (слот [`COMMAND_MODULE_SLOT`]) — имя
//! команды несёт идентичность ребёнка, IR-расширения не требуется. §1.0: файла нет → модуля
//! в IR нет (честно; команда без модуля представима дескриптором). NB: top-level вид
//! `CommonCommand` несёт `CommandModule.bsl` СИБЛИНГОМ собственного дескриптора (witnessed
//! SSL 63/63) — его подключает [`crate::module_read`] (слот-таблица вида), не этот проход.

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::{MetadataObject, Module};

use crate::module_read::reencode_module_body;
use crate::ConvertError;

/// Канонический слот единственного модуля команды (аналог `"Module"` у CommonModule).
const COMMAND_MODULE_SLOT: &str = "CommandModule";

/// Дитя-команда? Канонический код вида команды владельца — `<OwnerKind>.Command`
/// (`Catalog.Command`, `AccountingRegister.Command`, …).
fn is_command_child(kind: &str) -> bool {
    kind.ends_with(".Command")
}

/// Подгрузить тела модулей команд (если файлы существуют) в `modules` каждого дитя-команды.
///
/// `descriptor_path` — путь к прочитанному дескриптору ВЛАДЕЛЬЦА (`.mdo`/`.xml`). Команды
/// берутся из `obj.children` (НЕ скан ФС — §1.0: дескриптор объявляет команды). Файл
/// отсутствует → no-op для этой команды (модуля в IR нет, честно). Ошибка чтения
/// существующего файла → [`ConvertError::Io`].
pub fn attach_command_modules(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(()); // контейнер — не файловый сайдкар.
    }
    let owner = obj.name.clone();
    for child in obj
        .children
        .iter_mut()
        .filter(|c| is_command_child(c.kind.as_str()))
    {
        let path = match command_module_path(format, descriptor_path, &child.name) {
            Some(p) => p,
            None => continue,
        };
        if !path.is_file() {
            continue; // нет файла модуля → нет модуля в IR (честно).
        }
        // §1.0: дескриптор-read НЕ заполняет модуль команды — только этот проход. Уже
        // занятый слот означает двойной attach / неожиданную проекцию — громко.
        if child.modules.iter().any(|m| m.slot == COMMAND_MODULE_SLOT) {
            return Err(ConvertError::Read {
                kind: kind.to_string(),
                object: owner,
                reason: format!(
                    "command {:?} already carries a {COMMAND_MODULE_SLOT:?} module before the \
                     sidecar attach (unexpected — the descriptor projection must not populate it)",
                    child.name
                ),
            });
        }
        // Command modules are text sidecars (no protected command module is witnessed;
        // a non-UTF-8 one would surface as a typed read error rather than silently pass).
        let source = crate::module_read::read_module_text(&path)?;
        child.modules.push(Module::text(COMMAND_MODULE_SLOT, source));
    }
    Ok(())
}

/// Write-side mirror of [`attach_command_modules`]: emit each command child's module body
/// beside the just-written OWNER descriptor `descriptor_out`, in the target format's layout.
/// Re-encodes per the shared BOM convention ([`reencode_module_for_format`]: Designer +BOM,
/// EDT bare). Command children with no module are honest no-ops; a command carrying modules
/// but NOT the expected slot is a typed [`ConvertError::Write`] (§1.0 — never a silent drop).
pub fn write_command_modules(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(()); // контейнер — тела собирает cf-ассемблер, не файловый сайдкар.
    }
    for child in obj
        .children
        .iter()
        .filter(|c| is_command_child(c.kind.as_str()))
    {
        if child.modules.is_empty() {
            continue; // команда без модуля — представимо (честный no-op, как на read).
        }
        let module = child
            .modules
            .iter()
            .find(|m| m.slot == COMMAND_MODULE_SLOT)
            .ok_or_else(|| ConvertError::Write {
                kind: kind.to_string(),
                object: obj.name.clone(),
                reason: format!(
                    "command {:?} carries module slots {:?} but not the expected \
                         {COMMAND_MODULE_SLOT:?}",
                    child.name,
                    child
                        .modules
                        .iter()
                        .map(|m| m.slot.as_str())
                        .collect::<Vec<_>>()
                ),
            })?;
        let path = match command_module_path(format, descriptor_out, &child.name) {
            Some(p) => p,
            None => continue,
        };
        crate::form_write::write_file(&path, &reencode_module_body(&module.body, format))?;
    }
    Ok(())
}

/// Путь к `CommandModule.bsl` команды `command` рядом с дескриптором владельца по раскладке
/// формата (см. модульный docstring). `None` для cf (контейнер — не файловая раскладка).
fn command_module_path(format: Format, descriptor_path: &Path, command: &str) -> Option<PathBuf> {
    match format {
        // EDT: дескриптор `<obj-dir>/<Owner>.mdo` → `<obj-dir>/Commands/<Cmd>/CommandModule.bsl`.
        Format::Edt => {
            let obj_dir = descriptor_path.parent()?;
            Some(
                obj_dir
                    .join("Commands")
                    .join(command)
                    .join("CommandModule.bsl"),
            )
        }
        // Designer: дескриптор `<dir>/<Owner>.xml` →
        // `<dir>/<Owner>/Commands/<Cmd>/Ext/CommandModule.bsl`.
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            Some(
                dir.join(stem)
                    .join("Commands")
                    .join(command)
                    .join("Ext")
                    .join("CommandModule.bsl"),
            )
        }
        Format::Cf => None,
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::{ObjectKind, Uuid};

    /// Владелец `Catalog` c одним дитём-командой `Кмд`.
    fn owner_with_command() -> MetadataObject {
        let mut obj = MetadataObject::new(ObjectKind::new("Catalog"), "Спр", Uuid([0; 16]));
        obj.children.push(MetadataObject::new(
            ObjectKind::new("Catalog.Command"),
            "Кмд",
            Uuid([1; 16]),
        ));
        obj
    }

    /// Свежий temp-каталог на тест (pid + счётчик времени — без коллизий между прогонами).
    fn temp_base(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "morph1c-cmdmod-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn edt_command_module_path_is_under_commands() {
        let p = Path::new("/root/Catalogs/Спр/Спр.mdo");
        let m = command_module_path(Format::Edt, p, "Кмд").unwrap();
        assert!(
            m.ends_with(Path::new("Catalogs/Спр/Commands/Кмд/CommandModule.bsl")),
            "got {m:?}"
        );
    }

    #[test]
    fn designer_command_module_path_is_under_commands_ext() {
        let p = Path::new("/root/Catalogs/Спр.xml");
        let m = command_module_path(Format::Designer, p, "Кмд").unwrap();
        assert!(
            m.ends_with(Path::new("Catalogs/Спр/Commands/Кмд/Ext/CommandModule.bsl")),
            "got {m:?}"
        );
    }

    #[test]
    fn attach_reads_edt_module_bare_and_designer_module_bom() {
        // EDT: файл без BOM → source как есть.
        let base = temp_base("attach-edt");
        let obj_dir = base.join("Catalogs").join("Спр");
        std::fs::create_dir_all(obj_dir.join("Commands").join("Кмд")).unwrap();
        crate::fsio::write(
            obj_dir
                .join("Commands")
                .join("Кмд")
                .join("CommandModule.bsl"),
            "// м\r\n".as_bytes(),
        )
        .unwrap();
        let mut obj = owner_with_command();
        attach_command_modules(Format::Edt, "Catalog", &obj_dir.join("Спр.mdo"), &mut obj).unwrap();
        assert_eq!(obj.children[0].modules.len(), 1);
        assert_eq!(obj.children[0].modules[0].slot, COMMAND_MODULE_SLOT);
        assert_eq!(obj.children[0].modules[0].as_text().unwrap(), "// м\r\n");

        // Designer: файл с BOM → source несёт BOM (канонизация — забота reencode на write).
        let dbase = temp_base("attach-designer");
        let ddir = dbase.join("Catalogs");
        std::fs::create_dir_all(ddir.join("Спр").join("Commands").join("Кмд").join("Ext")).unwrap();
        crate::fsio::write(
            ddir.join("Спр")
                .join("Commands")
                .join("Кмд")
                .join("Ext")
                .join("CommandModule.bsl"),
            "\u{FEFF}// м\r\n".as_bytes(),
        )
        .unwrap();
        let mut dobj = owner_with_command();
        attach_command_modules(
            Format::Designer,
            "Catalog",
            &ddir.join("Спр.xml"),
            &mut dobj,
        )
        .unwrap();
        assert_eq!(dobj.children[0].modules.len(), 1);
        assert_eq!(
            dobj.children[0].modules[0].as_text().unwrap(),
            "\u{FEFF}// м\r\n"
        );

        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&dbase);
    }

    #[test]
    fn missing_module_file_and_commandless_owner_are_noops() {
        // Команда объявлена, файла нет → нет модуля (честно), не ошибка.
        let base = temp_base("noop");
        let obj_dir = base.join("Catalogs").join("Спр");
        std::fs::create_dir_all(&obj_dir).unwrap();
        let mut obj = owner_with_command();
        attach_command_modules(Format::Edt, "Catalog", &obj_dir.join("Спр.mdo"), &mut obj).unwrap();
        assert!(obj.children[0].modules.is_empty());
        // Владелец без команд — тоже no-op.
        let mut plain = MetadataObject::new(ObjectKind::new("Constant"), "К", Uuid([0; 16]));
        attach_command_modules(Format::Edt, "Constant", &obj_dir.join("К.mdo"), &mut plain)
            .unwrap();
        assert!(plain.children.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn write_reencodes_per_format_bom_convention() {
        // IR с designer-прочитанным (BOM) модулем → EDT-запись БЕЗ BOM, Designer-запись С BOM.
        let mut obj = owner_with_command();
        obj.children[0]
            .modules
            .push(Module::text(COMMAND_MODULE_SLOT, "\u{FEFF}// м\r\n"));

        let ebase = temp_base("write-edt");
        let e_out = ebase.join("Catalogs").join("Спр").join("Спр.mdo");
        std::fs::create_dir_all(e_out.parent().unwrap()).unwrap();
        write_command_modules(Format::Edt, "Catalog", &e_out, &obj).unwrap();
        let e_bytes = std::fs::read(
            ebase
                .join("Catalogs")
                .join("Спр")
                .join("Commands")
                .join("Кмд")
                .join("CommandModule.bsl"),
        )
        .unwrap();
        assert_eq!(e_bytes, "// м\r\n".as_bytes(), "EDT must be bare (no BOM)");

        let dbase = temp_base("write-designer");
        let d_out = dbase.join("Catalogs").join("Спр.xml");
        std::fs::create_dir_all(d_out.parent().unwrap()).unwrap();
        write_command_modules(Format::Designer, "Catalog", &d_out, &obj).unwrap();
        let d_bytes = std::fs::read(
            dbase
                .join("Catalogs")
                .join("Спр")
                .join("Commands")
                .join("Кмд")
                .join("Ext")
                .join("CommandModule.bsl"),
        )
        .unwrap();
        assert_eq!(
            d_bytes,
            "\u{FEFF}// м\r\n".as_bytes(),
            "Designer must carry the BOM"
        );

        let _ = std::fs::remove_dir_all(&ebase);
        let _ = std::fs::remove_dir_all(&dbase);
    }

    #[test]
    fn write_rejects_unexpected_module_slot() {
        let mut obj = owner_with_command();
        obj.children[0].modules.push(Module::text("Module", "// x"));
        let base = temp_base("badslot");
        let out = base.join("Catalogs").join("Спр.xml");
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        let err = write_command_modules(Format::Designer, "Catalog", &out, &obj).unwrap_err();
        match err {
            ConvertError::Write { reason, .. } => {
                assert!(reason.contains("CommandModule"), "got {reason}")
            }
            other => panic!("expected Write, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}
