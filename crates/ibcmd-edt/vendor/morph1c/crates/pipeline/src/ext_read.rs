//! Attach / emit the CONFIG-LEVEL `Ext` sidecars of the root `Configuration` object
//! (§1.0/§1.6): the four application modules (`ManagedApplicationModule` /
//! `OrdinaryApplicationModule` / `SessionModule` / `ExternalConnectionModule`), the two
//! root pictures (`Splash` / `MainSectionPicture`), the two VERBATIM binary sidecars
//! (`ParentConfigurations` / `MobileClientSignature`), the STRUCTURED standalone-content
//! sidecar (`StandaloneConfigurationContent.bin` / `MobileApplicationContent.scc` —
//! delegated to [`crate::standalone_content_read`]) and the FOUR config-level interface
//! sidecars (`CommandInterface` / `MainSectionCommandInterface` / `HomePageWorkArea` /
//! `ClientApplicationInterface` — delegated to [`crate::config_interface_read`], cf bodies
//! `<host>.{a,9,8,b}`). Sibling of [`crate::module_read`] /
//! [`crate::picture_read`] — "the descriptor read is metadata-only, the BODY is a sibling
//! file the pipeline attaches" — applied to the ROOT object, whose layout diverges from
//! every per-object kind (root-level `Ext/`, not `<Name>/Ext/`).
//!
//! # Layout beside the root descriptor (RE: coverage/s15_subordinate, both dialects)
//! * **EDT** (root `src/Configuration/Configuration.mdo`): the sidecars are SIBLINGS of the
//!   descriptor — modules `src/Configuration/<Slot>.bsl` (no BOM, CRLF), pictures
//!   `src/Configuration/<Slot>.<ext>` (raw image, name = slot).
//! * **Designer** (root `<root>/Configuration.xml`): a root-level `Ext/` dir —
//!   modules `Ext/<Slot>.bsl` (UTF-8 BOM + CRLF), pictures `Ext/<Slot>.xml` (the SAME
//!   `<ExtPicture>` wrapper as CommonPicture — `<xr:Abs>Picture.<ext></xr:Abs>`,
//!   `LoadTransparent=false`) plus the raw image `Ext/<Slot>/Picture.<ext>`.
//!
//! Module sources equal modulo the BOM ([`crate::module_read::reencode_module_for_format`]
//! re-encodes per format); the raw image bytes are IDENTICAL across formats (§1.6, verified
//! byte-equal on s15). The root DESCRIPTOR carries no reference to any of these — presence
//! is gated purely by the files on disk, absence is honest. EDT marks a present picture
//! with an EMPTY descriptor node (`<splash/>`/`<mainSectionPicture/>`, witnessed: s15 has
//! both nodes + files, s1..s14 have neither); Designer has no descriptor node at all, so
//! [`attach_config_ext`] SYNTHESIZES the present-empty property from the attached file on
//! Designer reads — a cross-write to EDT then regenerates the node exactly when the source
//! actually carries the picture.
//!
//! # VERBATIM binary sidecars (ParentConfigurations / MobileClientSignature)
//! Два бинарных конфиг-сайдкара носятся ДОСЛОВНО (это не модули: BOM/CRLF не трогаются,
//! никакой перекодировки — [`morph1c_core::ir::ConfigBlob`]):
//! * **`ParentConfigurations`** — designer `Ext/ParentConfigurations.bin`, edt
//!   `ParentConfigurations.bin` (сиблинг `.mdo`). Витнессы byte-identical: ERP 16 Б
//!   (`{6,0,0,0,1,0}`) == erp.cf `<host>.4`; SSL 619 225 Б designer == edt == ssl.cf `.4`.
//! * **`MobileClientSignature`** — designer `Ext/MobileClientSignature.bin`, edt
//!   ⚠️ ДРУГОЕ имя `MobileClientSign.bin`. Витнесс: ERP 560 886 Б (brace
//!   `{2,"MIIBtjCC…"}`), designer == edt (сверено sha256) == erp.cf `<host>.10`.
//!   У SSL файла нет ни в одном диалекте (и в ssl.cf нет `.10`) — отсутствие честно.
//!
//! cf-СТОРОНА: запись — `formats_cf::assemble` (`CONFIG_BLOB_SLOTS`, verbatim-лист
//! `<host>.N`); ЧТЕНИЕ cf → IR корневые Ext-тела НЕ подхватывает (cf-декомпиляция
//! неполна — модули/картинки корня из cf так же не читаются; здесь честно то же).

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::{ConfigBlob, ConfigPicture, MetadataObject, Module};

use crate::ConvertError;

/// The root object's kind (the only kind this pass touches).
const CONFIGURATION_KIND: &str = "Configuration";

/// Ordered application-module slots of the root (witnessed on s15; the order is the
/// deterministic IR order, mirroring [`crate::module_read`]'s per-kind slot tables).
const APP_MODULE_SLOTS: &[&str] = &[
    "ManagedApplicationModule",
    "OrdinaryApplicationModule",
    "SessionModule",
    "ExternalConnectionModule",
];

/// Ordered root-picture slots (witnessed on s15).
const CONFIG_PICTURE_SLOTS: &[&str] = &["MainSectionPicture", "Splash"];

/// `(IR-слот, designer-имя файла, edt-имя файла)` бинарных VERBATIM-сайдкаров корня —
/// детерминированный порядок IR. ⚠️ EDT-имя `MobileClientSignature` — ДРУГОЕ
/// (`MobileClientSign.bin`, сверено: байты идентичны designer-файлу, sha256 ERP).
const CONFIG_BLOB_SLOTS: &[(&str, &str, &str)] = &[
    (
        "ParentConfigurations",
        "ParentConfigurations.bin",
        "ParentConfigurations.bin",
    ),
    (
        "MobileClientSignature",
        "MobileClientSignature.bin",
        "MobileClientSign.bin",
    ),
];

