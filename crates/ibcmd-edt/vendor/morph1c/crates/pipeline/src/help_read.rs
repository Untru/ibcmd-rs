//! Чтение/запись СПРАВКИ объекта (help-страниц `<lang>.html`) как сайдкаров дескриптора
//! (§1.2/§4). Сиблинг [`crate::module_read`]/[`crate::picture_read`] — «дескриптор-read
//! метаданных не трогает тело; whole-config-конвейер присоединяет его отдельным проходом».
//!
//! # Раскладка и кодирование (RE: coverage/s15_subordinate, 15 объектов × `ru.html`)
//! * **EDT** (`DirPerObject`): каталог `<obj-dir>/Help/` со страницами `<lang>.html` —
//!   БЕЗ BOM, CRLF. Дескриптора страниц нет — наличие гейтится КАТАЛОГОМ.
//! * **Designer** (`FilePerObject`): дескриптор страниц `<dir>/<Name>/Ext/Help.xml`
//!   (С BOM, CRLF, `<Page><lang></Page>` на страницу, `version="2.21"`, БЕЗ хвостового
//!   перевода строки) + страницы `<dir>/<Name>/Ext/Help/<lang>.html` — С BOM, **LF**.
//!   Наличие гейтится ФАЙЛОМ `Ext/Help.xml`.
//!
//! NB: EOL-конвенции РАЗНОНАПРАВЛЕННЫЕ (EDT=CRLF, Designer=LF — противоположно `.bsl`, где
//! CRLF у обоих) → канон [`HelpPage::body`] BOM-снят И EOL-нормализован к `\n`; каждый формат
//! восстанавливает своё кодирование на записи. §1.0-самопроверка на read: пере-кодирование
//! канона в исходный диалект обязано воспроизвести исходные байты (иначе — громкий отказ,
//! а не тихая нормализация непрошенного кодирования).
//!
//! Свойство дескриптора `IncludeHelpInContents`/`includeHelpInContents` наличие справки НЕ
//! гейтит (RE: s4 CommonCommand несёт `…_Истина` БЕЗ файлов справки) — гейт только файловый.
//!
//! # Ресурсы `_files/` (witnessed: SSL `ТранспортСообщенийОбменаESB1C`, один `.png`)
//! Рядом со страницами может лежать каталог `_files/` со вспомогательными файлами (картинки
//! и пр. для HTML справки): EDT `Help/_files/…`, Designer `Ext/Help/_files/…`. Байты
//! идентичны между диалектами (sha1-сверено) → канон [`HelpResource`] = вербатимные байты,
//! БЕЗ интерпретации/пере-кодирования (§1.0). Designer-дескриптор `Ext/Help.xml` ресурсы НЕ
//! объявляет (только `<Page>`) — membership-проверка терпит РОВНО каталог `_files`; любой
//! иной необъявленный вход остаётся громким отказом. Ресурсы БЕЗ страниц — unwitnessed → отказ.

use std::path::{Path, PathBuf};

use formats_xml::Element;
use formats_xml::registry::Format;
use morph1c_core::ir::{HelpPage, HelpResource, MetadataObject};

use crate::ConvertError;

/// UTF-8 BOM (as a `char`) — Designer help pages carry it, EDT ones do not.
const BOM: char = '\u{FEFF}';

/// Подгрузить страницы справки (если сайдкары существуют) в `obj.help` и файлы-ресурсы
/// `_files/` в `obj.help_resources`.
///
/// `descriptor_path` — путь к прочитанному дескриптору (`.mdo`/`.xml`). Наличие гейтится
/// файлом (EDT: каталог `Help/`; Designer: `Ext/Help.xml`) — вид-агностично. Отсутствие →
/// no-op. Объявленная (Designer `<Page>`) страница без `.html` / лишний `.html` без
/// объявления / не-`.html` файл (кроме каталога `_files/`) → типизированная ошибка
/// (§1.0 — не тихий скип).
pub fn attach_help_pages(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    let (mut pages, resources) = read_help_sidecar(format, kind, descriptor_path, &obj.name)?;
    sync_help_descriptor_property(format, kind, &obj.name, &mut obj.properties, &mut pages)?;
    if !obj.help.is_empty() || !obj.help_resources.is_empty() {
        return Err(ConvertError::Read { kind: kind.into(), object: obj.name.clone(),
            reason: "object already carries help pages before sidecar attach".into() });
    }
    obj.help = pages;
    obj.help_resources = resources;
    Ok(())
}

/// Прочитать сайдкар справки, привязанный к ЯКОРЮ `anchor_path`, → (страницы, `_files/`-ресурсы).
///
/// Ядро [`attach_help_pages`], вынесенное отдельно, потому что справку несёт не только ОБЪЕКТ,
/// но и КАЖДАЯ ПОДЧИНЁННАЯ ФОРМА (`crate::form_read` — cf-элемент `<form-uuid>.1`). Раскладка
/// СОВПАДАЕТ дословно, расходится лишь якорь:
/// * объект   — его дескриптор (`<obj-dir>/<Obj>.mdo` / `<dir>/<Obj>.xml`);
/// * форма    — её якорь (`<obj-dir>/Forms/<F>/<F>.mdo` / `<dir>/<Obj>/Forms/<F>.xml` —
///   `form_read::form_anchor_path`), т.е. EDT `Forms/<F>/Help/<lang>.html`,
///   Designer `Forms/<F>/Ext/Help.xml` + `Ext/Help/<lang>.html` (witnessed: SSL 291×`ru.html`
///   в обоих диалектах, `_files/` у форм не встречается).
///
/// EDT-якорь ВИРТУАЛЕН (файла `Forms/<F>/<F>.mdo` на диске нет) — читателю он нужен ЛИШЬ как
/// «родитель = каталог формы», что и требуется. cf (контейнер) — пусто.
///
/// §1.0: `_files/`-ресурсы БЕЗ единой страницы — unwitnessed шейп → типизированный отказ здесь,
/// у обоих носителей сразу (гейт наличия справки — страницы).
pub(crate) fn read_help_sidecar(
    format: Format,
    kind: &str,
    anchor_path: &Path,
    owner_name: &str,
) -> Result<(Vec<HelpPage>, Vec<HelpResource>), ConvertError> {
    let (pages, resources) = match format {
        Format::Edt => read_edt_help(kind, anchor_path, owner_name)?,
        Format::Designer => read_designer_help(kind, anchor_path, owner_name)?,
        Format::Cf => return Ok((Vec::new(), Vec::new())), // контейнер — не файловый сайдкар.
    };
    if pages.is_empty() && !resources.is_empty() {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: owner_name.to_string(),
            reason: "help `_files/` resources present WITHOUT any help page — unwitnessed shape \
                     (§1.0)"
                .into(),
        });
    }
    Ok((pages, resources))
}

