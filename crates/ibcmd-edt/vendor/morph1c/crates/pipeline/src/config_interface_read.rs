//! Чтение/запись ЧЕТЫРЁХ КОНФИГ-УРОВНЕВЫХ ИНТЕРФЕЙС-САЙДКАРОВ КОРНЯ (§1.0/§1.6) в IR:
//! * корневой командный интерфейс — `Ext/CommandInterface.xml` Designer /
//!   `CommandInterface.cmi` EDT → [`MetadataObject::root_command_interface`] (cf `<host>.a`);
//! * командный интерфейс основного раздела — `Ext/MainSectionCommandInterface.xml` /
//!   `MainSectionCommandInterface.cmi` → `main_section_command_interface` (cf `<host>.9`);
//! * рабочая область начальной страницы — `Ext/HomePageWorkArea.xml` /
//!   `HomePageWorkArea.hpwa` → `home_page_work_area` (cf `<host>.8`);
//! * интерфейс клиентского приложения — `Ext/ClientApplicationInterface.xml` /
//!   `ClientApplicationInterface.cai` → `client_application_interface` (cf `<host>.b`).
//!
//! Сосед [`crate::ext_read`] (тот зовёт attach/write отсюда на конфиг-Ext-проходе) и
//! [`crate::standalone_content_read`] (тот же приём §1.0-самопроверки: пере-сериализация
//! канона ОБЯЗАНА воспроизвести исходные байты — иначе файл несёт не смоделированную
//! кодеком байт-форму, громкий отказ вместо тихой нормализации).
//!
//! # Диалекты (витнесс ERP — единственный носитель корпуса; SSL/coverage файлов НЕ имеют)
//! * Оба cmi-сайдкара — ТОТ ЖЕ диалект, что подсистемный `CommandInterface`
//!   ([`crate::cmi_read`], включая BOM/CRLF/trailing-конвенции), с ОДНИМ отличием:
//!   Designer-`<Placement>` здесь `Manual` (148/148), не `Auto` — параметр кодека.
//! * `HomePageWorkArea` — элементные диалекты (designer `extrnprops`+`version` / EDT
//!   `hpwa`-ns), шаблон + 2 колонки форм с высотой и видимостью (Common witnessed;
//!   по-ролевые значения кодируются по образцу cmi — общая под-схема Visibility).
//! * `ClientApplicationInterface` — СТРУКТУРНО РАЗНЫЕ диалекты (как `.scc`): designer
//!   `managed-application/core` (`xsi:type="InterfaceLayouter"`, БЕЗ version) ссылает
//!   панели на platform-uuid и несёт список `<panelDef>`; EDT (`g5.1c.ru/v8/dt/cai`,
//!   атрибутный) зовёт панели ИМЕНАМИ и перечисляет неразмещённые `<unset>`. Связка —
//!   [`STANDARD_CLIENT_PANELS`]; ОБЕ проекции выводимы из одного канона (§1.6):
//!   designer-`<panelDef>`-список ОБЯЗАН быть полной таблицей в её порядке, EDT-`<unset>`
//!   ОБЯЗАН быть дополнением размещённых панелей в порядке таблицы (§1.0-проверяется);
//!   `displayType="PictureAndText"` witnessed ТОЛЬКО у SectionPanel — константа проекции.

use std::path::Path;

use formats_xml::registry::Format;
use morph1c_core::ir::{
    CaiGroup, CaiPanel, ClientApplicationInterface, CommandInterface, HomePageItem,
    HomePageWorkArea, MetadataObject, RoleVisibility, STANDARD_CLIENT_PANELS,
};
use morph1c_core::version::FormatVersion;

use crate::standalone_content_read::{bool_text, name_text, parse_bool, parse_u64, Cursor};
use crate::ConvertError;

/// UTF-8 BOM — Designer-файлы его несут, EDT нет (witnessed ERP, как все Ext-сайдкары).
const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// `(designer-имя в Ext/, edt-имя рядом с Configuration.mdo)` для каждого сайдкара.
const ROOT_CI_FILES: (&str, &str) = ("CommandInterface.xml", "CommandInterface.cmi");
const MAIN_SECTION_CI_FILES: (&str, &str) = (
    "MainSectionCommandInterface.xml",
    "MainSectionCommandInterface.cmi",
);
const HPWA_FILES: (&str, &str) = ("HomePageWorkArea.xml", "HomePageWorkArea.hpwa");
const CAI_FILES: (&str, &str) = (
    "ClientApplicationInterface.xml",
    "ClientApplicationInterface.cai",
);

/// Подгрузить все четыре интерфейс-сайдкара (какие существуют) в поля корня. Зовётся из
/// [`crate::ext_read::attach_config_ext`] с уже вычисленным конфиг-Ext-каталогом;
/// отсутствие файла — честное отсутствие (SSL/coverage), no-op. §1.0: файл, чья
/// пере-сериализация не воспроизводит исходные байты → громкий отказ.
pub(crate) fn attach_config_interfaces(
    format: Format,
    ext_dir: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(()); // контейнер — не файловые сайдкары.
    }
    let ident = obj_ident(obj);
    if let Some(ci) = read_config_cmi(format, ext_dir, ROOT_CI_FILES, obj)? {
        put(
            &mut obj.root_command_interface,
            ci,
            "root command interface",
            ident.clone(),
        )?;
    }
    if let Some(ci) = read_config_cmi(format, ext_dir, MAIN_SECTION_CI_FILES, obj)? {
        put(
            &mut obj.main_section_command_interface,
            ci,
            "main section command interface",
            ident.clone(),
        )?;
    }
    if let Some(hp) = read_hpwa(format, ext_dir, obj)? {
        put(
            &mut obj.home_page_work_area,
            hp,
            "home page work area",
            ident.clone(),
        )?;
    }
    if let Some(cai) = read_cai(format, ext_dir, obj)? {
        put(
            &mut obj.client_application_interface,
            cai,
            "client application interface",
            ident,
        )?;
    }
    Ok(())
}

/// Write-side mirror of [`attach_config_interfaces`]: эмитить присутствующие интерфейс-поля
/// корня в конфиг-Ext-каталог целевого формата (пер-форматные ИМЯ и ДИАЛЕКТ; Designer-версия
/// — амбьентный round-trip-таргет). Отсутствующее поле — no-op.
pub(crate) fn write_config_interfaces(
    format: Format,
    ext_dir: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(()); // контейнер — тела `<host>.{8,9,a,b}` собирает cf-ассемблер.
    }
    let target = crate::sidecar_version::write_target();
    if let Some(ci) = &obj.root_command_interface {
        write_file(format, ext_dir, ROOT_CI_FILES, &config_cmi_bytes(format, ci, target))?;
    }
    if let Some(ci) = &obj.main_section_command_interface {
        write_file(
            format,
            ext_dir,
            MAIN_SECTION_CI_FILES,
            &config_cmi_bytes(format, ci, target),
        )?;
    }
    if let Some(hp) = &obj.home_page_work_area {
        let bytes = match format {
            Format::Designer => serialize_hpwa_designer(hp, target),
            Format::Edt => serialize_hpwa_edt(hp),
            Format::Cf => unreachable!("cf returned above"),
        };
        write_file(format, ext_dir, HPWA_FILES, &bytes)?;
    }
    if let Some(cai) = &obj.client_application_interface {
        let bytes = match format {
            Format::Designer => serialize_cai_designer(cai),
            Format::Edt => serialize_cai_edt(cai),
            Format::Cf => unreachable!("cf returned above"),
        };
        write_file(format, ext_dir, CAI_FILES, &bytes)?;
    }
    Ok(())
}