/// Пер-форматное имя файла VERBATIM-сайдкара по записи [`CONFIG_BLOB_SLOTS`]. cf сюда не
/// доходит ([`ext_dir`] возвращает `None`).
fn blob_file_name(
    format: Format,
    entry: &(&'static str, &'static str, &'static str),
) -> &'static str {
    match format {
        Format::Designer => entry.1,
        Format::Edt => entry.2,
        Format::Cf => unreachable!("cf has no Ext dir (ext_dir returned None)"),
    }
}

/// The Designer raw-image stem inside `Ext/<Slot>/` (the wrapper's `<xr:Abs>` file name is
/// `Picture.<ext>` — same convention as CommonPicture).
const PICTURE_STEM: &str = "Picture";

/// The config-level Ext directory beside the root descriptor, per format. `None` for cf
/// (container — no file sidecars).
///
/// * EDT: the sidecars are siblings of the descriptor → the descriptor's own dir.
/// * Designer: `<descriptor-dir>/Ext` (root-level `Ext/`, NOT `Configuration/Ext`).
fn ext_dir(format: Format, descriptor_path: &Path) -> Option<PathBuf> {
    let dir = descriptor_path.parent()?;
    match format {
        Format::Edt => Some(dir.to_path_buf()),
        Format::Designer => Some(dir.join("Ext")),
        Format::Cf => None,
    }
}