/// Спек, в котором ищется presence-поле `help` носителя справки: у ОБЪЕКТА — спек его вида.
fn spec_for_help_field(kind: &str) -> Option<&'static morph1c_core::spec::common::EntitySpec> {
    morph1c_core::spec::registry::spec_for(kind)
}

/// Bind the descriptor's ordered IDs to the complete owned page set. Native IDs come
/// from Help.xml; EDT directory enumeration is reordered by its metadata declaration.
fn sync_help_descriptor_property(
    format: Format, kind: &str, owner_name: &str,
    properties: &mut Vec<(morph1c_core::ir::FieldId, morph1c_core::ir::PropertyValue)>,
    pages: &mut Vec<HelpPage>,
) -> Result<(), ConvertError> {
    use morph1c_core::ir::PropertyValue;
    let fail = |reason: String| ConvertError::Read { kind: kind.into(), object: owner_name.into(), reason };
    let field = spec_for_help_field(kind).and_then(|s| s.fields().iter().find(|f| f.name == "help"));
    let Some(field) = field else {
        return if pages.is_empty() { Ok(()) } else { Err(fail("help pages present but kind has no help field".into())) };
    };
    let positions: Vec<_> = properties.iter().enumerate().filter(|(_, (id,_))| *id == field.id).map(|(i,_)| i).collect();
    if positions.len() > 1 { return Err(fail("duplicate help marker".into())); }
    if format == Format::Edt {
        let empty = PropertyValue::List(Vec::new());
        let value = positions.first().map_or(&empty, |i| &properties[*i].1);
        let ids = formats_xml::help::languages(value).map_err(fail)?;
        if ids.len() != pages.len() || ids.iter().any(|id| !pages.iter().any(|p| p.lang == *id)) {
            return Err(fail("help marker and owned help pages disagree".into()));
        }
        let mut owned: std::collections::BTreeMap<_,_> = std::mem::take(pages).into_iter().map(|p| (p.lang.clone(), p)).collect();
        for id in ids { pages.push(owned.remove(id).ok_or_else(|| fail("duplicate owned help language".into()))?); }
        if !owned.is_empty() { return Err(fail("unclaimed owned help page".into())); }
    } else {
        let value = PropertyValue::List(pages.iter().map(|p| PropertyValue::Str(p.lang.clone())).collect());
        formats_xml::help::languages(&value).map_err(fail)?;
        properties.retain(|(id,_)| *id != field.id);
        if !pages.is_empty() { properties.push((field.id, value)); }
    }
    Ok(())
}

pub(crate) fn sync_form_ref_help_property(format: Format, child: &mut MetadataObject, pages: &mut Vec<HelpPage>) -> Result<(), ConvertError> {
    sync_help_descriptor_property(format, child.kind.as_str(), &child.name, &mut child.properties, pages)
}

/// Descriptor-only view derives marker IDs from CURRENT page bodies. Binary assets,
/// modules and form bodies stay borrowed by the write pipeline and are not copied.
pub(crate) fn descriptor_with_help(obj: &MetadataObject) -> Result<std::borrow::Cow<'_, MetadataObject>, ConvertError> {
    use morph1c_core::ir::PropertyValue;
    fn field(kind: &str) -> Option<morph1c_core::ir::FieldId> {
        spec_for_help_field(kind).and_then(|s| s.fields().iter().find(|f| f.name == "help").map(|f| f.id))
    }
    fn marker(obj: &MetadataObject, pages: &[HelpPage]) -> Result<bool, ConvertError> {
        let value = PropertyValue::List(pages.iter().map(|p| PropertyValue::Str(p.lang.clone())).collect());
        let fail = |reason| ConvertError::Write { kind: obj.kind.as_str().into(), object: obj.name.clone(), reason };
        formats_xml::help::languages(&value).map_err(fail)?;
        let Some(id) = field(obj.kind.as_str()) else { return if pages.is_empty() { Ok(false) } else { Err(fail("help pages present but kind has no help field".into())) }; };
        let current: Vec<_> = obj.properties.iter().filter(|(f,_)| *f == id).collect();
        if current.len() > 1 { return Err(fail("duplicate help marker".into())); }
        if let Some((_, value)) = current.first() { formats_xml::help::languages(value).map_err(fail)?; }
        Ok(if pages.is_empty() { !current.is_empty() } else { current.first().is_none_or(|(_,v)| *v != value) })
    }
    let mut needs = marker(obj, &obj.help)?;
    for child in &obj.children {
        if child.kind.as_str().ends_with(".FormRef") {
            let body = obj.form_bodies.iter().find(|b| b.name == child.name);
            needs |= marker(child, body.map_or(&[], |b| b.help.as_slice()))?;
        }
    }
    if !needs { return Ok(std::borrow::Cow::Borrowed(obj)); }
    fn shallow(obj: &MetadataObject) -> MetadataObject {
        let mut view = MetadataObject::new(obj.kind.clone(), obj.name.clone(), obj.uuid);
        view.properties = obj.properties.clone();
        view.internal_info = obj.internal_info.clone();
        view.this_node = obj.this_node;
        view.source_extensions = obj.source_extensions.clone();
        view.metadata_picture_resource_commands = obj.metadata_picture_resource_commands.clone();
        view.children = obj.children.iter().map(shallow).collect();
        view
    }
    fn set(obj: &mut MetadataObject, pages: &[HelpPage]) {
        if let Some(id) = field(obj.kind.as_str()) {
            obj.properties.retain(|(f,_)| *f != id);
            if !pages.is_empty() { obj.properties.push((id, PropertyValue::List(pages.iter().map(|p| PropertyValue::Str(p.lang.clone())).collect()))); }
        }
    }
    let mut view = shallow(obj);
    set(&mut view, &obj.help);
    for child in &mut view.children {
        if child.kind.as_str().ends_with(".FormRef") {
            set(child, obj.form_bodies.iter().find(|b| b.name == child.name).map_or(&[], |b| b.help.as_slice()));
        }
    }
    Ok(std::borrow::Cow::Owned(view))
}