/// `(kind, name)` корня для сообщений об ошибках.
fn obj_ident(obj: &MetadataObject) -> String {
    obj.name.clone()
}

/// §1.0: спутник уже присутствует до attach'а (дескрипторная проекция не должна его
/// населять) → типизированный отказ; иначе — установка.
fn put<T>(slot: &mut Option<T>, value: T, what: &str, object: String) -> Result<(), ConvertError> {
    if slot.is_some() {
        return Err(ConvertError::Read {
            kind: "Configuration".to_string(),
            object,
            reason: format!(
                "object already carries a {what} before the sidecar attach (unexpected — the \
                 descriptor projection must not populate it)"
            ),
        });
    }
    *slot = Some(value);
    Ok(())
}

/// Пер-форматное имя файла сайдкара.
fn file_name(format: Format, files: (&'static str, &'static str)) -> &'static str {
    match format {
        Format::Designer => files.0,
        Format::Edt => files.1,
        Format::Cf => unreachable!("cf has no config-interface sidecar files"),
    }
}

fn write_file(
    format: Format,
    ext_dir: &Path,
    files: (&'static str, &'static str),
    bytes: &[u8],
) -> Result<(), ConvertError> {
    crate::form_write::write_file(&ext_dir.join(file_name(format, files)), bytes)
}

fn read_err(obj: &MetadataObject, path: &Path, reason: String) -> ConvertError {
    ConvertError::Read {
        kind: "Configuration".to_string(),
        object: obj.name.clone(),
        reason: format!("config interface sidecar {}: {reason}", path.display()),
    }
}