/// Load the root's config-level Ext sidecars (app modules → `obj.modules`, pictures →
/// `obj.config_pictures`). No-op for non-root objects and cf. §1.0: a missing file means
/// the module/picture honestly does not exist; a present-but-unreadable file is a typed
/// [`ConvertError`]; a malformed Designer picture wrapper errors (never guessed).
pub fn attach_config_ext(
    format: Format,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if obj.kind.as_str() != CONFIGURATION_KIND {
        return Ok(());
    }
    let dir = match ext_dir(format, descriptor_path) {
        Some(d) => d,
        None => return Ok(()), // cf: container — no file sidecars.
    };

    // (1) Application modules — `<Slot>.bsl` in the Ext dir, deterministic slot order.
    for &slot in APP_MODULE_SLOTS {
        let path = dir.join(format!("{slot}.bsl"));
        if !path.is_file() {
            continue; // no file → no module in the IR (honest).
        }
        // Config-level application modules are text sidecars (no protected app module is
        // witnessed in the corpus; a non-UTF-8 one would surface as a typed read error).
        let source = std::fs::read_to_string(&path).map_err(|e| ConvertError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        obj.modules.push(Module::text(slot, source));
    }

    // (2) Root pictures — Splash / MainSectionPicture, deterministic slot order.
    for &slot in CONFIG_PICTURE_SLOTS {
        if let Some(pic) = read_picture(format, &dir, slot)? {
            obj.config_pictures.push(pic);
            // Designer's root descriptor carries NO picture node at all, while EDT marks a
            // present picture with a present-empty node (`<mainSectionPicture/>`/`<splash/>`,
            // witnessed: s15 both / s1..s14 neither). Synthesize the present-empty property
            // for Designer reads so a cross-write to EDT regenerates the node exactly when
            // the picture exists; EDT reads already carry it from the descriptor itself.
            if format == Format::Designer {
                inject_present_empty_picture_prop(obj, slot);
            }
        }
    }

    // (3) VERBATIM binary sidecars — ParentConfigurations / MobileClientSignature,
    // детерминированный порядок таблицы. Байты — ДОСЛОВНО (никакой перекодировки:
    // BOM/CRLF не трогаются — это не модуль). Файл отсутствует → честно нет (no-op).
    for entry in CONFIG_BLOB_SLOTS {
        let path = dir.join(blob_file_name(format, entry));
        if !path.is_file() {
            continue; // no file → no blob in the IR (honest; SSL carries no signature).
        }
        obj.config_blobs.push(ConfigBlob {
            slot: entry.0.to_string(),
            bytes: read_bytes(&path)?,
        });
    }

    // (4) СТРУКТУРНЫЙ сайдкар состава автономной конфигурации (designer
    // `StandaloneConfigurationContent.bin` / edt `MobileApplicationContent.scc`) —
    // XML-парс + §1.0-самопроверка в [`crate::standalone_content_read`]; отсутствие
    // файла — честное отсутствие (SSL/coverage).
    crate::standalone_content_read::attach_standalone_content(format, &dir, obj)?;

    // (5) ЧЕТЫРЕ конфиг-уровневых ИНТЕРФЕЙС-сайдкара (CommandInterface /
    // MainSectionCommandInterface / HomePageWorkArea / ClientApplicationInterface) —
    // строгий парс + §1.0-самопроверка в [`crate::config_interface_read`]; отсутствие
    // каждого файла — честное отсутствие (SSL/coverage не несут ни одного).
    crate::config_interface_read::attach_config_interfaces(format, &dir, obj)?;
    Ok(())
}

/// Put the present-empty (`Str("")`) root-picture property for `slot` into `obj.properties`
/// at its CANONICAL spec position (the bag from `engine::read` is in spec order). No-op if
/// the bag already carries the field (EDT source: the descriptor node put it there).
fn inject_present_empty_picture_prop(obj: &mut MetadataObject, slot: &str) {
    use morph1c_core::ir::value::PropertyValue;
    use morph1c_core::spec::metadata::configuration as cfg;
    let field = match slot {
        "MainSectionPicture" => cfg::F_MAIN_SECTION_PICTURE,
        "Splash" => cfg::F_SPLASH,
        _ => unreachable!("CONFIG_PICTURE_SLOTS is exhaustive"),
    };
    if obj.properties.iter().any(|(id, _)| *id == field) {
        return;
    }
    let spec = cfg::configuration();
    let rank = |id: morph1c_core::ir::FieldId| {
        spec.fields()
            .iter()
            .position(|f| f.id == id)
            .unwrap_or(usize::MAX)
    };
    let target = rank(field);
    let pos = obj
        .properties
        .iter()
        .position(|(id, _)| rank(*id) > target)
        .unwrap_or(obj.properties.len());
    obj.properties
        .insert(pos, (field, PropertyValue::Str(String::new())));
}

/// Read ONE root picture slot from the Ext dir, per format. `None` = the slot honestly has
/// no picture (no EDT `<Slot>.*` sibling / no Designer `Ext/<Slot>.xml` wrapper).
fn read_picture(
    format: Format,
    dir: &Path,
    slot: &str,
) -> Result<Option<ConfigPicture>, ConvertError> {
    match format {
        // EDT: the image is the sibling file whose stem is the slot (`Splash.png`).
        Format::Edt => {
            let rd = match std::fs::read_dir(dir) {
                Ok(rd) => rd,
                Err(_) => return Ok(None), // no dir ⇒ no image (honest).
            };
            let mut found: Option<PathBuf> = None;
            for entry in rd {
                let path = entry
                    .map_err(|e| ConvertError::Io {
                        path: dir.display().to_string(),
                        reason: e.to_string(),
                    })?
                    .path();
                let is_image = path.is_file()
                    && path.file_stem().and_then(|s| s.to_str()) == Some(slot)
                    && path.extension().and_then(|e| e.to_str()) != Some("bsl");
                if is_image {
                    if found.is_some() {
                        return Err(read_err(
                            slot,
                            format!(
                                "multiple `{slot}.*` image files beside the root descriptor in {} \
                                 (only one witnessed, §1.0)",
                                dir.display()
                            ),
                        ));
                    }
                    found = Some(path);
                }
            }
            let path = match found {
                Some(p) => p,
                None => return Ok(None),
            };
            let ext = image_ext(&path, slot)?;
            let bytes = read_bytes(&path)?;
            Ok(Some(ConfigPicture {
                slot: slot.to_string(),
                ext,
                bytes,
            }))
        }
        // Designer: wrapper `Ext/<Slot>.xml` → referenced file name (`Picture.<ext>`) → raw
        // image `Ext/<Slot>/Picture.<ext>`.
        Format::Designer => {
            let wrapper = dir.join(format!("{slot}.xml"));
            if !wrapper.is_file() {
                return Ok(None); // no wrapper ⇒ slot has no picture (honest).
            }
            let (file_name, pixel) = crate::picture_read::parse_wrapper(&wrapper, slot)?;
            // §1.0: config-root wrappers are witnessed pixel-less (`LoadTransparent=false`,
            // s15 + ERP corpus); `ConfigPicture` has no canonical home for a transparent
            // pixel → refuse loudly rather than drop it.
            if pixel.is_some() {
                return Err(read_err(
                    slot,
                    format!(
                        "Ext/{slot}.xml carries <xr:TransparentPixel> — unwitnessed for \
                         config-root pictures (§1.0)"
                    ),
                ));
            }
            let ref_path = Path::new(&file_name);
            // §1.0 witnessed-only: the wrapper references `Picture.<ext>` (same stem as
            // CommonPicture). Any other name is unwitnessed — refuse, don't guess.
            if ref_path.file_stem().and_then(|s| s.to_str()) != Some(PICTURE_STEM) {
                return Err(read_err(
                    slot,
                    format!(
                        "Ext/{slot}.xml references {file_name:?}; only `{PICTURE_STEM}.<ext>` \
                         witnessed for root pictures (§1.0)"
                    ),
                ));
            }
            let raw = dir.join(slot).join(&file_name);
            let ext = image_ext(&raw, slot)?;
            let bytes = read_bytes(&raw)?;
            Ok(Some(ConfigPicture {
                slot: slot.to_string(),
                ext,
                bytes,
            }))
        }
        Format::Cf => Ok(None),
    }
}

/// Write-side mirror of [`attach_config_ext`]: emit the root's app modules, pictures and
/// VERBATIM binary sidecars beside the just-written root descriptor `descriptor_out`, in
/// the target format's layout. No-op for non-root objects and cf. §1.0: a module in a
/// non-app slot, a picture or a blob in an unknown slot is a typed [`ConvertError::Write`],
/// never a silent drop.
pub fn write_config_ext(
    format: Format,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if obj.kind.as_str() != CONFIGURATION_KIND {
        return Ok(());
    }
    if obj.modules.is_empty()
        && obj.config_pictures.is_empty()
        && obj.config_blobs.is_empty()
        && obj.standalone_content.is_none()
        && obj.root_command_interface.is_none()
        && obj.main_section_command_interface.is_none()
        && obj.home_page_work_area.is_none()
        && obj.client_application_interface.is_none()
    {
        return Ok(());
    }
    let dir = match ext_dir(format, descriptor_out) {
        Some(d) => d,
        None => return Ok(()), // cf: container — the assembler owns cf body emission.
    };

    // (1) Application modules — per-format BOM convention via the shared re-encoder.
    for module in &obj.modules {
        if !APP_MODULE_SLOTS.contains(&module.slot.as_str()) {
            return Err(write_err(
                obj,
                format!(
                    "root Configuration carries module slot {:?} which is not a witnessed \
                     config-level Ext slot (known slots: {APP_MODULE_SLOTS:?})",
                    module.slot
                ),
            ));
        }
        let path = dir.join(format!("{}.bsl", module.slot));
        crate::form_write::write_file(
            &path,
            &crate::module_read::reencode_module_body(&module.body, format),
        )?;
    }

    // (2) Root pictures.
    for pic in &obj.config_pictures {
        if !CONFIG_PICTURE_SLOTS.contains(&pic.slot.as_str()) {
            return Err(write_err(
                obj,
                format!(
                    "root Configuration carries picture slot {:?} which is not a witnessed \
                     config-level Ext slot (known slots: {CONFIG_PICTURE_SLOTS:?})",
                    pic.slot
                ),
            ));
        }
        match format {
            // EDT: raw image `<Slot>.<ext>` beside the descriptor.
            Format::Edt => {
                crate::form_write::write_file(
                    &dir.join(format!("{}.{}", pic.slot, pic.ext)),
                    &pic.bytes,
                )?;
            }
            // Designer: raw `Ext/<Slot>/Picture.<ext>` + byte-exact `Ext/<Slot>.xml` wrapper.
            Format::Designer => {
                let file_name = format!("{PICTURE_STEM}.{}", pic.ext);
                crate::form_write::write_file(&dir.join(&pic.slot).join(&file_name), &pic.bytes)?;
                crate::form_write::write_file(
                    &dir.join(format!("{}.xml", pic.slot)),
                    // Config-root wrappers are pixel-less (witnessed s15 + ERP, §1.0).
                    &crate::picture_read::serialize_wrapper(&file_name, None),
                )?;
            }
            Format::Cf => unreachable!("cf returned above (no Ext dir)"),
        }
    }

    // (3) VERBATIM binary sidecars — байты дословно, пер-форматное ИМЯ из таблицы
    // (designer `Ext/MobileClientSignature.bin` ↔ edt `MobileClientSign.bin`).
    for blob in &obj.config_blobs {
        let entry = CONFIG_BLOB_SLOTS
            .iter()
            .find(|(slot, _, _)| *slot == blob.slot)
            .ok_or_else(|| {
                write_err(
                    obj,
                    format!(
                        "root Configuration carries binary sidecar slot {:?} which is not a \
                         witnessed config-level Ext slot (known slots: {:?})",
                        blob.slot,
                        CONFIG_BLOB_SLOTS
                            .iter()
                            .map(|(s, _, _)| *s)
                            .collect::<Vec<_>>()
                    ),
                )
            })?;
        crate::form_write::write_file(&dir.join(blob_file_name(format, entry)), &blob.bytes)?;
    }

    // (4) Состав автономной конфигурации — пер-диалектная СТРУКТУРНАЯ запись
    // ([`crate::standalone_content_read`]; no-op при `standalone_content == None`).
    crate::standalone_content_read::write_standalone_content(format, &dir, obj)?;

    // (5) Конфиг-уровневые интерфейс-сайдкары — пер-диалектная запись
    // ([`crate::config_interface_read`]; no-op при отсутствующих полях).
    crate::config_interface_read::write_config_interfaces(format, &dir, obj)?;
    Ok(())
}

/// The image file's extension (lowercase not enforced — kept verbatim). §1.0: an image with
/// NO extension is unwitnessed → typed error, not a guess.
fn image_ext(path: &Path, slot: &str) -> Result<String, ConvertError> {
    match path.extension().and_then(|e| e.to_str()) {
        Some(e) if !e.is_empty() => Ok(e.to_string()),
        _ => Err(read_err(
            slot,
            format!(
                "root picture file {} has no extension (unwitnessed, §1.0)",
                path.display()
            ),
        )),
    }
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, ConvertError> {
    std::fs::read(path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })
}

fn read_err(slot: &str, reason: String) -> ConvertError {
    ConvertError::Read {
        kind: CONFIGURATION_KIND.to_string(),
        object: slot.to_string(),
        reason,
    }
}

fn write_err(obj: &MetadataObject, reason: String) -> ConvertError {
    ConvertError::Write {
        kind: CONFIGURATION_KIND.to_string(),
        object: obj.name.clone(),
        reason,
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::{ObjectKind, Uuid};

    const BOM: &str = "\u{FEFF}";

    fn root() -> MetadataObject {
        MetadataObject::new(ObjectKind::new("Configuration"), "Конфиг", Uuid([0; 16]))
    }

    fn temp_base(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "morph1c-ext-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn non_root_kind_is_noop() {
        let mut obj = MetadataObject::new(ObjectKind::new("Catalog"), "Спр", Uuid([0; 16]));
        attach_config_ext(
            Format::Designer,
            Path::new("/nope/Catalogs/Спр.xml"),
            &mut obj,
        )
        .unwrap();
        assert!(
            obj.modules.is_empty() && obj.config_pictures.is_empty() && obj.config_blobs.is_empty()
        );
    }

    #[test]
    fn missing_files_attach_nothing() {
        let mut obj = root();
        attach_config_ext(
            Format::Edt,
            Path::new("/nope/Configuration/Configuration.mdo"),
            &mut obj,
        )
        .unwrap();
        assert!(
            obj.modules.is_empty() && obj.config_pictures.is_empty() && obj.config_blobs.is_empty()
        );
    }

    /// attach по обоим диалектам + write byte-exact (свой диалект) + КРОСС-диалектно
    /// (BOM/имена файлов цели), на temp-дереве, зеркалирующем s15-раскладку.
    #[test]
    fn attach_and_write_roundtrip_both_dialects() {
        let base = temp_base("rt");
        let session_src = "// Модуль сеанса\r\nПроцедура С() КонецПроцедуры\r\n";
        let managed_src = "// Модуль управляемого приложения\r\nПроцедура У() КонецПроцедуры\r\n";
        let png: &[u8] = b"\x89PNG\r\n\x1a\nfakebody";

        // EDT: сайдкары — сиблинги Configuration.mdo.
        let edt_dir = base.join("edt/src/Configuration");
        std::fs::create_dir_all(&edt_dir).unwrap();
        let edt_mdo = edt_dir.join("Configuration.mdo");
        crate::fsio::write(&edt_mdo, b"<mdo/>").unwrap();
        crate::fsio::write(edt_dir.join("ManagedApplicationModule.bsl"), managed_src).unwrap();
        crate::fsio::write(edt_dir.join("SessionModule.bsl"), session_src).unwrap();
        crate::fsio::write(edt_dir.join("Splash.png"), png).unwrap();
        crate::fsio::write(edt_dir.join("MainSectionPicture.png"), png).unwrap();

        // Designer: корневой Ext/ рядом с Configuration.xml (+BOM у модулей, обёртки картинок).
        let dsn_root = base.join("designer");
        let dsn_ext = dsn_root.join("Ext");
        std::fs::create_dir_all(dsn_ext.join("Splash")).unwrap();
        std::fs::create_dir_all(dsn_ext.join("MainSectionPicture")).unwrap();
        let dsn_xml = dsn_root.join("Configuration.xml");
        crate::fsio::write(&dsn_xml, b"<xml/>").unwrap();
        crate::fsio::write(
            dsn_ext.join("ManagedApplicationModule.bsl"),
            format!("{BOM}{managed_src}"),
        )
        .unwrap();
        crate::fsio::write(
            dsn_ext.join("SessionModule.bsl"),
            format!("{BOM}{session_src}"),
        )
        .unwrap();
        crate::fsio::write(dsn_ext.join("Splash/Picture.png"), png).unwrap();
        crate::fsio::write(
            dsn_ext.join("Splash.xml"),
            crate::picture_read::serialize_wrapper("Picture.png", None),
        )
        .unwrap();
        crate::fsio::write(dsn_ext.join("MainSectionPicture/Picture.png"), png).unwrap();
        crate::fsio::write(
            dsn_ext.join("MainSectionPicture.xml"),
            crate::picture_read::serialize_wrapper("Picture.png", None),
        )
        .unwrap();

        // attach: оба диалекта дают те же слоты в детерминированном порядке.
        let mut edt_obj = root();
        attach_config_ext(Format::Edt, &edt_mdo, &mut edt_obj).unwrap();
        assert_eq!(
            edt_obj
                .modules
                .iter()
                .map(|m| m.slot.as_str())
                .collect::<Vec<_>>(),
            ["ManagedApplicationModule", "SessionModule"]
        );
        assert_eq!(edt_obj.modules[0].as_text().unwrap(), managed_src);
        assert_eq!(
            edt_obj
                .config_pictures
                .iter()
                .map(|p| p.slot.as_str())
                .collect::<Vec<_>>(),
            ["MainSectionPicture", "Splash"]
        );
        assert_eq!(edt_obj.config_pictures[0].ext, "png");
        assert_eq!(edt_obj.config_pictures[0].bytes, png);

        let mut dsn_obj = root();
        attach_config_ext(Format::Designer, &dsn_xml, &mut dsn_obj).unwrap();
        assert_eq!(
            dsn_obj
                .modules
                .iter()
                .map(|m| m.slot.as_str())
                .collect::<Vec<_>>(),
            ["ManagedApplicationModule", "SessionModule"]
        );
        assert_eq!(
            dsn_obj.modules[0].as_text().unwrap(),
            format!("{BOM}{managed_src}")
        );
        assert_eq!(
            dsn_obj
                .config_pictures
                .iter()
                .map(|p| p.slot.as_str())
                .collect::<Vec<_>>(),
            ["MainSectionPicture", "Splash"]
        );
        assert_eq!(dsn_obj.config_pictures[1].bytes, png);
        // Designer-attach синтезирует present-empty picture-свойства (file-gated: EDT-узел
        // `<mainSectionPicture/>`/`<splash/>` существует РОВНО при наличии картинки); EDT-
        // attach свойств НЕ трогает — их несёт сам дескриптор.
        {
            use morph1c_core::ir::value::PropertyValue;
            use morph1c_core::spec::metadata::configuration as cfgspec;
            assert!(
                edt_obj.properties.is_empty(),
                "edt attach must not synthesize descriptor properties"
            );
            assert_eq!(
                dsn_obj.properties,
                vec![
                    (
                        cfgspec::F_MAIN_SECTION_PICTURE,
                        PropertyValue::Str(String::new())
                    ),
                    (cfgspec::F_SPLASH, PropertyValue::Str(String::new())),
                ],
                "designer attach must synthesize present-empty picture properties"
            );
        }

        // КРОСС-диалектная запись: designer-IR → edt-байты (BOM снят, имя = слот)…
        let cross_edt = base.join("cross-edt/src/Configuration");
        std::fs::create_dir_all(&cross_edt).unwrap();
        let cross_edt_mdo = cross_edt.join("Configuration.mdo");
        crate::fsio::write(&cross_edt_mdo, b"<mdo/>").unwrap();
        write_config_ext(Format::Edt, &cross_edt_mdo, &dsn_obj).unwrap();
        assert_eq!(
            std::fs::read(cross_edt.join("SessionModule.bsl")).unwrap(),
            session_src.as_bytes(),
            "designer-IR → edt module must strip the BOM"
        );
        assert_eq!(std::fs::read(cross_edt.join("Splash.png")).unwrap(), png);
        assert_eq!(
            std::fs::read(cross_edt.join("MainSectionPicture.png")).unwrap(),
            png
        );

        // …и edt-IR → designer-байты (BOM добавлен, Ext/<Slot>.xml + Ext/<Slot>/Picture.png).
        let cross_dsn = base.join("cross-dsn");
        std::fs::create_dir_all(&cross_dsn).unwrap();
        let cross_dsn_xml = cross_dsn.join("Configuration.xml");
        crate::fsio::write(&cross_dsn_xml, b"<xml/>").unwrap();
        write_config_ext(Format::Designer, &cross_dsn_xml, &edt_obj).unwrap();
        assert_eq!(
            std::fs::read(cross_dsn.join("Ext/SessionModule.bsl")).unwrap(),
            format!("{BOM}{session_src}").as_bytes(),
            "edt-IR → designer module must add the BOM"
        );
        assert_eq!(
            std::fs::read(cross_dsn.join("Ext/Splash/Picture.png")).unwrap(),
            png
        );
        assert_eq!(
            std::fs::read(cross_dsn.join("Ext/Splash.xml")).unwrap(),
            std::fs::read(dsn_ext.join("Splash.xml")).unwrap(),
            "regenerated Ext/Splash.xml wrapper must be byte-exact"
        );

        // Свой диалект: designer-IR → designer byte-exact по всем шести файлам.
        let own_dsn = base.join("own-dsn");
        std::fs::create_dir_all(&own_dsn).unwrap();
        let own_dsn_xml = own_dsn.join("Configuration.xml");
        crate::fsio::write(&own_dsn_xml, b"<xml/>").unwrap();
        write_config_ext(Format::Designer, &own_dsn_xml, &dsn_obj).unwrap();
        for rel in [
            "Ext/ManagedApplicationModule.bsl",
            "Ext/SessionModule.bsl",
            "Ext/Splash.xml",
            "Ext/Splash/Picture.png",
            "Ext/MainSectionPicture.xml",
            "Ext/MainSectionPicture/Picture.png",
        ] {
            assert_eq!(
                std::fs::read(own_dsn.join(rel)).unwrap(),
                std::fs::read(dsn_root.join(rel)).unwrap(),
                "designer→designer {rel} not byte-exact"
            );
        }

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Designer-attach БЕЗ файлов картинок НЕ синтезирует picture-свойств (file-gated
    /// presence: нет Ext/<Slot>.xml ⇒ EDT-узел `<mainSectionPicture/>`/`<splash/>` не
    /// регенерируется — s1..s14 их не несут).
    #[test]
    fn designer_attach_without_pictures_injects_no_properties() {
        let base = temp_base("nopics");
        let ext = base.join("Ext");
        std::fs::create_dir_all(&ext).unwrap();
        crate::fsio::write(base.join("Configuration.xml"), b"<xml/>").unwrap();
        crate::fsio::write(ext.join("SessionModule.bsl"), format!("{BOM}// x\r\n")).unwrap();
        let mut obj = root();
        attach_config_ext(Format::Designer, &base.join("Configuration.xml"), &mut obj).unwrap();
        assert_eq!(obj.modules.len(), 1);
        assert!(obj.config_pictures.is_empty());
        assert!(
            obj.config_blobs.is_empty(),
            "no .bin files => no config blobs (honest absence, как SSL без сигнатуры)"
        );
        assert!(
            obj.properties.is_empty(),
            "no picture files => no synthesized properties"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// VERBATIM-сайдкары: attach по обоим диалектам (байты ДОСЛОВНО, вкл. BOM; порядок =
    /// таблица слотов) + КРОСС-диалектная запись с пер-форматным ИМЕНЕМ файла
    /// (designer `Ext/MobileClientSignature.bin` ↔ edt `MobileClientSign.bin`) байт-в-байт.
    /// Микрофикстура — витнесс-эхо ERP: 16-байтный `ParentConfigurations.bin` =
    /// BOM + `{6,0,0,0,1,0}`; сигнатура — brace `{2,"MIIBtjCC…"}`.
    #[test]
    fn attach_and_write_config_blobs_both_dialects() {
        let base = temp_base("blobs");
        let parent: &[u8] = b"\xEF\xBB\xBF{6,0,0,0,1,0}";
        let sig: &[u8] = b"\xEF\xBB\xBF{2,\"MIIBtjCC\"}\r\n";

        // EDT: сайдкары — сиблинги Configuration.mdo; имя сигнатуры КОРОЧЕ.
        let edt_dir = base.join("edt/src/Configuration");
        std::fs::create_dir_all(&edt_dir).unwrap();
        let edt_mdo = edt_dir.join("Configuration.mdo");
        crate::fsio::write(&edt_mdo, b"<mdo/>").unwrap();
        crate::fsio::write(edt_dir.join("ParentConfigurations.bin"), parent).unwrap();
        crate::fsio::write(edt_dir.join("MobileClientSign.bin"), sig).unwrap();

        // Designer: корневой Ext/ рядом с Configuration.xml; полное имя сигнатуры.
        let dsn_root = base.join("designer");
        let dsn_ext = dsn_root.join("Ext");
        std::fs::create_dir_all(&dsn_ext).unwrap();
        let dsn_xml = dsn_root.join("Configuration.xml");
        crate::fsio::write(&dsn_xml, b"<xml/>").unwrap();
        crate::fsio::write(dsn_ext.join("ParentConfigurations.bin"), parent).unwrap();
        crate::fsio::write(dsn_ext.join("MobileClientSignature.bin"), sig).unwrap();

        // attach: оба диалекта дают те же слоты в порядке таблицы, байты — вербатим.
        let mut edt_obj = root();
        attach_config_ext(Format::Edt, &edt_mdo, &mut edt_obj).unwrap();
        let mut dsn_obj = root();
        attach_config_ext(Format::Designer, &dsn_xml, &mut dsn_obj).unwrap();
        for (tag, obj) in [("edt", &edt_obj), ("designer", &dsn_obj)] {
            assert_eq!(
                obj.config_blobs
                    .iter()
                    .map(|b| b.slot.as_str())
                    .collect::<Vec<_>>(),
                ["ParentConfigurations", "MobileClientSignature"],
                "{tag}: slots in deterministic table order"
            );
            assert_eq!(
                obj.config_blobs[0].bytes, parent,
                "{tag}: verbatim (BOM intact)"
            );
            assert_eq!(
                obj.config_blobs[1].bytes, sig,
                "{tag}: verbatim (CRLF intact)"
            );
        }
        assert_eq!(
            edt_obj.config_blobs, dsn_obj.config_blobs,
            "§1.6: единый канон"
        );

        // КРОСС-диалектная запись: designer-IR → edt-имена (MobileClientSign.bin)…
        let cross_edt = base.join("cross-edt/src/Configuration");
        std::fs::create_dir_all(&cross_edt).unwrap();
        let cross_edt_mdo = cross_edt.join("Configuration.mdo");
        crate::fsio::write(&cross_edt_mdo, b"<mdo/>").unwrap();
        write_config_ext(Format::Edt, &cross_edt_mdo, &dsn_obj).unwrap();
        assert_eq!(
            std::fs::read(cross_edt.join("ParentConfigurations.bin")).unwrap(),
            parent
        );
        assert_eq!(
            std::fs::read(cross_edt.join("MobileClientSign.bin")).unwrap(),
            sig,
            "designer-IR → edt must use the SHORT edt file name, bytes verbatim"
        );
        assert!(
            !cross_edt.join("MobileClientSignature.bin").exists(),
            "edt must NOT get the designer-named file"
        );

        // …и edt-IR → designer-имена (Ext/MobileClientSignature.bin), байт-в-байт.
        let cross_dsn = base.join("cross-dsn");
        std::fs::create_dir_all(&cross_dsn).unwrap();
        let cross_dsn_xml = cross_dsn.join("Configuration.xml");
        crate::fsio::write(&cross_dsn_xml, b"<xml/>").unwrap();
        write_config_ext(Format::Designer, &cross_dsn_xml, &edt_obj).unwrap();
        for rel in [
            "Ext/ParentConfigurations.bin",
            "Ext/MobileClientSignature.bin",
        ] {
            assert_eq!(
                std::fs::read(cross_dsn.join(rel)).unwrap(),
                std::fs::read(dsn_root.join(rel)).unwrap(),
                "edt-IR → designer {rel} not byte-exact"
            );
        }
        assert!(
            !cross_dsn.join("Ext/MobileClientSign.bin").exists(),
            "designer must NOT get the edt-named file"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Один сайдкар без другого (SSL: ParentConfigurations есть, сигнатуры нет) →
    /// IR несёт ровно присутствующий; отсутствие второго честно.
    #[test]
    fn attach_single_config_blob_is_honest() {
        let base = temp_base("oneblob");
        let ext = base.join("Ext");
        std::fs::create_dir_all(&ext).unwrap();
        crate::fsio::write(base.join("Configuration.xml"), b"<xml/>").unwrap();
        crate::fsio::write(
            ext.join("ParentConfigurations.bin"),
            b"\xEF\xBB\xBF{6,0,0,0,1,0}",
        )
        .unwrap();
        let mut obj = root();
        attach_config_ext(Format::Designer, &base.join("Configuration.xml"), &mut obj).unwrap();
        assert_eq!(
            obj.config_blobs
                .iter()
                .map(|b| b.slot.as_str())
                .collect::<Vec<_>>(),
            ["ParentConfigurations"],
            "only the present sidecar attaches"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// СТРУКТУРНЫЙ standalone-сайдкар: attach по обоим диалектам через конфиг-Ext-проход
    /// (designer `Ext/StandaloneConfigurationContent.bin` ↔ edt `MobileApplicationContent.scc`)
    /// → равный IR; КРОСС-диалектная запись кладёт пер-форматное ИМЯ и ДИАЛЕКТ.
    #[test]
    fn attach_and_write_standalone_content_both_dialects() {
        use morph1c_core::ir::{StandaloneContent, StandalonePriority};
        let base = temp_base("standalone");
        let sc = StandaloneContent {
            used: vec!["Role.РольА".to_string()],
            unused: vec!["Catalog.Кат.Attribute.Рекв".to_string()],
            priority: vec![StandalonePriority {
                metadata: "Constant.Конст".to_string(),
                priority: "LocalServer".to_string(),
            }],
            exchange_on_change_data: true,
            exchange_period: 300,
            transaction_count: 1000,
            inactive_nodes_cleanup_timeout: 0,
        };

        // Designer-дерево: Ext/StandaloneConfigurationContent.bin рядом с Configuration.xml.
        let dsn_root = base.join("designer");
        let dsn_ext = dsn_root.join("Ext");
        std::fs::create_dir_all(&dsn_ext).unwrap();
        let dsn_xml = dsn_root.join("Configuration.xml");
        crate::fsio::write(&dsn_xml, b"<xml/>").unwrap();
        let dsn_bytes = morph1c_core::version::with_roundtrip_target(
            morph1c_core::version::ERP,
            || -> Vec<u8> {
                let mut obj = root();
                obj.standalone_content = Some(sc.clone());
                write_config_ext(Format::Designer, &dsn_xml, &obj).unwrap();
                std::fs::read(dsn_ext.join("StandaloneConfigurationContent.bin")).unwrap()
            },
        );
        assert!(dsn_bytes.starts_with(&[0xEF, 0xBB, 0xBF]), "designer BOM");

        let mut dsn_obj = root();
        attach_config_ext(Format::Designer, &dsn_xml, &mut dsn_obj).unwrap();
        assert_eq!(dsn_obj.standalone_content.as_ref(), Some(&sc));

        // КРОСС-диалектная запись: designer-IR → edt-имя `MobileApplicationContent.scc`.
        let edt_dir = base.join("edt/src/Configuration");
        std::fs::create_dir_all(&edt_dir).unwrap();
        let edt_mdo = edt_dir.join("Configuration.mdo");
        crate::fsio::write(&edt_mdo, b"<mdo/>").unwrap();
        write_config_ext(Format::Edt, &edt_mdo, &dsn_obj).unwrap();
        let scc = edt_dir.join("MobileApplicationContent.scc");
        assert!(scc.is_file(), "edt gets the .scc name");
        assert!(
            !edt_dir.join("StandaloneConfigurationContent.bin").exists(),
            "edt must NOT get the designer-named file"
        );
        let mut edt_obj = root();
        attach_config_ext(Format::Edt, &edt_mdo, &mut edt_obj).unwrap();
        assert_eq!(
            edt_obj.standalone_content, dsn_obj.standalone_content,
            "§1.6: единый канон"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Конфиг-уровневые ИНТЕРФЕЙС-сайдкары через конфиг-Ext-проход: запись 4 полей в
    /// designer-дерево → attach designer → равный IR → КРОСС-диалектная запись в edt
    /// (пер-форматные имена: `.cmi`/`.hpwa`/`.cai`) → attach edt → §1.6-равенство канонов.
    #[test]
    fn attach_and_write_config_interfaces_both_dialects() {
        use morph1c_core::ir::{
            CaiGroup, CaiPanel, ClientApplicationInterface, CommandInterface, HomePageItem,
            HomePageWorkArea,
        };
        let base = temp_base("ifaces");
        let empty_ci = CommandInterface {
            commands: Vec::new(),
            placement: Vec::new(),
            order: Vec::new(),
            subsystems_order: vec!["Subsystem.Продажи".to_string()],
        };
        let hp = HomePageWorkArea {
            template: "TwoColumnsVariableWidth".to_string(),
            left: vec![HomePageItem {
                form: "Catalog.Заметки.Form.МоиЗаметки".to_string(),
                height: 10,
                common_visible: true,
                role_values: Vec::new(),
            }],
            right: Vec::new(),
        };
        let cai = ClientApplicationInterface {
            top: CaiGroup {
                id: "1b4042b7-6a75-4b3a-a7cd-e5306844686c".to_string(),
                panels: vec![CaiPanel {
                    id: "1c6905f0-bef8-4e56-958c-25c457268b87".to_string(),
                    name: "ToolsPanel".to_string(),
                }],
            },
            left: CaiGroup {
                id: "5e17de8d-d23a-4fad-ba89-943c4cc9deb6".to_string(),
                panels: vec![CaiPanel {
                    id: "de5a40ef-33e7-45fb-a690-73fc69670243".to_string(),
                    name: "SectionPanel".to_string(),
                }],
            },
        };

        // Designer-дерево: Ext/ рядом с Configuration.xml (версия сайдкаров — ERP-таргет).
        let dsn_root = base.join("designer");
        std::fs::create_dir_all(dsn_root.join("Ext")).unwrap();
        let dsn_xml = dsn_root.join("Configuration.xml");
        crate::fsio::write(&dsn_xml, b"<xml/>").unwrap();
        morph1c_core::version::with_roundtrip_target(morph1c_core::version::ERP, || {
            let mut obj = root();
            obj.root_command_interface = Some(empty_ci.clone());
            obj.main_section_command_interface = Some(empty_ci.clone());
            obj.home_page_work_area = Some(hp.clone());
            obj.client_application_interface = Some(cai.clone());
            write_config_ext(Format::Designer, &dsn_xml, &obj).unwrap();
        });
        for f in [
            "Ext/CommandInterface.xml",
            "Ext/MainSectionCommandInterface.xml",
            "Ext/HomePageWorkArea.xml",
            "Ext/ClientApplicationInterface.xml",
        ] {
            assert!(dsn_root.join(f).is_file(), "designer wrote {f}");
        }

        let mut dsn_obj = root();
        attach_config_ext(Format::Designer, &dsn_xml, &mut dsn_obj).unwrap();
        assert_eq!(dsn_obj.root_command_interface.as_ref(), Some(&empty_ci));
        assert_eq!(
            dsn_obj.main_section_command_interface.as_ref(),
            Some(&empty_ci)
        );
        assert_eq!(dsn_obj.home_page_work_area.as_ref(), Some(&hp));
        assert_eq!(dsn_obj.client_application_interface.as_ref(), Some(&cai));

        // КРОСС-диалектная запись: designer-IR → edt-имена, затем edt-attach → тот же канон.
        let edt_dir = base.join("edt/src/Configuration");
        std::fs::create_dir_all(&edt_dir).unwrap();
        let edt_mdo = edt_dir.join("Configuration.mdo");
        crate::fsio::write(&edt_mdo, b"<mdo/>").unwrap();
        write_config_ext(Format::Edt, &edt_mdo, &dsn_obj).unwrap();
        for f in [
            "CommandInterface.cmi",
            "MainSectionCommandInterface.cmi",
            "HomePageWorkArea.hpwa",
            "ClientApplicationInterface.cai",
        ] {
            assert!(edt_dir.join(f).is_file(), "edt wrote {f}");
        }
        let mut edt_obj = root();
        attach_config_ext(Format::Edt, &edt_mdo, &mut edt_obj).unwrap();
        assert_eq!(
            (
                &edt_obj.root_command_interface,
                &edt_obj.main_section_command_interface,
                &edt_obj.home_page_work_area,
                &edt_obj.client_application_interface,
            ),
            (
                &dsn_obj.root_command_interface,
                &dsn_obj.main_section_command_interface,
                &dsn_obj.home_page_work_area,
                &dsn_obj.client_application_interface,
            ),
            "§1.6: единый канон"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// §1.0: бинарный сайдкар в неизвестном слоте → громкая типизированная ошибка.
    #[test]
    fn write_unknown_blob_slot_is_loud_error() {
        let mut obj = root();
        obj.config_blobs.push(ConfigBlob {
            slot: "StandaloneConfigurationContent".into(),
            bytes: vec![1, 2, 3],
        });
        let err = write_config_ext(Format::Designer, Path::new("/nope/Configuration.xml"), &obj)
            .unwrap_err();
        assert!(
            err.to_string().contains("StandaloneConfigurationContent"),
            "got: {err}"
        );
    }

    /// §1.0: модуль в чужом слоте (не app-модуль корня) → громкая типизированная ошибка.
    #[test]
    fn write_unknown_module_slot_is_loud_error() {
        let mut obj = root();
        obj.modules.push(Module::text("ObjectModule", "// x\r\n"));
        let err = write_config_ext(
            Format::Edt,
            Path::new("/nope/Configuration/Configuration.mdo"),
            &obj,
        )
        .unwrap_err();
        assert!(err.to_string().contains("ObjectModule"), "got: {err}");
    }

    /// §1.0: картинка в неизвестном слоте → громкая типизированная ошибка.
    #[test]
    fn write_unknown_picture_slot_is_loud_error() {
        let mut obj = root();
        obj.config_pictures.push(ConfigPicture {
            slot: "Logo".into(),
            ext: "png".into(),
            bytes: vec![1, 2, 3],
        });
        let err = write_config_ext(Format::Designer, Path::new("/nope/Configuration.xml"), &obj)
            .unwrap_err();
        assert!(err.to_string().contains("Logo"), "got: {err}");
    }

    /// §1.0: designer-обёртка, ссылающаяся НЕ на `Picture.<ext>`, отвергается (unwitnessed).
    #[test]
    fn designer_wrapper_with_foreign_file_name_is_refused() {
        let base = temp_base("badref");
        let ext = base.join("Ext");
        std::fs::create_dir_all(&ext).unwrap();
        crate::fsio::write(base.join("Configuration.xml"), b"<xml/>").unwrap();
        crate::fsio::write(
            ext.join("Splash.xml"),
            crate::picture_read::serialize_wrapper("Logo.png", None),
        )
        .unwrap();
        let mut obj = root();
        let err = attach_config_ext(Format::Designer, &base.join("Configuration.xml"), &mut obj)
            .unwrap_err();
        assert!(err.to_string().contains("Logo.png"), "got: {err}");
        let _ = std::fs::remove_dir_all(&base);
    }
}