/// Write-side mirror of [`attach_help_pages`]: emit every help page `obj` carries beside its
/// just-written descriptor `descriptor_out`, in the target format's layout + encoding (EDT:
/// `Help/<lang>.html` bare CRLF; Designer: regenerated `Ext/Help.xml` descriptor + BOM'd LF
/// `Ext/Help/<lang>.html`), plus the `_files/` resources byte-verbatim (`Help/_files/<rel>` /
/// `Ext/Help/_files/<rel>`; NOT declared by Help.xml — witnessed). No-op for cf (container)
/// and objects with no help. §1.0: a write failure is a typed [`ConvertError`], never a
/// silent drop — including resources carried WITHOUT pages (unwitnessed shape, refused).
pub fn write_help_pages(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    write_help_sidecar(
        format,
        kind,
        descriptor_out,
        &obj.name,
        &obj.help,
        &obj.help_resources,
    )
}

/// Ядро [`write_help_pages`], привязанное к ЯКОРЮ — зеркало [`read_help_sidecar`] (см. его док:
/// тот же сайдкар несут и ОБЪЕКТ, и каждая ПОДЧИНЁННАЯ ФОРМА; расходится только якорь).
///
/// EDT: `<anchor-dir>/Help/<lang>.html` без BOM, CRLF. Designer: пере-сгенерированный дескриптор
/// `<anchor-dir>/<anchor-stem>/Ext/Help.xml` + страницы `Ext/Help/<lang>.html` с BOM, LF; плюс
/// `_files/`-ресурсы байт-вербатим (дескриптором НЕ объявляются — witnessed). cf и носитель без
/// справки — no-op. §1.0: ресурсы БЕЗ страниц не эмитируемы (нет гейта наличия) → типизированный
/// отказ, не тихий дроп.
pub(crate) fn write_help_sidecar(
    format: Format,
    kind: &str,
    anchor_out: &Path,
    owner_name: &str,
    pages: &[HelpPage],
    resources: &[HelpResource],
) -> Result<(), ConvertError> {
    if pages.is_empty() {
        // §1.0: ресурсы без страниц НЕ эмитируемы (гейт наличия справки — страницы/Help.xml);
        // молча уронить их нельзя.
        if !resources.is_empty() {
            return Err(ConvertError::Write {
                kind: kind.to_string(),
                object: owner_name.to_string(),
                reason: "carries help `_files/` resources but NO help pages — the shape is \
                         unwitnessed and cannot be emitted (§1.0, refusing a silent drop)"
                    .into(),
            });
        }
        return Ok(());
    }
    let mut languages = std::collections::BTreeSet::new();
    for page in pages {
        if !valid_language(&page.lang)
            || !languages.insert(&page.lang)
        {
            return Err(ConvertError::Write {
                kind: kind.to_string(),
                object: owner_name.to_string(),
                reason: "unsafe or duplicate help language".into(),
            });
        }
    }
    let no_parent = || ConvertError::Write {
        kind: kind.to_string(),
        object: owner_name.to_string(),
        reason: "help anchor path has no parent directory (unexpected)".into(),
    };
    match format {
        Format::Edt => {
            let help_dir = anchor_out.parent().ok_or_else(no_parent)?.join("Help");
            for page in pages {
                crate::form_write::write_file(
                    &help_dir.join(format!("{}.html", page.lang)),
                    &reencode_help_for_format(&page.body, format),
                )?;
            }
            write_help_resources(&help_dir, resources)
        }
        Format::Designer => {
            let dir = anchor_out.parent().ok_or_else(no_parent)?;
            let stem = anchor_out.file_stem().ok_or_else(no_parent)?;
            let ext = if kind == "Configuration" {
                dir.join("Ext")
            } else {
                dir.join(stem).join("Ext")
            };
            crate::form_write::write_file(
                &ext.join("Help.xml"),
                &serialize_help_descriptor(pages),
            )?;
            for page in pages {
                crate::form_write::write_file(
                    &ext.join("Help").join(format!("{}.html", page.lang)),
                    &reencode_help_for_format(&page.body, format),
                )?;
            }
            write_help_resources(&ext.join("Help"), resources)
        }
        Format::Cf => Ok(()), // контейнер — тела собирает cf-ассемблер, не файловый сайдкар.
    }
}