/// §1.0-самопроверка: пере-сериализация канона обязана дать ИСХОДНЫЕ байты.
fn self_check(
    obj: &MetadataObject,
    path: &Path,
    bytes: &[u8],
    back: &[u8],
) -> Result<(), ConvertError> {
    if back != bytes {
        return Err(read_err(
            obj,
            path,
            "does not round-trip byte-exactly through the IR (the sidecar carries a byte-shape \
             this codec does not model — refusing to silently normalize it, §1.0)"
                .into(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Конфиг-уровневые cmi-сайдкары (кодек — crate::cmi_read, placement `Manual`)
// ---------------------------------------------------------------------------

/// Прочитать один конфиг-уровневый cmi-сайдкар (если существует): парс cmi-кодеком с
/// Designer-`<Placement>` `Manual` + §1.0-самопроверка re-serialize (Designer — версией
/// ИСТОЧНИКА: версия — свойство файла, §1.6).
fn read_config_cmi(
    format: Format,
    ext_dir: &Path,
    files: (&'static str, &'static str),
    obj: &MetadataObject,
) -> Result<Option<CommandInterface>, ConvertError> {
    let path = ext_dir.join(file_name(format, files));
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let ci = crate::cmi_read::parse_command_interface_with_placement(
        format,
        &bytes,
        crate::cmi_read::PLACEMENT_MANUAL,
    )
    .map_err(|reason| read_err(obj, &path, reason))?;
    let back = match format {
        Format::Designer => {
            let version = designer_root_version(&bytes).map_err(|r| read_err(obj, &path, r))?;
            crate::cmi_read::serialize_designer_with_placement(
                &ci,
                version,
                crate::cmi_read::PLACEMENT_MANUAL,
            )
        }
        Format::Edt => crate::cmi_read::serialize_edt(&ci),
        Format::Cf => unreachable!("cf returned in the caller"),
    };
    self_check(obj, &path, &bytes, &back)?;
    Ok(Some(ci))
}

/// Пер-форматные байты конфиг-уровневого cmi-сайдкара из канона (запись).
fn config_cmi_bytes(format: Format, ci: &CommandInterface, target: FormatVersion) -> Vec<u8> {
    match format {
        Format::Designer => crate::cmi_read::serialize_designer_with_placement(
            ci,
            target,
            crate::cmi_read::PLACEMENT_MANUAL,
        ),
        Format::Edt => crate::cmi_read::serialize_edt(ci),
        Format::Cf => unreachable!("cf returned in the caller"),
    }
}

/// Witnessed-версия `version="…"` корневого Designer-тега (для самопроверки cmi-сайдкаров —
/// их парсер версию валидирует, но не возвращает).
fn designer_root_version(bytes: &[u8]) -> Result<FormatVersion, String> {
    let doc = formats_xml::parse(bytes).map_err(|e| format!("XML: {e}"))?;
    let version = doc
        .root
        .attr("version")
        .ok_or_else(|| "root has no version attribute (§1.0)".to_string())?;
    crate::sidecar_version::parse_witnessed(&version.value, "config interface sidecar")
}

// ---------------------------------------------------------------------------
// HomePageWorkArea — Designer-диалект
// ---------------------------------------------------------------------------

/// Designer: пролог + голова корневого тега ДО значения `version="…"` (witnessed ERP —
/// тот же ns-блок, что у подсистемного CommandInterface + основной ns extrnprops).
const HPWA_DESIGNER_HEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<HomePageWorkArea \
                                  xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" \
                                  xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" \
                                  xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" \
                                  xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
                                  version=\"";
/// EDT: пролог + корневой тег целиком (версии нет — witnessed ERP).
const HPWA_EDT_HEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<hpwa:HomePageWorkArea \
                             xmlns:hpwa=\"http://g5.1c.ru/v8/dt/hpwa\">\r\n";

/// Прочитать сайдкар рабочей области (если существует): строгий парс + §1.0-самопроверка.
fn read_hpwa(
    format: Format,
    ext_dir: &Path,
    obj: &MetadataObject,
) -> Result<Option<HomePageWorkArea>, ConvertError> {
    let path = ext_dir.join(file_name(format, HPWA_FILES));
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let (hp, back) = match format {
        Format::Designer => {
            let (hp, v) = parse_hpwa_designer(&bytes).map_err(|r| read_err(obj, &path, r))?;
            let back = serialize_hpwa_designer(&hp, v);
            (hp, back)
        }
        Format::Edt => {
            let hp = parse_hpwa_edt(&bytes).map_err(|r| read_err(obj, &path, r))?;
            let back = serialize_hpwa_edt(&hp);
            (hp, back)
        }
        Format::Cf => unreachable!("cf returned in the caller"),
    };
    self_check(obj, &path, &bytes, &back)?;
    Ok(Some(hp))
}

/// Разобрать Designer-байты → (канон, witnessed-версия источника). Строгий последовательный
/// парс witnessed-формы (см. [`crate::standalone_content_read`] — тот же приём).
fn parse_hpwa_designer(bytes: &[u8]) -> Result<(HomePageWorkArea, FormatVersion), String> {
    let body = bytes
        .strip_prefix(BOM)
        .ok_or_else(|| "Designer sidecar carries no BOM (witnessed: it DOES) — §1.0".to_string())?;
    let text = std::str::from_utf8(body).map_err(|e| format!("not UTF-8: {e}"))?;
    let mut cur = Cursor::new(text);
    cur.expect(HPWA_DESIGNER_HEAD)?;
    let version = cur.until("\"")?;
    let version = crate::sidecar_version::parse_witnessed(&version, "home page work area")?;
    cur.expect(">\r\n\t<WorkingAreaTemplate>")?;
    let template = name_text(cur.until("</WorkingAreaTemplate>\r\n")?)?;
    let mut hp = HomePageWorkArea {
        template,
        left: Vec::new(),
        right: Vec::new(),
    };
    for region in ["LeftColumn", "RightColumn"] {
        cur.expect(&format!("\t<{region}>\r\n"))?;
        let mut items = Vec::new();
        while cur.try_expect("\t\t<Item>\r\n\t\t\t<Form>") {
            let form = name_text(cur.until("</Form>\r\n\t\t\t<Height>")?)?;
            let height = parse_u64(&cur.until("</Height>\r\n")?)?;
            cur.expect("\t\t\t<Visibility>\r\n\t\t\t\t<xr:Common>")?;
            let common_visible = parse_bool(&cur.until("</xr:Common>\r\n")?)?;
            let mut role_values = Vec::new();
            while cur.try_expect("\t\t\t\t<xr:Value name=\"") {
                let role = name_text(cur.until("\">")?)?;
                let visible = parse_bool(&cur.until("</xr:Value>\r\n")?)?;
                role_values.push(RoleVisibility { role, visible });
            }
            cur.expect("\t\t\t</Visibility>\r\n\t\t</Item>\r\n")?;
            items.push(HomePageItem {
                form,
                height,
                common_visible,
                role_values,
            });
        }
        cur.expect(&format!("\t</{region}>\r\n"))?;
        if region == "LeftColumn" {
            hp.left = items;
        } else {
            hp.right = items;
        }
    }
    cur.expect("</HomePageWorkArea>")?;
    cur.expect_eof()?;
    Ok((hp, version))
}

/// Designer-байты из канона под ЗАДАННОЙ версией (обратная [`parse_hpwa_designer`]).
fn serialize_hpwa_designer(hp: &HomePageWorkArea, version: FormatVersion) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(HPWA_DESIGNER_HEAD);
    out.push_str(&version.to_string());
    out.push_str("\">\r\n\t<WorkingAreaTemplate>");
    out.push_str(&hp.template);
    out.push_str("</WorkingAreaTemplate>\r\n");
    for (region, items) in [("LeftColumn", &hp.left), ("RightColumn", &hp.right)] {
        out.push_str(&format!("\t<{region}>\r\n"));
        for it in items {
            out.push_str("\t\t<Item>\r\n\t\t\t<Form>");
            out.push_str(&it.form);
            out.push_str("</Form>\r\n\t\t\t<Height>");
            out.push_str(&it.height.to_string());
            out.push_str("</Height>\r\n\t\t\t<Visibility>\r\n\t\t\t\t<xr:Common>");
            out.push_str(bool_text(it.common_visible));
            out.push_str("</xr:Common>\r\n");
            for rv in &it.role_values {
                out.push_str("\t\t\t\t<xr:Value name=\"");
                out.push_str(&rv.role);
                out.push_str("\">");
                out.push_str(bool_text(rv.visible));
                out.push_str("</xr:Value>\r\n");
            }
            out.push_str("\t\t\t</Visibility>\r\n\t\t</Item>\r\n");
        }
        out.push_str(&format!("\t</{region}>\r\n"));
    }
    out.push_str("</HomePageWorkArea>");
    let mut bytes = BOM.to_vec();
    bytes.extend_from_slice(out.as_bytes());
    bytes
}

// ---------------------------------------------------------------------------
// HomePageWorkArea — EDT-диалект
// ---------------------------------------------------------------------------

/// Разобрать EDT-байты → канон. `<visibility/>` == Common false без ролей; `<common>true`
/// эмитится только при TRUE; роли — `<for>` по образцу `.cmi` (общая под-схема Visibility).
fn parse_hpwa_edt(bytes: &[u8]) -> Result<HomePageWorkArea, String> {
    if bytes.starts_with(BOM) {
        return Err("EDT sidecar carries a BOM (witnessed: it does NOT) — §1.0".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|e| format!("not UTF-8: {e}"))?;
    let mut cur = Cursor::new(text);
    cur.expect(HPWA_EDT_HEAD)?;
    cur.expect("  <workingAreaTemplate>")?;
    let template = name_text(cur.until("</workingAreaTemplate>\r\n")?)?;
    let mut hp = HomePageWorkArea {
        template,
        left: Vec::new(),
        right: Vec::new(),
    };
    for side in 0..2 {
        cur.expect("  <columns>\r\n")?;
        let mut items = Vec::new();
        while cur.try_expect("    <items>\r\n      <form>") {
            let form = name_text(cur.until("</form>\r\n      <height>")?)?;
            let height = parse_u64(&cur.until("</height>\r\n")?)?;
            let (common_visible, role_values) = if cur.try_expect("      <visibility/>\r\n") {
                (false, Vec::new())
            } else {
                cur.expect("      <visibility>\r\n")?;
                let common = cur.try_expect("        <common>true</common>\r\n");
                let mut roles = Vec::new();
                while cur.try_expect("        <for>\r\n") {
                    let visible = cur.try_expect("          <value>true</value>\r\n");
                    cur.expect("          <role>")?;
                    let role = name_text(cur.until("</role>\r\n        </for>\r\n")?)?;
                    roles.push(RoleVisibility { role, visible });
                }
                cur.expect("      </visibility>\r\n")?;
                (common, roles)
            };
            cur.expect("    </items>\r\n")?;
            items.push(HomePageItem {
                form,
                height,
                common_visible,
                role_values,
            });
        }
        cur.expect("  </columns>\r\n")?;
        if side == 0 {
            hp.left = items;
        } else {
            hp.right = items;
        }
    }
    cur.expect("</hpwa:HomePageWorkArea>\r\n")?;
    cur.expect_eof()?;
    Ok(hp)
}

/// EDT-байты из канона (обратная [`parse_hpwa_edt`]).
fn serialize_hpwa_edt(hp: &HomePageWorkArea) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(HPWA_EDT_HEAD);
    out.push_str("  <workingAreaTemplate>");
    out.push_str(&hp.template);
    out.push_str("</workingAreaTemplate>\r\n");
    for items in [&hp.left, &hp.right] {
        out.push_str("  <columns>\r\n");
        for it in items {
            out.push_str("    <items>\r\n      <form>");
            out.push_str(&it.form);
            out.push_str("</form>\r\n      <height>");
            out.push_str(&it.height.to_string());
            out.push_str("</height>\r\n");
            if !it.common_visible && it.role_values.is_empty() {
                out.push_str("      <visibility/>\r\n");
            } else {
                out.push_str("      <visibility>\r\n");
                if it.common_visible {
                    out.push_str("        <common>true</common>\r\n");
                }
                for rv in &it.role_values {
                    out.push_str("        <for>\r\n");
                    if rv.visible {
                        out.push_str("          <value>true</value>\r\n");
                    }
                    out.push_str("          <role>");
                    out.push_str(&rv.role);
                    out.push_str("</role>\r\n        </for>\r\n");
                }
                out.push_str("      </visibility>\r\n");
            }
            out.push_str("    </items>\r\n");
        }
        out.push_str("  </columns>\r\n");
    }
    out.push_str("</hpwa:HomePageWorkArea>\r\n");
    out.into_bytes()
}

// ---------------------------------------------------------------------------
// ClientApplicationInterface — Designer-диалект
// ---------------------------------------------------------------------------

/// Designer: пролог + корневой тег ЦЕЛИКОМ (версии НЕТ — witnessed ERP; диалект
/// `managed-application/core`, `xsi:type="InterfaceLayouter"`).
const CAI_DESIGNER_HEAD: &str =
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<ClientApplicationInterface \
     xmlns=\"http://v8.1c.ru/8.2/managed-application/core\" \
     xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" \
     xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
     xsi:type=\"InterfaceLayouter\">\r\n";
/// EDT: пролог + корневой тег целиком.
const CAI_EDT_HEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
                            <cai:ClientApplicationInterface \
                            xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
                            xmlns:cai=\"http://g5.1c.ru/v8/dt/cai\">\r\n";

/// Витнессированный EDT-`displayType` (ТОЛЬКО у SectionPanel) — константа проекции.
const SECTION_PANEL: &str = "SectionPanel";
const SECTION_DISPLAY_TYPE: &str = "PictureAndText";

/// Имя стандартной панели по её platform-uuid (§1.0: незнакомый uuid → отказ).
fn panel_name_for_uuid(uuid: &str) -> Result<&'static str, String> {
    STANDARD_CLIENT_PANELS
        .iter()
        .find(|(_, u)| *u == uuid)
        .map(|(n, _)| *n)
        .ok_or_else(|| {
            format!(
                "panel uuid {uuid:?} is not a witnessed standard client panel (§1.0 — no guess)"
            )
        })
}

/// Platform-uuid стандартной панели по имени (§1.0: незнакомое имя → отказ).
fn panel_uuid_for_name(name: &str) -> Result<&'static str, String> {
    STANDARD_CLIENT_PANELS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, u)| *u)
        .ok_or_else(|| {
            let known: Vec<&str> = STANDARD_CLIENT_PANELS.iter().map(|(n, _)| *n).collect();
            format!(
                "panel {name:?} is not a witnessed standard client panel (known: {}; §1.0 — \
                 no guess)",
                known.join("/")
            )
        })
}