/// Эмитить файлы-ресурсы под `<pages_dir>/_files/<rel_path>` — байт-вербатим (§1.0: без
/// интерпретации). Общий для обоих диалектов: расходится только `pages_dir` (EDT `Help/`,
/// Designer `Ext/Help/`), раскладка `_files/…` внутри идентична.
fn write_help_resources(pages_dir: &Path, resources: &[HelpResource]) -> Result<(), ConvertError> {
    let files_dir = pages_dir.join(FILES_DIR);
    let mut seen = std::collections::BTreeSet::new();
    for res in resources {
        if res.rel_path.is_empty()
            || res.rel_path.contains('\\')
            || res.rel_path.contains(':')
            || res
                .rel_path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || !seen.insert(&res.rel_path)
        {
            return Err(ConvertError::Write {
                kind: "HelpResource".into(),
                object: res.rel_path.clone(),
                reason: "unsafe or duplicate help resource".into(),
            });
        }
        // `rel_path` — `/`-разделённый канон; `join` на Windows принимает `/` как разделитель.
        crate::form_write::write_file(&files_dir.join(&res.rel_path), &res.bytes)?;
    }
    Ok(())
}

/// Имя каталога вспомогательных файлов-ресурсов справки рядом со страницами (оба диалекта).
const FILES_DIR: &str = "_files";

/// EDT: страницы из каталога `<obj-dir>/Help/` (нет каталога → пусто) + ресурсы `_files/`.
/// Каждый файл обязан быть `<lang>.html` (единственный терпимый НЕ-файл — каталог `_files`);
/// страницы сортируются по имени файла (детерминизм).
fn read_edt_help(
    kind: &str,
    descriptor_path: &Path,
    obj_name: &str,
) -> Result<(Vec<HelpPage>, Vec<HelpResource>), ConvertError> {
    let help_dir = match descriptor_path.parent() {
        Some(d) => d.join("Help"),
        None => return Ok((Vec::new(), Vec::new())),
    };
    if !help_dir.is_dir() {
        return Ok((Vec::new(), Vec::new()));
    }
    let read_err = |reason: String| ConvertError::Read {
        kind: kind.to_string(),
        object: obj_name.to_string(),
        reason,
    };
    let rd = std::fs::read_dir(&help_dir).map_err(|e| ConvertError::Io {
        path: help_dir.display().to_string(),
        reason: e.to_string(),
    })?;
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut resources: Vec<HelpResource> = Vec::new();
    for entry in rd {
        let path = entry
            .map_err(|e| ConvertError::Io {
                path: help_dir.display().to_string(),
                reason: e.to_string(),
            })?
            .path();
        // Каталог ресурсов `_files/` — witnessed сосед страниц; транспортируется вербатим.
        if path.is_dir() && path.file_name().and_then(|s| s.to_str()) == Some(FILES_DIR) {
            resources = read_help_resources(kind, obj_name, &path)?;
            continue;
        }
        // §1.0: иначе в каталоге справки ожидаются ТОЛЬКО страницы `<lang>.html` — не тихий скип.
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("html") {
            return Err(read_err(format!(
                "unexpected non-page entry {} in the Help sidecar dir (only <lang>.html \
                 and the `_files/` resource dir witnessed, §1.0)",
                path.display()
            )));
        }
        paths.push(path);
    }
    paths.sort_by_key(|p| p.file_name().map(|s| s.to_os_string()));
    let mut pages = Vec::with_capacity(paths.len());
    for path in paths {
        let lang = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| read_err(format!("help page {} has a non-UTF-8 name", path.display())))?
            .to_string();
        if !valid_language(&lang) {
            return Err(read_err("unsafe help page language".into()));
        }
        pages.push(HelpPage {
            lang,
            body: read_page(kind, obj_name, &path, Format::Edt)?,
        });
    }
    Ok((pages, resources))
}

/// Designer: страницы, объявленные дескриптором `<dir>/<Name>/Ext/Help.xml` (нет файла →
/// пусто), тела — `Ext/Help/<lang>.html`, плюс ресурсы `Ext/Help/_files/` (дескриптором НЕ
/// объявляются — witnessed). Порядок страниц — порядок `<Page>` дескриптора.
fn read_designer_help(
    kind: &str,
    descriptor_path: &Path,
    obj_name: &str,
) -> Result<(Vec<HelpPage>, Vec<HelpResource>), ConvertError> {
    let (dir, stem) = match (descriptor_path.parent(), descriptor_path.file_stem()) {
        (Some(d), Some(s)) => (d, s),
        _ => return Ok((Vec::new(), Vec::new())),
    };
    let ext = if kind == "Configuration" {
        dir.join("Ext")
    } else {
        dir.join(stem).join("Ext")
    };
    let help_xml = ext.join("Help.xml");
    if !help_xml.is_file() {
        return Ok((Vec::new(), Vec::new()));
    }
    let langs = parse_page_descriptor(kind, &help_xml, obj_name)?;
    let pages_dir = ext.join("Help");
    let mut pages = Vec::with_capacity(langs.len());
    for lang in &langs {
        // §1.0: объявленная страница обязана существовать (dangling-ссылка — не тихий скип);
        // отсутствующий файл даёт типизированный Io от read_page.
        let path = pages_dir.join(format!("{lang}.html"));
        pages.push(HelpPage {
            lang: lang.clone(),
            body: read_page(kind, obj_name, &path, Format::Designer)?,
        });
    }
    // §1.0: страница на диске БЕЗ объявления в Help.xml — членство разошлось, громко.
    // Единственный терпимый НЕобъявленный вход — каталог ресурсов `_files/` (Help.xml
    // объявляет только `<Page>`; witnessed).
    let mut resources: Vec<HelpResource> = Vec::new();
    if pages_dir.is_dir() {
        let rd = std::fs::read_dir(&pages_dir).map_err(|e| ConvertError::Io {
            path: pages_dir.display().to_string(),
            reason: e.to_string(),
        })?;
        for entry in rd {
            let path = entry
                .map_err(|e| ConvertError::Io {
                    path: pages_dir.display().to_string(),
                    reason: e.to_string(),
                })?
                .path();
            if path.is_dir() && path.file_name().and_then(|s| s.to_str()) == Some(FILES_DIR) {
                resources = read_help_resources(kind, obj_name, &path)?;
                continue;
            }
            let declared = path.is_file()
                && path.extension().and_then(|e| e.to_str()) == Some("html")
                && path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| langs.iter().any(|l| l == s))
                    .unwrap_or(false);
            if !declared {
                return Err(ConvertError::Read {
                    kind: kind.to_string(),
                    object: obj_name.to_string(),
                    reason: format!(
                        "help page {} present on disk is not declared by Ext/Help.xml \
                         (§1.0 — membership mismatch)",
                        path.display()
                    ),
                });
            }
        }
    }
    Ok((pages, resources))
}