/// Неразмещённые панели канона = таблица МИНУС размещённые, в порядке таблицы (§1.6:
/// designer-`<panelDef>`-список и EDT-`<unset>`-список оба выводимы из канона).
fn unset_panels(cai: &ClientApplicationInterface) -> Vec<&'static str> {
    let used: Vec<&str> = cai
        .top
        .panels
        .iter()
        .chain(cai.left.panels.iter())
        .map(|p| p.name.as_str())
        .collect();
    STANDARD_CLIENT_PANELS
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| !used.contains(n))
        .collect()
}

/// Прочитать сайдкар интерфейса клиентского приложения (если существует): строгий парс +
/// §1.0-самопроверка.
fn read_cai(
    format: Format,
    ext_dir: &Path,
    obj: &MetadataObject,
) -> Result<Option<ClientApplicationInterface>, ConvertError> {
    let path = ext_dir.join(file_name(format, CAI_FILES));
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let (cai, back) = match format {
        Format::Designer => {
            let cai = parse_cai_designer(&bytes).map_err(|r| read_err(obj, &path, r))?;
            let back = serialize_cai_designer(&cai);
            (cai, back)
        }
        Format::Edt => {
            let cai = parse_cai_edt(&bytes).map_err(|r| read_err(obj, &path, r))?;
            let back = serialize_cai_edt(&cai);
            (cai, back)
        }
        Format::Cf => unreachable!("cf returned in the caller"),
    };
    self_check(obj, &path, &bytes, &back)?;
    Ok(Some(cai))
}

/// Разобрать Designer-байты → канон. Каждая панель обёрнута АНОНИМНОЙ `<group>` с ровно
/// одним `<panel>` (структурный шум разметки — в каноне не представлен); `<panelDef>`-список
/// §1.0-ОБЯЗАН быть полной таблицей [`STANDARD_CLIENT_PANELS`] в её порядке.
fn parse_cai_designer(bytes: &[u8]) -> Result<ClientApplicationInterface, String> {
    let body = bytes
        .strip_prefix(BOM)
        .ok_or_else(|| "Designer sidecar carries no BOM (witnessed: it DOES) — §1.0".to_string())?;
    let text = std::str::from_utf8(body).map_err(|e| format!("not UTF-8: {e}"))?;
    let mut cur = Cursor::new(text);
    cur.expect(CAI_DESIGNER_HEAD)?;
    let mut groups: Vec<CaiGroup> = Vec::new();
    for region in ["top", "left"] {
        cur.expect(&format!("\t<{region}>\r\n\t\t<group id=\""))?;
        let id = name_text(cur.until("\">\r\n")?)?;
        let mut panels = Vec::new();
        while cur.try_expect("\t\t\t<group>\r\n\t\t\t\t<panel id=\"") {
            let pid = name_text(cur.until("\">\r\n\t\t\t\t\t<uuid>")?)?;
            let uuid = name_text(cur.until("</uuid>\r\n\t\t\t\t</panel>\r\n\t\t\t</group>\r\n")?)?;
            panels.push(CaiPanel {
                id: pid,
                name: panel_name_for_uuid(&uuid)?.to_string(),
            });
        }
        cur.expect(&format!("\t\t</group>\r\n\t</{region}>\r\n"))?;
        groups.push(CaiGroup { id, panels });
    }
    // `<panelDef>`-список: ПОЛНАЯ таблица в её порядке (witnessed ERP; §1.0 — отклонение
    // означает не смоделированную проекцию, не тихий пере-порядок).
    for (_, uuid) in STANDARD_CLIENT_PANELS {
        cur.expect(&format!("\t<panelDef id=\"{uuid}\"/>\r\n"))?;
    }
    cur.expect("</ClientApplicationInterface>")?;
    cur.expect_eof()?;
    let left = groups.pop().expect("two regions parsed");
    let top = groups.pop().expect("two regions parsed");
    Ok(ClientApplicationInterface { top, left })
}

/// Designer-байты из канона (обратная [`parse_cai_designer`]).
fn serialize_cai_designer(cai: &ClientApplicationInterface) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(CAI_DESIGNER_HEAD);
    for (region, group) in [("top", &cai.top), ("left", &cai.left)] {
        out.push_str(&format!("\t<{region}>\r\n\t\t<group id=\""));
        out.push_str(&group.id);
        out.push_str("\">\r\n");
        for p in &group.panels {
            out.push_str("\t\t\t<group>\r\n\t\t\t\t<panel id=\"");
            out.push_str(&p.id);
            out.push_str("\">\r\n\t\t\t\t\t<uuid>");
            // Канон гарантированно несёт witnessed-имя (парсеры §1.0-гейтят) — но чужой
            // IR мог быть собран вручную: отказ громкий, не panic.
            out.push_str(panel_uuid_for_name(&p.name).unwrap_or(&p.name));
            out.push_str("</uuid>\r\n\t\t\t\t</panel>\r\n\t\t\t</group>\r\n");
        }
        out.push_str(&format!("\t\t</group>\r\n\t</{region}>\r\n"));
    }
    for (_, uuid) in STANDARD_CLIENT_PANELS {
        out.push_str(&format!("\t<panelDef id=\"{uuid}\"/>\r\n"));
    }
    out.push_str("</ClientApplicationInterface>");
    let mut bytes = BOM.to_vec();
    bytes.extend_from_slice(out.as_bytes());
    bytes
}

// ---------------------------------------------------------------------------
// ClientApplicationInterface — EDT-диалект
// ---------------------------------------------------------------------------