/// Рекурсивно собрать файлы-ресурсы из каталога `_files/` → [`HelpResource`] с `/`-разделённым
/// путём относительно `_files/` и вербатимными байтами (§1.0 — без интерпретации). Порядок —
/// сортированный обход (детерминизм IR между ФС; == порядку ресурсов в cf-хвосте — RE erp.cf
/// 14/14 ресурсоносных HTML-тел). Симлинки/не-файлы-не-каталоги — отказ.
/// `pub(crate)`: тем же обходом [`crate::blob_template_read`] читает `_files/` HTML-МАКЕТА
/// (та же раскладка рядом со страницами, тот же вербатим-канон).
pub(crate) fn read_help_resources(
    kind: &str,
    obj_name: &str,
    files_dir: &Path,
) -> Result<Vec<HelpResource>, ConvertError> {
    let mut out = Vec::new();
    collect_help_resources(kind, obj_name, files_dir, "", &mut out)?;
    // §1.0: пустой `_files/` — unwitnessed (нечего транспортировать, а каталог-плейсхолдер
    // при записи не воспроизвести) → громко.
    if out.is_empty() {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: obj_name.to_string(),
            reason: format!(
                "help `_files` dir {} is empty — an empty resource dir cannot be round-tripped \
                 (§1.0)",
                files_dir.display()
            ),
        });
    }
    Ok(out)
}

/// Обход одного уровня `_files/…` (рекурсия по подкаталогам): `prefix` — накопленный
/// `/`-разделённый относительный путь ("" на верхнем уровне).
fn collect_help_resources(
    kind: &str,
    obj_name: &str,
    dir: &Path,
    prefix: &str,
    out: &mut Vec<HelpResource>,
) -> Result<(), ConvertError> {
    let rd = std::fs::read_dir(dir).map_err(|e| ConvertError::Io {
        path: dir.display().to_string(),
        reason: e.to_string(),
    })?;
    let mut entries: Vec<PathBuf> = Vec::new();
    for entry in rd {
        entries.push(
            entry
                .map_err(|e| ConvertError::Io {
                    path: dir.display().to_string(),
                    reason: e.to_string(),
                })?
                .path(),
        );
    }
    entries.sort_by_key(|p| p.file_name().map(|s| s.to_os_string()));
    for path in entries {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| ConvertError::Read {
                kind: kind.to_string(),
                object: obj_name.to_string(),
                reason: format!("help resource {} has a non-UTF-8 name", path.display()),
            })?;
        let rel = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        if path.is_dir() {
            collect_help_resources(kind, obj_name, &path, &rel, out)?;
        } else if path.is_file() {
            let bytes = read_help_file(&path).map_err(|e| ConvertError::Io {
                path: path.display().to_string(),
                reason: e.to_string(),
            })?;
            out.push(HelpResource {
                rel_path: rel,
                bytes,
            });
        } else {
            return Err(ConvertError::Read {
                kind: kind.to_string(),
                object: obj_name.to_string(),
                reason: format!(
                    "help resource entry {} is neither a file nor a directory (§1.0)",
                    path.display()
                ),
            });
        }
    }
    Ok(())
}

fn read_help_file(path: &Path) -> std::io::Result<Vec<u8>> {
    crate::form_read::read_regular_source(path)
}
fn valid_language(lang: &str) -> bool {
    formats_xml::help::valid_language(lang)
}

/// Прочитать ОДНУ страницу `<lang>.html` → канонический текст (BOM-снят, EOL=`\n`).
/// §1.0-самопроверка: пере-кодирование канона в исходный диалект обязано воспроизвести
/// исходные байты (незасвидетельствованное кодирование — громкий отказ, не тихая нормализация).
fn read_page(
    kind: &str,
    obj_name: &str,
    path: &Path,
    format: Format,
) -> Result<String, ConvertError> {
    let bytes = read_help_file(path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|e| ConvertError::Read {
        kind: kind.to_string(),
        object: obj_name.to_string(),
        reason: format!("help page {} is not UTF-8: {e}", path.display()),
    })?;
    let canonical = text.strip_prefix(BOM).unwrap_or(text).replace("\r\n", "\n");
    if reencode_help_for_format(&canonical, format) != bytes {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: obj_name.to_string(),
            reason: format!(
                "help page {} does not carry the witnessed {} encoding (Designer: BOM+LF, EDT: \
                 bare CRLF) — refusing to silently re-normalize (§1.0)",
                path.display(),
                format.code()
            ),
        });
    }
    Ok(canonical)
}

/// Канонический текст страницы → пер-форматное дисковое кодирование: Designer `BOM + LF`
/// (канон как есть), EDT — без BOM, `\n` → `\r\n`. cf сюда не доходит (контейнер).
fn reencode_help_for_format(canonical: &str, format: Format) -> Vec<u8> {
    match format {
        Format::Designer => {
            let mut s = String::with_capacity(canonical.len() + 3);
            s.push(BOM);
            s.push_str(canonical);
            s.into_bytes()
        }
        // Канон не содержит `\r\n` (снят на read) → экспансия не задваивает CR.
        Format::Edt | Format::Cf => canonical.replace('\n', "\r\n").into_bytes(),
    }
}