/// Разобрать EDT-байты → канон. Панели — по ИМЕНИ; `displayType` witnessed ТОЛЬКО у
/// SectionPanel (константа проекции); `<unset>` §1.0-ОБЯЗАН быть дополнением размещённых
/// панелей в порядке таблицы, с таблично-верными id.
fn parse_cai_edt(bytes: &[u8]) -> Result<ClientApplicationInterface, String> {
    if bytes.starts_with(BOM) {
        return Err("EDT sidecar carries a BOM (witnessed: it does NOT) — §1.0".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|e| format!("not UTF-8: {e}"))?;
    let mut cur = Cursor::new(text);
    cur.expect(CAI_EDT_HEAD)?;
    let mut groups: Vec<CaiGroup> = Vec::new();
    for region in ["top", "left"] {
        cur.expect(&format!("  <{region} xsi:type=\"cai:CaiGroup\" id=\""))?;
        let id = name_text(cur.until("\">\r\n")?)?;
        let mut panels = Vec::new();
        while cur.try_expect("    <panels id=\"") {
            let pid = name_text(cur.until("\" name=\"")?)?;
            let name = name_text(cur.until("\"")?)?;
            // §1.0: `displayType` — witnessed-константа SectionPanel; прочие панели без него.
            if name == SECTION_PANEL {
                cur.expect(&format!(" displayType=\"{SECTION_DISPLAY_TYPE}\"/>\r\n"))?;
            } else {
                cur.expect("/>\r\n")?;
            }
            panel_uuid_for_name(&name)?; // §1.0: незнакомое имя — отказ уже на чтении.
            panels.push(CaiPanel { id: pid, name });
        }
        cur.expect(&format!("  </{region}>\r\n"))?;
        groups.push(CaiGroup { id, panels });
    }
    let left = groups.pop().expect("two regions parsed");
    let top = groups.pop().expect("two regions parsed");
    let cai = ClientApplicationInterface { top, left };
    // `<unset>`-список: дополнение размещённых панелей в порядке таблицы, id — платформенный
    // uuid панели (witnessed ERP; §1.0 — отклонение не смоделировано).
    for name in unset_panels(&cai) {
        let uuid = panel_uuid_for_name(name).expect("table name");
        cur.expect(&format!("  <unset id=\"{uuid}\" name=\"{name}\"/>\r\n"))?;
    }
    cur.expect("</cai:ClientApplicationInterface>\r\n")?;
    cur.expect_eof()?;
    Ok(cai)
}

/// EDT-байты из канона (обратная [`parse_cai_edt`]).
fn serialize_cai_edt(cai: &ClientApplicationInterface) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(CAI_EDT_HEAD);
    for (region, group) in [("top", &cai.top), ("left", &cai.left)] {
        out.push_str(&format!("  <{region} xsi:type=\"cai:CaiGroup\" id=\""));
        out.push_str(&group.id);
        out.push_str("\">\r\n");
        for p in &group.panels {
            out.push_str("    <panels id=\"");
            out.push_str(&p.id);
            out.push_str("\" name=\"");
            out.push_str(&p.name);
            if p.name == SECTION_PANEL {
                out.push_str(&format!("\" displayType=\"{SECTION_DISPLAY_TYPE}\"/>\r\n"));
            } else {
                out.push_str("\"/>\r\n");
            }
        }
        out.push_str(&format!("  </{region}>\r\n"));
    }
    for name in unset_panels(cai) {
        let uuid = panel_uuid_for_name(name).expect("table name");
        out.push_str(&format!("  <unset id=\"{uuid}\" name=\"{name}\"/>\r\n"));
    }
    out.push_str("</cai:ClientApplicationInterface>\r\n");
    out.into_bytes()
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::version::{ERP, SSL};

    /// Микро-канон hpwa (витнесс-эхо ERP в миниатюре: item Common-true + item-«пустая
    /// видимость» + правая колонка).
    fn micro_hpwa() -> HomePageWorkArea {
        HomePageWorkArea {
            template: "TwoColumnsVariableWidth".to_string(),
            left: vec![
                HomePageItem {
                    form: "Task.ЗадачаИсполнителя.Form.МоиЗадачи".to_string(),
                    height: 10,
                    common_visible: true,
                    role_values: Vec::new(),
                },
                HomePageItem {
                    form: "Catalog.Заметки.Form.МоиЗаметки".to_string(),
                    height: 10,
                    common_visible: false,
                    role_values: Vec::new(),
                },
            ],
            right: vec![HomePageItem {
                form: "DataProcessor.ТекущиеДела.Form.Форма".to_string(),
                height: 10,
                common_visible: true,
                role_values: Vec::new(),
            }],
        }
    }

    /// Микро-канон cai (полное витнесс-эхо ERP: те же панели/id, что фикстура).
    fn micro_cai() -> ClientApplicationInterface {
        ClientApplicationInterface {
            top: CaiGroup {
                id: "1b4042b7-6a75-4b3a-a7cd-e5306844686c".to_string(),
                panels: vec![
                    CaiPanel {
                        id: "1c6905f0-bef8-4e56-958c-25c457268b87".to_string(),
                        name: "ToolsPanel".to_string(),
                    },
                    CaiPanel {
                        id: "074990e2-40ee-43fa-9a09-eeab8933d830".to_string(),
                        name: "OpenedPanel".to_string(),
                    },
                ],
            },
            left: CaiGroup {
                id: "5e17de8d-d23a-4fad-ba89-943c4cc9deb6".to_string(),
                panels: vec![CaiPanel {
                    id: "de5a40ef-33e7-45fb-a690-73fc69670243".to_string(),
                    name: "SectionPanel".to_string(),
                }],
            },
        }
    }

    /// hpwa: оба диалекта микрофикстуры → РАВНЫЙ канон (§1.6) + byte-exact re-serialize
    /// (§1.0); Designer-версия — параметр.
    #[test]
    fn hpwa_both_dialects_roundtrip_and_agree() {
        let hp = micro_hpwa();
        let dsn = serialize_hpwa_designer(&hp, ERP);
        let (from_dsn, v) = parse_hpwa_designer(&dsn).expect("designer parse");
        assert_eq!(v, ERP);
        assert_eq!(from_dsn, hp);
        assert_eq!(serialize_hpwa_designer(&from_dsn, ERP), dsn, "designer byte-exact");

        let edt = serialize_hpwa_edt(&hp);
        let from_edt = parse_hpwa_edt(&edt).expect("edt parse");
        assert_eq!(from_edt, hp, "§1.6: единый канон");
        assert_eq!(serialize_hpwa_edt(&from_edt), edt, "edt byte-exact");

        let ssl_bytes = serialize_hpwa_designer(&hp, SSL);
        let (_, v) = parse_hpwa_designer(&ssl_bytes).expect("parse 2.21");
        assert_eq!(v, SSL, "designer version is a parameter");
    }

    /// hpwa: по-ролевые значения видимости — оба диалекта, канон равный, byte-exact.
    #[test]
    fn hpwa_role_values_roundtrip() {
        let mut hp = micro_hpwa();
        hp.left[1].role_values = vec![
            RoleVisibility {
                role: "Role.Маркетолог".to_string(),
                visible: true,
            },
            RoleVisibility {
                role: "Role.Админ".to_string(),
                visible: false,
            },
        ];
        let dsn = serialize_hpwa_designer(&hp, ERP);
        let (from_dsn, _) = parse_hpwa_designer(&dsn).expect("designer parse");
        assert_eq!(from_dsn, hp);
        let edt = serialize_hpwa_edt(&hp);
        let from_edt = parse_hpwa_edt(&edt).expect("edt parse");
        assert_eq!(from_edt, hp, "§1.6: единый канон (роли)");
        assert_eq!(serialize_hpwa_edt(&from_edt), edt);
    }

    /// cai: оба диалекта микрофикстуры → РАВНЫЙ канон (§1.6) + byte-exact re-serialize;
    /// designer несёт ПОЛНЫЙ `<panelDef>`-список, edt — `<unset>`-дополнение.
    #[test]
    fn cai_both_dialects_roundtrip_and_agree() {
        let cai = micro_cai();
        let dsn = serialize_cai_designer(&cai);
        let from_dsn = parse_cai_designer(&dsn).expect("designer parse");
        assert_eq!(from_dsn, cai);
        assert_eq!(serialize_cai_designer(&from_dsn), dsn, "designer byte-exact");

        let edt = serialize_cai_edt(&cai);
        let from_edt = parse_cai_edt(&edt).expect("edt parse");
        assert_eq!(from_edt, cai, "§1.6: единый канон");
        assert_eq!(serialize_cai_edt(&from_edt), edt, "edt byte-exact");

        let dsn_text = String::from_utf8(dsn).unwrap();
        assert_eq!(dsn_text.matches("<panelDef id=").count(), 6, "full table");
        let edt_text = String::from_utf8(edt).unwrap();
        assert_eq!(edt_text.matches("<unset id=").count(), 3, "complement");
        assert!(
            edt_text.contains("name=\"SectionPanel\" displayType=\"PictureAndText\""),
            "SectionPanel displayType const: {edt_text}"
        );
    }

    /// §1.0: незнакомый panel-uuid (designer) / чужой порядок panelDef — громкие отказы.
    #[test]
    fn cai_unwitnessed_shapes_are_refused() {
        let cai = micro_cai();
        let dsn = String::from_utf8(serialize_cai_designer(&cai)).unwrap();
        // Незнакомый uuid панели.
        let bad = dsn.replace(
            "8e10648b-f52d-4ec2-b4dd-87de33778d95</uuid>",
            "00000000-0000-0000-0000-000000000000</uuid>",
        );
        let err = parse_cai_designer(bad.as_bytes()).unwrap_err();
        assert!(err.contains("not a witnessed standard client panel"), "{err}");
        // panelDef-список не в порядке таблицы.
        let bad = dsn.replacen(
            "\t<panelDef id=\"b553047f-c9aa-4157-978d-448ecad24248\"/>\r\n",
            "",
            1,
        );
        let err = parse_cai_designer(bad.as_bytes()).unwrap_err();
        assert!(err.contains("§1.0"), "{err}");
    }

    /// §1.0: EDT `<unset>` не-дополнение (лишняя/отсутствующая панель) — громкий отказ.
    #[test]
    fn cai_edt_unset_mismatch_is_refused() {
        let cai = micro_cai();
        let edt = String::from_utf8(serialize_cai_edt(&cai)).unwrap();
        let bad = edt.replacen(
            "  <unset id=\"13322b22-3960-4d68-93a6-fe2dd7f28ca3\" name=\"FavoritePanel\"/>\r\n",
            "",
            1,
        );
        let err = parse_cai_edt(bad.as_bytes()).unwrap_err();
        assert!(err.contains("§1.0"), "{err}");
    }

    /// КЛЮЧЕВОЙ ИНТЕГРАЦИОННЫЙ: НАСТОЯЩИЕ ERP-фикстуры всех 4 сайдкаров × оба диалекта —
    /// парс, §1.6-равенство канонов designer==edt И byte-exact re-serialize каждого файла
    /// (самопроверка на реальном корпусе). Счётчики витнесса: root-CI 23 подсистемы;
    /// MSCI 146 команд/148 placement; hpwa 10+3 форм; cai 2+1 панели.
    #[test]
    fn erp_fixture_files_parse_equal_and_roundtrip() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dsn_ext = root.join(".fixtures/ERP/designer_8.3.27/Ext");
        let edt_cfg = root.join(".fixtures/ERP/edt/src/Configuration");
        if !dsn_ext.is_dir() || !edt_cfg.is_dir() {
            eprintln!("skipping: ERP fixtures not present");
            return;
        }
        let rd = |p: &Path| std::fs::read(p).unwrap();

        // (a) корневой CommandInterface — cmi-кодек, только subsystemsOrder (23).
        let dsn = rd(&dsn_ext.join("CommandInterface.xml"));
        let edt = rd(&edt_cfg.join("CommandInterface.cmi"));
        let ci_dsn = crate::cmi_read::parse_command_interface_with_placement(
            Format::Designer,
            &dsn,
            crate::cmi_read::PLACEMENT_MANUAL,
        )
        .expect("root CI designer parses");
        let ci_edt = crate::cmi_read::parse_command_interface_with_placement(
            Format::Edt,
            &edt,
            crate::cmi_read::PLACEMENT_MANUAL,
        )
        .expect("root CI edt parses");
        assert_eq!(ci_dsn, ci_edt, "§1.6: root CI canon");
        assert_eq!(ci_dsn.subsystems_order.len(), 23, "witnessed count");
        assert!(ci_dsn.commands.is_empty() && ci_dsn.placement.is_empty());
        assert_eq!(
            crate::cmi_read::serialize_designer_with_placement(
                &ci_dsn,
                ERP,
                crate::cmi_read::PLACEMENT_MANUAL
            ),
            dsn,
            "root CI designer byte-exact"
        );
        assert_eq!(crate::cmi_read::serialize_edt(&ci_edt), edt, "root CI edt byte-exact");

        // (b) MainSectionCommandInterface — cmi-кодек, placement `Manual`.
        let dsn = rd(&dsn_ext.join("MainSectionCommandInterface.xml"));
        let edt = rd(&edt_cfg.join("MainSectionCommandInterface.cmi"));
        let ci_dsn = crate::cmi_read::parse_command_interface_with_placement(
            Format::Designer,
            &dsn,
            crate::cmi_read::PLACEMENT_MANUAL,
        )
        .expect("MSCI designer parses");
        let ci_edt = crate::cmi_read::parse_command_interface_with_placement(
            Format::Edt,
            &edt,
            crate::cmi_read::PLACEMENT_MANUAL,
        )
        .expect("MSCI edt parses");
        assert_eq!(ci_dsn, ci_edt, "§1.6: MSCI canon");
        assert_eq!(ci_dsn.commands.len(), 146, "witnessed visibility count");
        let placement_total: usize = ci_dsn.placement.iter().map(|f| f.commands.len()).sum();
        assert_eq!(placement_total, 148, "witnessed placement count");
        assert_eq!(
            crate::cmi_read::serialize_designer_with_placement(
                &ci_dsn,
                ERP,
                crate::cmi_read::PLACEMENT_MANUAL
            ),
            dsn,
            "MSCI designer byte-exact"
        );
        assert_eq!(crate::cmi_read::serialize_edt(&ci_edt), edt, "MSCI edt byte-exact");

        // (c) HomePageWorkArea.
        let dsn = rd(&dsn_ext.join("HomePageWorkArea.xml"));
        let edt = rd(&edt_cfg.join("HomePageWorkArea.hpwa"));
        let (hp_dsn, v) = parse_hpwa_designer(&dsn).expect("hpwa designer parses");
        assert_eq!(v, ERP);
        let hp_edt = parse_hpwa_edt(&edt).expect("hpwa edt parses");
        assert_eq!(hp_dsn, hp_edt, "§1.6: hpwa canon");
        assert_eq!(hp_dsn.template, "TwoColumnsVariableWidth");
        assert_eq!((hp_dsn.left.len(), hp_dsn.right.len()), (10, 3));
        assert!(hp_dsn.left.iter().all(|i| i.height == 10));
        assert_eq!(serialize_hpwa_designer(&hp_dsn, ERP), dsn, "hpwa designer byte-exact");
        assert_eq!(serialize_hpwa_edt(&hp_edt), edt, "hpwa edt byte-exact");

        // (d) ClientApplicationInterface.
        let dsn = rd(&dsn_ext.join("ClientApplicationInterface.xml"));
        let edt = rd(&edt_cfg.join("ClientApplicationInterface.cai"));
        let cai_dsn = parse_cai_designer(&dsn).expect("cai designer parses");
        let cai_edt = parse_cai_edt(&edt).expect("cai edt parses");
        assert_eq!(cai_dsn, cai_edt, "§1.6: cai canon");
        assert_eq!(cai_dsn.top.panels.len(), 2);
        assert_eq!(cai_dsn.left.panels.len(), 1);
        assert_eq!(cai_dsn.left.panels[0].name, "SectionPanel");
        assert_eq!(serialize_cai_designer(&cai_dsn), dsn, "cai designer byte-exact");
        assert_eq!(serialize_cai_edt(&cai_edt), edt, "cai edt byte-exact");
    }

    // -----------------------------------------------------------------------
    // ЭНКОДЕРЫ vs ЭТАЛОННЫЕ cf-ТЕЛА (fixture-gated): реестр/ростер харвестится ИЗ
    // designer-фикстуры по ссылкам самих канонов — БОЕВЫЕ cf-кодеки обязаны дать
    // байт-в-байт эталонные inflate-тела erp.cf `<host>.{8,9,a,b}`.
    // -----------------------------------------------------------------------

    /// `вид → каталог` designer-дампа (только виды, на которые ссылаются 4 витнесс-канона).
    const KIND_DIRS: &[(&str, &str)] = &[
        ("BusinessProcess", "BusinessProcesses"),
        ("Catalog", "Catalogs"),
        ("CommandGroup", "CommandGroups"),
        ("CommonCommand", "CommonCommands"),
        ("CommonForm", "CommonForms"),
        ("DataProcessor", "DataProcessors"),
        ("Document", "Documents"),
        ("DocumentJournal", "DocumentJournals"),
        ("InformationRegister", "InformationRegisters"),
        ("Report", "Reports"),
        ("Subsystem", "Subsystems"),
        ("Task", "Tasks"),
    ];

    /// Харвест-кеш распарсенных designer-дескрипторов фикстуры.
    struct Harvest {
        fix: std::path::PathBuf,
        cache: std::collections::BTreeMap<std::path::PathBuf, formats_xml::Descriptor>,
    }

    impl Harvest {
        fn doc(&mut self, path: std::path::PathBuf) -> &formats_xml::Descriptor {
            self.cache.entry(path.clone()).or_insert_with(|| {
                let bytes = std::fs::read(&path)
                    .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
                formats_xml::parse(&bytes)
                    .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
            })
        }

        fn kind_dir(kind: &str) -> &'static str {
            KIND_DIRS
                .iter()
                .find(|(k, _)| *k == kind)
                .map(|(_, d)| *d)
                .unwrap_or_else(|| panic!("no kind dir for {kind}"))
        }

        /// uuid объекта `<Kind>.<Name>` из его дескриптора.
        fn top_uuid(&mut self, kind: &str, name: &str) -> String {
            let path = self
                .fix
                .join(Self::kind_dir(kind))
                .join(format!("{name}.xml"));
            let kind = kind.to_string();
            let doc = self.doc(path);
            doc.root
                .children
                .iter()
                .find(|ch| ch.local == kind)
                .and_then(|ch| ch.attr("uuid"))
                .map(|a| a.value.clone())
                .unwrap_or_else(|| panic!("{kind} node uuid"))
        }

        /// uuid КОМАНДЫ-ребёнка `<Kind>.<Name>.Command.<Cmd>` из ChildObjects дескриптора.
        fn command_uuid(&mut self, kind: &str, name: &str, cmd: &str) -> String {
            let path = self
                .fix
                .join(Self::kind_dir(kind))
                .join(format!("{name}.xml"));
            let kind = kind.to_string();
            let doc = self.doc(path);
            let obj = doc
                .root
                .children
                .iter()
                .find(|ch| ch.local == kind)
                .expect("object node");
            for co in obj.children.iter().filter(|c| c.local == "ChildObjects") {
                for c in co.children.iter().filter(|c| c.local == "Command") {
                    let is_it = c
                        .child("Properties")
                        .and_then(|p| p.child("Name"))
                        .map(|n| n.text == cmd)
                        .unwrap_or(false);
                    if is_it {
                        return c.attr("uuid").expect("command uuid").value.clone();
                    }
                }
            }
            panic!("{kind}.{name}.Command.{cmd} not found");
        }

        /// uuid ФОРМЫ `<Kind>.<Name>.Form.<Form>` из её собственного дескриптора.
        fn form_uuid(&mut self, kind: &str, name: &str, form: &str) -> String {
            let path = self
                .fix
                .join(Self::kind_dir(kind))
                .join(name)
                .join("Forms")
                .join(format!("{form}.xml"));
            let doc = self.doc(path);
            doc.root
                .children
                .iter()
                .find(|ch| ch.local == "Form")
                .and_then(|ch| ch.attr("uuid"))
                .map(|a| a.value.clone())
                .expect("form uuid")
        }

        /// Влить резолв одной cmd-ссылки канона в реестр (те же 3 witnessed-формы,
        /// что резолвит `formats_cf::cmi_body::command_ref`).
        fn feed_command_ref(&mut self, full: &str, reg: &mut formats_cf::BraceTypeRegistry) {
            if let Some((obj_path, _)) = full.split_once(".StandardCommand.") {
                let (kind, name) = obj_path.split_once('.').expect("kind.name");
                let uuid = self.top_uuid(kind, name);
                reg.insert_object_path(uuid, obj_path.to_string());
            } else if let Some((obj_path, cmd)) = full.split_once(".Command.") {
                let (kind, name) = obj_path.split_once('.').expect("kind.name");
                let uuid = self.command_uuid(kind, name, cmd);
                reg.insert_field_path(uuid, full.to_string());
            } else if full.starts_with("CommonCommand.") {
                let name = full.split_once('.').expect("dot").1;
                let uuid = self.top_uuid("CommonCommand", name);
                reg.insert_object_path(uuid, full.to_string());
            } else {
                panic!("unexpected command ref form {full:?}");
            }
        }

        /// Влить резолв группы КИ (пользовательские `CommandGroup.<Name>`; стандартные
        /// группы кодек знает сам).
        fn feed_group(&mut self, group: &str, reg: &mut formats_cf::BraceTypeRegistry) {
            if let Some(name) = group.strip_prefix("CommandGroup.") {
                let uuid = self.top_uuid("CommandGroup", name);
                reg.insert_object_path(uuid, group.to_string());
            }
        }
    }

    /// БОЕВЫЕ cf-кодеки на РАСПАРСЕННЫХ фикстурных канонах + харвестнутых из фикстуры
    /// реестре/ростере == эталонные inflate-тела erp.cf `<host>.{8,9,a,b}` БАЙТ-В-БАЙТ
    /// (эталоны — `.fixtures/ERP/cf_bodies/ref_cfg_*.bin`, извлечены из erp.cf).
    #[test]
    fn erp_fixture_interface_bodies_encode_byte_exact() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let fix = root.join(".fixtures/ERP/designer_8.3.27");
        let refs = root.join(".fixtures/ERP/cf_bodies");
        if !fix.is_dir() || !refs.is_dir() {
            eprintln!("skipping: ERP fixtures / cf_bodies refs not present");
            return;
        }
        let rd = |p: std::path::PathBuf| std::fs::read(&p).unwrap();
        let mut harvest = Harvest {
            fix: fix.clone(),
            cache: Default::default(),
        };

        // (a) корневой CommandInterface → `<host>.a` (cmi-кодек, config-уровень).
        let ci_root = crate::cmi_read::parse_command_interface_with_placement(
            Format::Designer,
            &rd(fix.join("Ext/CommandInterface.xml")),
            crate::cmi_read::PLACEMENT_MANUAL,
        )
        .expect("root CI parses");
        let mut reg = formats_cf::BraceTypeRegistry::default();
        for sub in &ci_root.subsystems_order {
            let name = sub.strip_prefix("Subsystem.").expect("Subsystem.<Name>");
            let uuid = harvest.top_uuid("Subsystem", name);
            reg.insert_object_path(uuid, sub.clone());
        }
        let img = formats_cf::cmi_body::build_config_command_interface_body_image_with(
            &ci_root, &reg,
        )
        .expect("encode .a");
        assert_eq!(img, rd(refs.join("ref_cfg_a.bin")), ".a byte-exact vs erp.cf");

        // (9) MainSectionCommandInterface → `<host>.9` (cmi-кодек, placement Manual).
        let msci = crate::cmi_read::parse_command_interface_with_placement(
            Format::Designer,
            &rd(fix.join("Ext/MainSectionCommandInterface.xml")),
            crate::cmi_read::PLACEMENT_MANUAL,
        )
        .expect("MSCI parses");
        let mut reg = formats_cf::BraceTypeRegistry::default();
        for cmd in &msci.commands {
            harvest.feed_command_ref(&cmd.command, &mut reg);
        }
        for frag in msci.placement.iter().chain(msci.order.iter()) {
            harvest.feed_group(&frag.group, &mut reg);
            for cmd in &frag.commands {
                harvest.feed_command_ref(cmd, &mut reg);
            }
        }
        let img =
            formats_cf::cmi_body::build_config_command_interface_body_image_with(&msci, &reg)
                .expect("encode .9");
        assert_eq!(img, rd(refs.join("ref_cfg_9.bin")), ".9 byte-exact vs erp.cf");

        // (8) HomePageWorkArea → `<host>.8` (форм-ссылки — form roster из фикстуры).
        let (hp, _) = parse_hpwa_designer(&rd(fix.join("Ext/HomePageWorkArea.xml")))
            .expect("hpwa parses");
        let mut roster = formats_cf::FormRoster::default();
        for it in hp.left.iter().chain(hp.right.iter()) {
            let seg: Vec<&str> = it.form.split('.').collect();
            assert_eq!(seg.len(), 4, "witnessed Kind.Name.Form.FormName: {}", it.form);
            assert_eq!(seg[2], "Form");
            let uuid = harvest.form_uuid(seg[0], seg[1], seg[3]);
            roster.insert(it.form.clone(), uuid);
        }
        let img = formats_cf::hpwa_body::build_home_page_work_area_body_image_with(
            &hp,
            &formats_cf::BraceTypeRegistry::default(),
            &roster,
        )
        .expect("encode .8");
        assert_eq!(img, rd(refs.join("ref_cfg_8.bin")), ".8 byte-exact vs erp.cf");

        // (b) ClientApplicationInterface → `<host>.b` (реестры не нужны).
        let cai = parse_cai_designer(&rd(fix.join("Ext/ClientApplicationInterface.xml")))
            .expect("cai parses");
        let img = formats_cf::cai_body::build_client_application_interface_body_image(&cai)
            .expect("encode .b");
        assert_eq!(img, rd(refs.join("ref_cfg_b.bin")), ".b byte-exact vs erp.cf");
    }
}