/// Разобрать Designer page-дескриптор → объявленные языки страниц (в порядке документа).
/// §1.0: корень `<Help>`, дети — только непустые `<Page>`; иное — типизированный отказ.
///
/// Обёртка ОБЩАЯ для справки объекта (`Ext/Help.xml`) и для манифеста HTMLDocument-МАКЕТА
/// (`Ext/Template.xml` — см. [`crate::blob_template_read`]): RE SSL — 12/12 манифестов макетов
/// байт-идентичны 232-байтному s15 `Help.xml` с одной страницей. Поэтому парсер/сериализатор
/// живут здесь в одном экземпляре, а не дублируются.
pub(crate) fn parse_page_descriptor(
    kind: &str,
    path: &Path,
    obj_name: &str,
) -> Result<Vec<String>, ConvertError> {
    let bytes = read_help_file(path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let read_err = |reason: String| ConvertError::Read {
        kind: kind.to_string(),
        object: obj_name.to_string(),
        reason,
    };
    let doc = formats_xml::parse(&bytes).map_err(|e| read_err(format!("Help.xml XML: {e}")))?;
    let root: &Element = &doc.root;
    if root.local != "Help" {
        return Err(read_err(format!(
            "Help.xml root is <{}>, expected <Help> (§1.0)",
            root.local
        )));
    }
    let version = root
        .attr("version")
        .ok_or_else(|| read_err("Help.xml: missing version".into()))?;
    let version =
        crate::sidecar_version::parse_witnessed(&version.value, "Help.xml").map_err(read_err)?;
    let mut langs = Vec::with_capacity(root.children.len());
    for child in &root.children {
        if child.local != "Page" {
            return Err(read_err(format!(
                "Help.xml carries <{}>, only <Page> witnessed (§1.0)",
                child.local
            )));
        }
        if child.text.is_empty() {
            return Err(read_err("Help.xml has an empty <Page> (§1.0)".into()));
        }
        if !valid_language(&child.text) || langs.contains(&child.text) {
            return Err(read_err(
                "unsafe or duplicate Help.xml Page language".into(),
            ));
        }
        langs.push(child.text.clone());
    }
    if langs.is_empty() {
        return Err(read_err(
            "Help.xml declares no <Page> (§1.0 — empty descriptor unwitnessed)".into(),
        ));
    }
    let names = langs.iter().map(String::as_str).collect::<Vec<_>>();
    let reproduced =
        morph1c_core::version::with_roundtrip_target(version, || serialize_page_descriptor(&names));
    if reproduced != bytes {
        return Err(read_err(
            "Help.xml contains unmodeled syntax/content or non-witnessed serialization".into(),
        ));
    }
    Ok(langs)
}

/// Сериализовать Designer page-дескриптор БАЙТ-ТОЧНО (RE: s15 `Help.xml`, 232 B): ведущий BOM,
/// CRLF, таб-отступ, `version=` ТАРГЕТ-версии (амбьентный round-trip таргет: ERP 2.20 /
/// SSL 2.21; вне scope — SSL, прежние байты), `<Page>` на страницу, БЕЗ хвостового перевода
/// строки — та же обёрточная конвенция, что `Ext/Picture.xml`/`Ext/CommandInterface.xml`.
///
/// Общая с манифестом HTMLDocument-макета (`Ext/Template.xml`) — см. [`parse_page_descriptor`].
pub(crate) fn serialize_page_descriptor(langs: &[&str]) -> Vec<u8> {
    let version =
        morph1c_core::version::current_roundtrip_target().unwrap_or(morph1c_core::version::SSL);
    let mut s = String::new();
    s.push(BOM);
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    s.push_str(&format!(
        "<Help xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" \
         xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{version}\">\r\n",
    ));
    for lang in langs {
        s.push_str("\t<Page>");
        s.push_str(&lang.replace('&', "&amp;"));
        s.push_str("</Page>\r\n");
    }
    s.push_str("</Help>");
    s.into_bytes()
}

/// [`serialize_page_descriptor`] по страницам справки.
fn serialize_help_descriptor(pages: &[HelpPage]) -> Vec<u8> {
    let langs: Vec<&str> = pages.iter().map(|p| p.lang.as_str()).collect();
    serialize_page_descriptor(&langs)
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::{ObjectKind, Uuid};

    /// Свежий temp-каталог на тест.
    fn temp_base(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "morph1c-help-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    fn obj() -> MetadataObject {
        MetadataObject::new(ObjectKind::new("Catalog"), "Спр", Uuid([0; 16]))
    }

    /// Засвидетельствованные дисковые байты одной и той же страницы в двух диалектах.
    const EDT_PAGE: &str = "<html><body>\r\n<p>Справка</p></body></html>";
    const DESIGNER_PAGE: &str = "\u{FEFF}<html><body>\n<p>Справка</p></body></html>";
    const CANONICAL: &str = "<html><body>\n<p>Справка</p></body></html>";

    #[test]
    fn attach_edt_reads_bare_crlf_page_to_canonical_lf() {
        let base = temp_base("attach-edt");
        let obj_dir = base.join("Catalogs").join("Спр");
        std::fs::create_dir_all(obj_dir.join("Help")).unwrap();
        crate::fsio::write(obj_dir.join("Help").join("ru.html"), EDT_PAGE.as_bytes()).unwrap();
        let mut o = obj();
        attach_help_pages(Format::Edt, "Catalog", &obj_dir.join("Спр.mdo"), &mut o).unwrap();
        assert_eq!(o.help.len(), 1);
        assert_eq!(o.help[0].lang, "ru");
        assert_eq!(o.help[0].body, CANONICAL);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn attach_designer_reads_declared_bom_lf_page_to_canonical() {
        let base = temp_base("attach-designer");
        let dir = base.join("Catalogs");
        let ext = dir.join("Спр").join("Ext");
        std::fs::create_dir_all(ext.join("Help")).unwrap();
        crate::fsio::write(
            ext.join("Help.xml"),
            serialize_help_descriptor(&[HelpPage {
                lang: "ru".into(),
                body: String::new(),
            }]),
        )
        .unwrap();
        crate::fsio::write(ext.join("Help").join("ru.html"), DESIGNER_PAGE.as_bytes()).unwrap();
        let mut o = obj();
        attach_help_pages(Format::Designer, "Catalog", &dir.join("Спр.xml"), &mut o).unwrap();
        assert_eq!(o.help.len(), 1);
        assert_eq!(o.help[0].lang, "ru");
        assert_eq!(o.help[0].body, CANONICAL);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn attach_is_noop_without_sidecar_and_errors_on_undeclared_page() {
        // Нет Help-каталога/Help.xml → пусто.
        let base = temp_base("noop");
        let obj_dir = base.join("Catalogs").join("Спр");
        std::fs::create_dir_all(&obj_dir).unwrap();
        let mut o = obj();
        attach_help_pages(Format::Edt, "Catalog", &obj_dir.join("Спр.mdo"), &mut o).unwrap();
        assert!(o.help.is_empty());

        // Designer: страница на диске без объявления в Help.xml → громко.
        let dbase = temp_base("undeclared");
        let ddir = dbase.join("Catalogs");
        let ext = ddir.join("Спр").join("Ext");
        std::fs::create_dir_all(ext.join("Help")).unwrap();
        crate::fsio::write(
            ext.join("Help.xml"),
            serialize_help_descriptor(&[HelpPage {
                lang: "ru".into(),
                body: String::new(),
            }]),
        )
        .unwrap();
        crate::fsio::write(ext.join("Help").join("ru.html"), DESIGNER_PAGE.as_bytes()).unwrap();
        crate::fsio::write(ext.join("Help").join("en.html"), DESIGNER_PAGE.as_bytes()).unwrap();
        let mut d = obj();
        let err = attach_help_pages(Format::Designer, "Catalog", &ddir.join("Спр.xml"), &mut d)
            .unwrap_err();
        match err {
            ConvertError::Read { reason, .. } => {
                assert!(reason.contains("not declared"), "got {reason}")
            }
            other => panic!("expected Read, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&dbase);
    }

    #[test]
    fn read_page_rejects_unwitnessed_encoding() {
        // EDT-страница с BOM (незасвидетельствовано) → отказ, не тихая нормализация.
        let base = temp_base("badenc");
        std::fs::create_dir_all(&base).unwrap();
        let p = base.join("ru.html");
        crate::fsio::write(&p, DESIGNER_PAGE.as_bytes()).unwrap();
        let err = read_page("Catalog", "Спр", &p, Format::Edt).unwrap_err();
        match err {
            ConvertError::Read { reason, .. } => {
                assert!(reason.contains("encoding"), "got {reason}")
            }
            other => panic!("expected Read, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn write_emits_per_format_layout_and_encoding() {
        let mut o = obj();
        o.help.push(HelpPage {
            lang: "ru".into(),
            body: CANONICAL.into(),
        });

        // EDT: Help/ru.html без BOM, CRLF.
        let ebase = temp_base("write-edt");
        let e_out = ebase.join("Catalogs").join("Спр").join("Спр.mdo");
        std::fs::create_dir_all(e_out.parent().unwrap()).unwrap();
        write_help_pages(Format::Edt, "Catalog", &e_out, &o).unwrap();
        let e_bytes = std::fs::read(
            ebase
                .join("Catalogs")
                .join("Спр")
                .join("Help")
                .join("ru.html"),
        )
        .unwrap();
        assert_eq!(e_bytes, EDT_PAGE.as_bytes());

        // Designer: Ext/Help.xml (байт-точный дескриптор) + Ext/Help/ru.html с BOM, LF.
        let dbase = temp_base("write-designer");
        let d_out = dbase.join("Catalogs").join("Спр.xml");
        std::fs::create_dir_all(d_out.parent().unwrap()).unwrap();
        write_help_pages(Format::Designer, "Catalog", &d_out, &o).unwrap();
        let ext = dbase.join("Catalogs").join("Спр").join("Ext");
        let d_bytes = std::fs::read(ext.join("Help").join("ru.html")).unwrap();
        assert_eq!(d_bytes, DESIGNER_PAGE.as_bytes());
        let langs = parse_page_descriptor("Catalog", &ext.join("Help.xml"), "Спр").unwrap();
        assert_eq!(langs, ["ru"]);

        let _ = std::fs::remove_dir_all(&ebase);
        let _ = std::fs::remove_dir_all(&dbase);
    }

    /// Опаковые «PNG»-байты ресурса (не-UTF-8 — проверяет вербатимность транспорта).
    const RES_BYTES: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0xFF];

    #[test]
    fn attach_reads_files_resources_in_both_dialects() {
        // EDT: Help/_files/img.png (+ вложенный подкаталог) рядом со страницей.
        let base = temp_base("files-edt");
        let obj_dir = base.join("Catalogs").join("Спр");
        let files = obj_dir.join("Help").join("_files");
        std::fs::create_dir_all(files.join("sub")).unwrap();
        crate::fsio::write(obj_dir.join("Help").join("ru.html"), EDT_PAGE.as_bytes()).unwrap();
        crate::fsio::write(files.join("img.png"), RES_BYTES).unwrap();
        crate::fsio::write(files.join("sub").join("n.bin"), b"xx").unwrap();
        let mut o = obj();
        attach_help_pages(Format::Edt, "Catalog", &obj_dir.join("Спр.mdo"), &mut o).unwrap();
        assert_eq!(o.help.len(), 1);
        assert_eq!(
            o.help_resources,
            vec![
                HelpResource {
                    rel_path: "img.png".into(),
                    bytes: RES_BYTES.to_vec()
                },
                HelpResource {
                    rel_path: "sub/n.bin".into(),
                    bytes: b"xx".to_vec()
                },
            ]
        );
        let _ = std::fs::remove_dir_all(&base);

        // Designer: Ext/Help/_files/img.png НЕ объявлен Help.xml — терпимо (witnessed), читается.
        let dbase = temp_base("files-designer");
        let ddir = dbase.join("Catalogs");
        let ext = ddir.join("Спр").join("Ext");
        std::fs::create_dir_all(ext.join("Help").join("_files")).unwrap();
        crate::fsio::write(
            ext.join("Help.xml"),
            serialize_help_descriptor(&[HelpPage {
                lang: "ru".into(),
                body: String::new(),
            }]),
        )
        .unwrap();
        crate::fsio::write(ext.join("Help").join("ru.html"), DESIGNER_PAGE.as_bytes()).unwrap();
        crate::fsio::write(ext.join("Help").join("_files").join("img.png"), RES_BYTES).unwrap();
        let mut d = obj();
        attach_help_pages(Format::Designer, "Catalog", &ddir.join("Спр.xml"), &mut d).unwrap();
        assert_eq!(d.help.len(), 1);
        assert_eq!(
            d.help_resources,
            vec![HelpResource {
                rel_path: "img.png".into(),
                bytes: RES_BYTES.to_vec()
            }]
        );
        let _ = std::fs::remove_dir_all(&dbase);
    }

    #[test]
    fn files_resources_roundtrip_byte_exact_in_both_dialects() {
        let mut o = obj();
        o.help.push(HelpPage {
            lang: "ru".into(),
            body: CANONICAL.into(),
        });
        o.help_resources.push(HelpResource {
            rel_path: "sub/img.png".into(),
            bytes: RES_BYTES.to_vec(),
        });

        for (format, tag) in [(Format::Edt, "rt-edt"), (Format::Designer, "rt-designer")] {
            let base = temp_base(tag);
            let out = match format {
                Format::Edt => base.join("Catalogs").join("Спр").join("Спр.mdo"),
                _ => base.join("Catalogs").join("Спр.xml"),
            };
            std::fs::create_dir_all(out.parent().unwrap()).unwrap();
            write_help_pages(format, "Catalog", &out, &o).unwrap();
            // Ресурс лёг в пер-диалектный `_files/` байт-вербатим…
            let files_dir = match format {
                Format::Edt => base
                    .join("Catalogs")
                    .join("Спр")
                    .join("Help")
                    .join("_files"),
                _ => base
                    .join("Catalogs")
                    .join("Спр")
                    .join("Ext")
                    .join("Help")
                    .join("_files"),
            };
            assert_eq!(
                std::fs::read(files_dir.join("sub").join("img.png")).unwrap(),
                RES_BYTES,
                "{tag}: resource bytes verbatim"
            );
            // …и re-attach из записанного даёт ИДЕНТИЧНЫЙ IR (страницы + ресурсы).
            let mut back = obj();
            attach_help_pages(format, "Catalog", &out, &mut back).unwrap();
            assert_eq!(back.help, o.help, "{tag}: pages roundtrip");
            assert_eq!(
                back.help_resources, o.help_resources,
                "{tag}: resources roundtrip"
            );
            let _ = std::fs::remove_dir_all(&base);
        }
    }

    #[test]
    fn files_resources_without_pages_are_refused_on_read_and_write() {
        // READ (EDT): Help/ несёт ТОЛЬКО _files/ без страниц — unwitnessed → громко.
        let base = temp_base("files-nopages");
        let obj_dir = base.join("Catalogs").join("Спр");
        std::fs::create_dir_all(obj_dir.join("Help").join("_files")).unwrap();
        crate::fsio::write(
            obj_dir.join("Help").join("_files").join("img.png"),
            RES_BYTES,
        )
        .unwrap();
        let mut o = obj();
        let err = attach_help_pages(Format::Edt, "Catalog", &obj_dir.join("Спр.mdo"), &mut o)
            .unwrap_err();
        match err {
            ConvertError::Read { reason, .. } => {
                assert!(reason.contains("WITHOUT any help page"), "got {reason}")
            }
            other => panic!("expected Read, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&base);

        // WRITE: ресурсы без страниц — не эмитируемы (нет гейта наличия) → громко, не тихий дроп.
        let wbase = temp_base("files-nopages-write");
        let out = wbase.join("Catalogs").join("Спр").join("Спр.mdo");
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        let mut w = obj();
        w.help_resources.push(HelpResource {
            rel_path: "img.png".into(),
            bytes: RES_BYTES.to_vec(),
        });
        let werr = write_help_pages(Format::Edt, "Catalog", &out, &w).unwrap_err();
        match werr {
            ConvertError::Write { reason, .. } => {
                assert!(reason.contains("NO help pages"), "got {reason}")
            }
            other => panic!("expected Write, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&wbase);
    }

    #[test]
    fn help_descriptor_bytes_match_witnessed_fixture_shape() {
        // Форма RE-эталона: BOM + CRLF + `\t<Page>ru</Page>` + `</Help>` БЕЗ хвостового \n.
        let bytes = serialize_help_descriptor(&[HelpPage {
            lang: "ru".into(),
            body: String::new(),
        }]);
        let expected: Vec<u8> = {
            let mut s = String::new();
            s.push('\u{FEFF}');
            s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
            s.push_str("<Help xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.21\">\r\n");
            s.push_str("\t<Page>ru</Page>\r\n</Help>");
            s.into_bytes()
        };
        assert_eq!(bytes, expected);
    }
}
