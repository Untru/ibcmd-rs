use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use quick_xml::Reader;
use quick_xml::events::Event;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::parallel;

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceManifest {
    pub root: PathBuf,
    pub generated_at_unix: u64,
    pub files: Vec<SourceFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub kind: SourceKind,
    pub xml_root: Option<String>,
    pub object_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    ConfigurationRoot,
    MetadataXml,
    Module,
    Form,
    Template,
    Binary,
    OtherXml,
    Other,
}

pub fn scan_sources(root: &Path) -> Result<SourceManifest> {
    if !root.is_dir() {
        return Err(anyhow!(
            "source root is not a directory: {}",
            root.display()
        ));
    }

    let canonical_root = fs::canonicalize(root)
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    let mut files = parallel::install(|| {
        WalkDir::new(&canonical_root)
            .into_iter()
            .filter_entry(|entry| !is_ignored(entry.path()))
            .par_bridge()
            .filter_map(|entry| match entry {
                Ok(entry) if entry.file_type().is_file() => {
                    Some(Ok::<walkdir::DirEntry, anyhow::Error>(entry))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error.into())),
            })
            .map(|entry| {
                let entry = entry?;
                let path = entry.path();
                let relative = path.strip_prefix(&canonical_root).with_context(|| {
                    format!("failed to make relative path for {}", path.display())
                })?;
                scan_file(path, relative)
            })
            .collect::<Result<Vec<_>>>()
    })??;
    files.par_sort_by(|left, right| left.path.cmp(&right.path));

    Ok(SourceManifest {
        root: canonical_root,
        generated_at_unix: now_unix(),
        files,
    })
}

pub fn write_manifest(manifest: &SourceManifest, output: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(manifest)?;
    fs::write(output, json).with_context(|| format!("failed to write {}", output.display()))
}

pub fn read_manifest(path: &Path) -> Result<SourceManifest> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read manifest {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse manifest {}", path.display()))
}

fn scan_file(path: &Path, relative: &Path) -> Result<SourceFile> {
    let metadata =
        fs::metadata(path).with_context(|| format!("failed to stat {}", path.display()))?;
    let sha256 = sha256_file(path)?;
    let relative_text = normalize_path(relative);
    let xml_root = if is_xml(path) {
        if should_parse_xml_root(&relative_text) {
            first_xml_element(path).unwrap_or(None)
        } else {
            None
        }
    } else {
        None
    };
    let kind = classify(path, &relative_text, xml_root.as_deref());
    let object_hint = infer_object_hint(&relative_text, &kind, xml_root.as_deref());

    Ok(SourceFile {
        path: relative_text,
        size_bytes: metadata.len(),
        sha256,
        kind,
        xml_root,
        object_hint,
    })
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file =
        File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn first_xml_element(path: &Path) -> Result<Option<String>> {
    let file =
        File::open(path).with_context(|| format!("failed to open xml {}", path.display()))?;
    let mut reader = Reader::from_reader(BufReader::new(file));
    reader.config_mut().trim_text(true);

    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref()).to_string();
                return Ok(Some(name));
            }
            Ok(Event::Eof) => return Ok(None),
            Ok(_) => {}
            Err(error) => {
                return Err(anyhow!("invalid xml {}: {}", path.display(), error));
            }
        }
        buffer.clear();
    }
}

fn classify(path: &Path, relative: &str, xml_root: Option<&str>) -> SourceKind {
    let lower = relative.to_ascii_lowercase();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if lower == "configuration.xml" {
        return SourceKind::ConfigurationRoot;
    }
    if extension == "bsl" {
        return SourceKind::Module;
    }
    if lower.contains("/forms/") || lower.ends_with("/form.xml") {
        return SourceKind::Form;
    }
    if lower.contains("/templates/") || lower.ends_with("/template.xml") || extension == "mxl" {
        return SourceKind::Template;
    }
    if extension == "xml" {
        return match xml_root {
            Some(root) if root.contains("MetaDataObject") || root.contains("Configuration") => {
                SourceKind::MetadataXml
            }
            Some("DefinedType") => SourceKind::MetadataXml,
            _ if has_metadata_collection_folder(&lower) => SourceKind::MetadataXml,
            _ if is_metadata_subfile(&lower) => SourceKind::MetadataXml,
            _ => SourceKind::OtherXml,
        };
    }
    if matches!(
        extension.as_str(),
        "bin" | "png" | "jpg" | "jpeg" | "gif" | "ico" | "svg" | "zip"
    ) {
        return SourceKind::Binary;
    }

    SourceKind::Other
}

fn infer_object_hint(relative: &str, kind: &SourceKind, xml_root: Option<&str>) -> Option<String> {
    if matches!(kind, SourceKind::ConfigurationRoot) {
        return Some("Configuration".to_string());
    }

    let parts: Vec<&str> = relative.split('/').collect();
    for window in parts.windows(2) {
        let folder = window[0];
        let name = window[1];
        if is_metadata_collection(folder) {
            let name = Path::new(name)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or(name);
            return Some(format!("{}/{}", folder, name));
        }
    }

    xml_root.map(ToOwned::to_owned)
}

fn is_metadata_collection(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    matches!(
        value.as_str(),
        "catalogs"
            | "documents"
            | "informationregisters"
            | "accumulationregisters"
            | "accountingregisters"
            | "calculationregisters"
            | "chartsofcharacteristictypes"
            | "chartsofaccounts"
            | "chartsofcalculationtypes"
            | "chartsofcalculationregisters"
            | "commonmodules"
            | "commonforms"
            | "commonpictures"
            | "commontemplates"
            | "commonattributes"
            | "commandgroups"
            | "documentjournals"
            | "reports"
            | "dataprocessors"
            | "enums"
            | "exchangeplans"
            | "eventsubscriptions"
            | "filtercriteria"
            | "functionaloptions"
            | "functionaloptionsparameters"
            | "httpservices"
            | "languages"
            | "scheduledjobs"
            | "sessionparameters"
            | "settingsstorages"
            | "styleitems"
            | "subsystems"
            | "roles"
            | "commoncommands"
            | "businessprocesses"
            | "definedtypes"
            | "tasks"
            | "constants"
            | "webservices"
            | "xdtopackages"
    )
}

fn is_xml(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
}

fn is_ignored(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| matches!(name, ".git" | "target" | ".idea" | ".vscode"))
}

fn is_metadata_subfile(relative_lower: &str) -> bool {
    relative_lower.starts_with("ext/") || relative_lower.contains("/ext/")
}

fn should_parse_xml_root(relative: &str) -> bool {
    let lower = relative.to_ascii_lowercase();
    if lower == "configuration.xml" {
        return true;
    }
    if lower.contains("/forms/") || lower.ends_with("/form.xml") {
        return false;
    }
    if lower.contains("/templates/") || lower.ends_with("/template.xml") {
        return false;
    }
    if has_metadata_collection_folder(&lower) || is_metadata_subfile(&lower) {
        return false;
    }
    true
}

fn has_metadata_collection_folder(relative_lower: &str) -> bool {
    let parts: Vec<&str> = relative_lower.split('/').collect();
    parts
        .windows(2)
        .any(|window| is_metadata_collection(window[0]))
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{SourceKind, classify, infer_object_hint, scan_sources};

    #[test]
    fn classifies_common_1c_source_files() {
        assert_eq!(
            classify(
                "Configuration.xml".as_ref(),
                "Configuration.xml",
                Some("MetaDataObject.Configuration")
            ),
            SourceKind::ConfigurationRoot
        );
        assert_eq!(
            classify("Module.bsl".as_ref(), "CommonModules/Foo/Module.bsl", None),
            SourceKind::Module
        );
        assert_eq!(
            classify(
                "Form.xml".as_ref(),
                "Catalogs/Goods/Forms/ItemForm/Form.xml",
                Some("Form")
            ),
            SourceKind::Form
        );
    }

    #[test]
    fn infers_metadata_owner_from_path() {
        assert_eq!(
            infer_object_hint(
                "Catalogs/Goods/Forms/ItemForm/Form.xml",
                &SourceKind::Form,
                Some("Form")
            ),
            Some("Catalogs/Goods".to_string())
        );
    }

    #[test]
    fn infers_additional_metadata_owners_from_path() {
        assert_eq!(
            infer_object_hint(
                "AccumulationRegisters/Sales/Forms/Report/Form.xml",
                &SourceKind::Form,
                Some("Form")
            ),
            Some("AccumulationRegisters/Sales".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "ChartsOfCalculationRegisters/Payouts/Forms/ItemForm/Form.xml",
                &SourceKind::Form,
                Some("Form")
            ),
            Some("ChartsOfCalculationRegisters/Payouts".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "AccumulationRegisters/Продажи/Forms/ФормаСписка/Form.xml",
                &SourceKind::Form,
                Some("Form")
            ),
            Some("AccumulationRegisters/Продажи".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "AccountingRegisters/Хозрасчеты/Ext/Help.xml",
                &SourceKind::MetadataXml,
                Some("Help")
            ),
            Some("AccountingRegisters/Хозрасчеты".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "CalculationRegisters/Премии/Commands/Печать/Ext/CommandModule.bsl",
                &SourceKind::Module,
                None
            ),
            Some("CalculationRegisters/Премии".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "ChartsOfAccounts/ПланСчетов/Ext/Help.xml",
                &SourceKind::MetadataXml,
                Some("Help")
            ),
            Some("ChartsOfAccounts/ПланСчетов".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "CommonForms/АвтономнаяРабота.xml",
                &SourceKind::MetadataXml,
                Some("MetaDataObject")
            ),
            Some("CommonForms/АвтономнаяРабота".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "CommonCommands/АвтономнаяРабота/Ext/CommandInterface.xml",
                &SourceKind::MetadataXml,
                Some("CommandInterface")
            ),
            Some("CommonCommands/АвтономнаяРабота".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "BusinessProcesses/Задание/Ext/Flowchart.xml",
                &SourceKind::MetadataXml,
                Some("GraphicalSchema")
            ),
            Some("BusinessProcesses/Задание".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "DefinedTypes/ВладелецДополнительныхСведений.xml",
                &SourceKind::MetadataXml,
                Some("DefinedType")
            ),
            Some("DefinedTypes/ВладелецДополнительныхСведений".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "ScheduledJobs/ЗагрузкаКурсовВалют.xml",
                &SourceKind::MetadataXml,
                Some("MetaDataObject")
            ),
            Some("ScheduledJobs/ЗагрузкаКурсовВалют".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "Roles/АдминистраторСистемы.xml",
                &SourceKind::MetadataXml,
                Some("MetaDataObject")
            ),
            Some("Roles/АдминистраторСистемы".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "FunctionalOptions/ВыполнятьЗамерыПроизводительности.xml",
                &SourceKind::MetadataXml,
                Some("FunctionalOption")
            ),
            Some("FunctionalOptions/ВыполнятьЗамерыПроизводительности".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "FunctionalOptionsParameters/ОбщиеНастройкиУзлов.xml",
                &SourceKind::MetadataXml,
                Some("FunctionalOptionsParameter")
            ),
            Some("FunctionalOptionsParameters/ОбщиеНастройкиУзлов".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "Languages/Русский.xml",
                &SourceKind::MetadataXml,
                Some("Language")
            ),
            Some("Languages/Русский".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "EventSubscriptions/ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных.xml",
                &SourceKind::MetadataXml,
                Some("MetaDataObject")
            ),
            Some(
                "EventSubscriptions/ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных"
                    .to_string()
            )
        );
        assert_eq!(
            infer_object_hint(
                "FilterCriteria/СвязанныеДокументы.xml",
                &SourceKind::MetadataXml,
                Some("MetaDataObject")
            ),
            Some("FilterCriteria/СвязанныеДокументы".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "HTTPServices/exchange_dsl_1_0_0_1.xml",
                &SourceKind::MetadataXml,
                Some("MetaDataObject")
            ),
            Some("HTTPServices/exchange_dsl_1_0_0_1".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "StyleItems/ВажнаяНадписьШрифт.xml",
                &SourceKind::MetadataXml,
                Some("MetaDataObject")
            ),
            Some("StyleItems/ВажнаяНадписьШрифт".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "WebServices/EnterpriseDataExchange_1_0_1_1.xml",
                &SourceKind::MetadataXml,
                Some("MetaDataObject")
            ),
            Some("WebServices/EnterpriseDataExchange_1_0_1_1".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1.xml",
                &SourceKind::MetadataXml,
                Some("MetaDataObject")
            ),
            Some("XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1/Ext/Package.bin",
                &SourceKind::Binary,
                Some("MetaDataObject")
            ),
            Some("XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "SettingsStorages/ХранилищеВариантовОтчетов.xml",
                &SourceKind::MetadataXml,
                Some("SettingsStorage")
            ),
            Some("SettingsStorages/ХранилищеВариантовОтчетов".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "WebServices/Сервис.xml",
                &SourceKind::MetadataXml,
                Some("WebService")
            ),
            Some("WebServices/Сервис".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "SessionParameters/АвторизованныйПользователь.xml",
                &SourceKind::MetadataXml,
                Some("SessionParameter")
            ),
            Some("SessionParameters/АвторизованныйПользователь".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "Tasks/ЗадачаИсполнителя.xml",
                &SourceKind::MetadataXml,
                None
            ),
            Some("Tasks/ЗадачаИсполнителя".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "Tasks/ЗадачаИсполнителя/Commands/ВсеЗадачи/Ext/CommandModule.bsl",
                &SourceKind::Module,
                None
            ),
            Some("Tasks/ЗадачаИсполнителя".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Form.xml",
                &SourceKind::Form,
                Some("Form")
            ),
            Some("Tasks/ЗадачаИсполнителя".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "Tasks/ЗадачаИсполнителя/Ext/Help.xml",
                &SourceKind::MetadataXml,
                Some("Help")
            ),
            Some("Tasks/ЗадачаИсполнителя".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "Tasks/ЗадачаИсполнителя/Ext/Help/ru.html",
                &SourceKind::Other,
                None
            ),
            Some("Tasks/ЗадачаИсполнителя".to_string())
        );
        assert_eq!(
            infer_object_hint(
                "CommandGroups/Органайзер.xml",
                &SourceKind::MetadataXml,
                Some("MetaDataObject")
            ),
            Some("CommandGroups/Органайзер".to_string())
        );
    }

    #[test]
    fn scans_metadata_collection_xml_without_parsing_root_element() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-scan-lazy-xml-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("Catalogs")).unwrap();
        std::fs::write(
            root.join("Catalogs/Invalid.xml"),
            b"<Catalog uuid=\"11111111-1111-4111-8111-111111111111\">",
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let file = manifest
            .files
            .iter()
            .find(|file| file.path == "Catalogs/Invalid.xml")
            .expect("expected invalid catalog xml to be included");
        assert_eq!(file.kind, SourceKind::MetadataXml);
        assert_eq!(file.object_hint.as_deref(), Some("Catalogs/Invalid"));
    }

    #[test]
    fn classifies_metadata_subfiles_as_metadata_xml() {
        assert_eq!(
            classify(
                "Rights.xml".as_ref(),
                "Roles/АдминистраторСистемы/Ext/Rights.xml",
                Some("Rights")
            ),
            SourceKind::MetadataXml
        );
        assert_eq!(
            classify(
                "Schedule.xml".as_ref(),
                "ScheduledJobs/ЗагрузкаКурсовВалют/Ext/Schedule.xml",
                Some("JobSchedule")
            ),
            SourceKind::MetadataXml
        );
        assert_eq!(
            classify(
                "Form.xml".as_ref(),
                "CommonForms/АвтономнаяРабота/Ext/Form.xml",
                Some("Form")
            ),
            SourceKind::Form
        );
        assert_eq!(
            classify(
                "Template.xml".as_ref(),
                "Catalogs/МашиночитаемыеДоверенности/Templates/ПФ_MXL_Доверенность/Ext/Template.xml",
                Some("document")
            ),
            SourceKind::Template
        );
        assert_eq!(
            classify(
                "Help.xml".as_ref(),
                "Catalogs/Валюты/Ext/Help.xml",
                Some("Help")
            ),
            SourceKind::MetadataXml
        );
        assert_eq!(
            classify(
                "SettingsStorage.xml".as_ref(),
                "SettingsStorages/ХранилищеВариантовОтчетов.xml",
                Some("SettingsStorage")
            ),
            SourceKind::MetadataXml
        );
        assert_eq!(
            classify(
                "Predefined.xml".as_ref(),
                "Catalogs/ВидыКонтактнойИнформации/Ext/Predefined.xml",
                Some("PredefinedData")
            ),
            SourceKind::MetadataXml
        );
        assert_eq!(
            classify(
                "Flowchart.xml".as_ref(),
                "BusinessProcesses/Задание/Ext/Flowchart.xml",
                Some("GraphicalSchema")
            ),
            SourceKind::MetadataXml
        );
        assert_eq!(
            classify(
                "CommandInterface.xml".as_ref(),
                "CommonCommands/АвтономнаяРабота/Ext/CommandInterface.xml",
                Some("CommandInterface")
            ),
            SourceKind::MetadataXml
        );
        assert_eq!(
            classify(
                "Help.xml".as_ref(),
                "Tasks/ЗадачаИсполнителя/Ext/Help.xml",
                Some("Help")
            ),
            SourceKind::MetadataXml
        );
        assert_eq!(
            classify(
                "ru.html".as_ref(),
                "Tasks/ЗадачаИсполнителя/Ext/Help/ru.html",
                None
            ),
            SourceKind::Other
        );
        assert_eq!(
            classify(
                "Picture.svg".as_ref(),
                "CommonPictures/ТранспортHTTP/Ext/Picture.svg",
                None
            ),
            SourceKind::Binary
        );
        assert_eq!(
            classify(
                "Picture.zip".as_ref(),
                "CommonPictures/ФорматPDF/Ext/Picture.zip",
                None
            ),
            SourceKind::Binary
        );
        assert_eq!(
            classify(
                "Picture.xml".as_ref(),
                "CommonPictures/Адрес/Ext/Picture.xml",
                Some("ExtPicture")
            ),
            SourceKind::MetadataXml
        );
        assert_eq!(
            classify(
                "Content.xml".as_ref(),
                "ExchangePlans/ОбновлениеИнформационнойБазы/Ext/Content.xml",
                Some("ExchangePlanContent")
            ),
            SourceKind::MetadataXml
        );
        assert_eq!(
            classify("Splash.xml".as_ref(), "Ext/Splash.xml", Some("ExtPicture")),
            SourceKind::MetadataXml
        );
        assert_eq!(
            classify(
                "MainSectionPicture.xml".as_ref(),
                "Ext/MainSectionPicture.xml",
                Some("ExtPicture")
            ),
            SourceKind::MetadataXml
        );
        assert_eq!(
            classify(
                "Package.bin".as_ref(),
                "XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1/Ext/Package.bin",
                None
            ),
            SourceKind::Binary
        );
        assert_eq!(
            classify(
                "CommandModule.bsl".as_ref(),
                "Tasks/ЗадачаИсполнителя/Commands/ВсеЗадачи/Ext/CommandModule.bsl",
                None
            ),
            SourceKind::Module
        );
        assert_eq!(
            classify(
                "Form.xml".as_ref(),
                "Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Form.xml",
                Some("Form")
            ),
            SourceKind::Form
        );
    }

    #[test]
    fn scans_nested_object_and_service_subtrees() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("Tasks/ЗадачаИсполнителя/Commands/ВсеЗадачи/Ext"))
            .unwrap();
        std::fs::create_dir_all(root.join("Tasks/ЗадачаИсполнителя/Ext")).unwrap();
        std::fs::create_dir_all(root.join("BusinessProcesses/Задание/Ext")).unwrap();
        std::fs::create_dir_all(root.join("DefinedTypes")).unwrap();
        std::fs::create_dir_all(root.join("CommonAttributes")).unwrap();
        std::fs::create_dir_all(root.join("AccumulationRegisters/Продажи/Forms/ФормаСписка"))
            .unwrap();
        std::fs::create_dir_all(root.join("AccountingRegisters/Хозрасчеты/Ext")).unwrap();
        std::fs::create_dir_all(root.join("CalculationRegisters/Премии/Commands/Печать/Ext"))
            .unwrap();
        std::fs::create_dir_all(root.join("ChartsOfAccounts/ПланСчетов/Ext")).unwrap();
        std::fs::create_dir_all(root.join("FunctionalOptions")).unwrap();
        std::fs::create_dir_all(root.join("FunctionalOptionsParameters")).unwrap();
        std::fs::create_dir_all(root.join("EventSubscriptions")).unwrap();
        std::fs::create_dir_all(root.join("HTTPServices")).unwrap();
        std::fs::create_dir_all(root.join("CommonCommands/АвтономнаяРабота/Ext")).unwrap();

        std::fs::write(
            root.join("Tasks/ЗадачаИсполнителя.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Task uuid="3ad08f4a-6202-4099-b6cc-bc116e6731a0">
    <Properties>
      <Name>ЗадачаИсполнителя</Name>
    </Properties>
  </Task>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Tasks/ЗадачаИсполнителя/Ext/Help.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Help xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20">
  <Page>ru</Page>
</Help>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Tasks/ЗадачаИсполнителя/Commands/ВсеЗадачи/Ext/CommandModule.bsl"),
            "Procedure Test()\nEndProcedure\n",
        )
        .unwrap();
        std::fs::write(
            root.join("BusinessProcesses/Задание.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <BusinessProcess uuid="c1e32bc3-ef8c-44c0-8a50-7b5fef4c2c29">
    <Properties>
      <Name>Задание</Name>
    </Properties>
  </BusinessProcess>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("BusinessProcesses/Задание/Ext/Flowchart.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<GraphicalSchema xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20"/>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("DefinedTypes/ВладелецДополнительныхСведений.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<DefinedType xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Properties>
    <Name>ВладелецДополнительныхСведений</Name>
  </Properties>
</DefinedType>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("CommonAttributes/ДопРеквизит.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<CommonAttribute xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Properties>
    <Name>ДопРеквизит</Name>
  </Properties>
</CommonAttribute>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("AccumulationRegisters/Продажи.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <AccumulationRegister uuid="11111111-1111-4111-8111-111111111111">
    <Properties>
      <Name>Продажи</Name>
    </Properties>
  </AccumulationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("AccumulationRegisters/Продажи/Forms/ФормаСписка/Form.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20"/>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("AccountingRegisters/Хозрасчеты.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <AccountingRegister uuid="22222222-2222-4222-8222-222222222222">
    <Properties>
      <Name>Хозрасчеты</Name>
    </Properties>
  </AccountingRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("AccountingRegisters/Хозрасчеты/Ext/Help.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Help xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20">
  <Page>ru</Page>
</Help>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("CalculationRegisters/Премии.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <CalculationRegister uuid="33333333-3333-4333-8333-333333333333">
    <Properties>
      <Name>Премии</Name>
    </Properties>
  </CalculationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("CalculationRegisters/Премии/Commands/Печать/Ext/CommandModule.bsl"),
            "Procedure Test()\nEndProcedure\n",
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfAccounts/ПланСчетов.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfAccounts uuid="44444444-4444-4444-8444-444444444444">
    <Properties>
      <Name>ПланСчетов</Name>
    </Properties>
  </ChartOfAccounts>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfAccounts/ПланСчетов/Ext/Help.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Help xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20"/>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("FunctionalOptions/ВыполнятьЗамерыПроизводительности.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <FunctionalOption uuid="d2ae6d0f-6c08-4a70-a8fe-2b1adf77e2f9">
    <Properties>
      <Name>ВыполнятьЗамерыПроизводительности</Name>
    </Properties>
  </FunctionalOption>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("FunctionalOptionsParameters/ОбщиеНастройкиУзлов.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <FunctionalOptionsParameter uuid="f9479915-cdee-40d5-ba53-101132aac672">
    <Properties>
      <Name>ОбщиеНастройкиУзлов</Name>
    </Properties>
  </FunctionalOptionsParameter>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("EventSubscriptions/СобытиеПередЗаписью.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <EventSubscription uuid="54fe3d5c-898c-43be-8a9f-1d5d1f9677a7">
    <Properties>
      <Name>СобытиеПередЗаписью</Name>
    </Properties>
  </EventSubscription>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("HTTPServices/exchange_dsl_1_0_0_1.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <HTTPService uuid="e6a45fd4-b11f-47c8-9d21-4c6bc3f05a4a">
    <Properties>
      <Name>exchange_dsl_1_0_0_1</Name>
    </Properties>
  </HTTPService>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("CommonCommands/АвтономнаяРабота/Ext/CommandInterface.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20"/>
"#,
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "CommonCommands/АвтономнаяРабота/Ext/CommandInterface.xml",
            SourceKind::MetadataXml,
            Some("CommonCommands/АвтономнаяРабота")
        )));
        assert!(files.contains(&(
            "Tasks/ЗадачаИсполнителя.xml",
            SourceKind::MetadataXml,
            Some("Tasks/ЗадачаИсполнителя")
        )));
        assert!(files.contains(&(
            "Tasks/ЗадачаИсполнителя/Ext/Help.xml",
            SourceKind::MetadataXml,
            Some("Tasks/ЗадачаИсполнителя")
        )));
        assert!(files.contains(&(
            "BusinessProcesses/Задание/Ext/Flowchart.xml",
            SourceKind::MetadataXml,
            Some("BusinessProcesses/Задание")
        )));
        assert!(files.contains(&(
            "DefinedTypes/ВладелецДополнительныхСведений.xml",
            SourceKind::MetadataXml,
            Some("DefinedTypes/ВладелецДополнительныхСведений")
        )));
        assert!(files.contains(&(
            "CommonAttributes/ДопРеквизит.xml",
            SourceKind::MetadataXml,
            Some("CommonAttributes/ДопРеквизит")
        )));
        assert!(files.contains(&(
            "AccumulationRegisters/Продажи.xml",
            SourceKind::MetadataXml,
            Some("AccumulationRegisters/Продажи")
        )));
        assert!(files.contains(&(
            "AccumulationRegisters/Продажи/Forms/ФормаСписка/Form.xml",
            SourceKind::Form,
            Some("AccumulationRegisters/Продажи")
        )));
        assert!(files.contains(&(
            "AccountingRegisters/Хозрасчеты.xml",
            SourceKind::MetadataXml,
            Some("AccountingRegisters/Хозрасчеты")
        )));
        assert!(files.contains(&(
            "AccountingRegisters/Хозрасчеты/Ext/Help.xml",
            SourceKind::MetadataXml,
            Some("AccountingRegisters/Хозрасчеты")
        )));
        assert!(files.contains(&(
            "CalculationRegisters/Премии.xml",
            SourceKind::MetadataXml,
            Some("CalculationRegisters/Премии")
        )));
        assert!(files.contains(&(
            "CalculationRegisters/Премии/Commands/Печать/Ext/CommandModule.bsl",
            SourceKind::Module,
            Some("CalculationRegisters/Премии")
        )));
        assert!(files.contains(&(
            "ChartsOfAccounts/ПланСчетов.xml",
            SourceKind::MetadataXml,
            Some("ChartsOfAccounts/ПланСчетов")
        )));
        assert!(files.contains(&(
            "ChartsOfAccounts/ПланСчетов/Ext/Help.xml",
            SourceKind::MetadataXml,
            Some("ChartsOfAccounts/ПланСчетов")
        )));
        assert!(files.contains(&(
            "FunctionalOptions/ВыполнятьЗамерыПроизводительности.xml",
            SourceKind::MetadataXml,
            Some("FunctionalOptions/ВыполнятьЗамерыПроизводительности")
        )));
        assert!(files.contains(&(
            "FunctionalOptionsParameters/ОбщиеНастройкиУзлов.xml",
            SourceKind::MetadataXml,
            Some("FunctionalOptionsParameters/ОбщиеНастройкиУзлов")
        )));
        assert!(files.contains(&(
            "EventSubscriptions/СобытиеПередЗаписью.xml",
            SourceKind::MetadataXml,
            Some("EventSubscriptions/СобытиеПередЗаписью")
        )));
        assert!(files.contains(&(
            "HTTPServices/exchange_dsl_1_0_0_1.xml",
            SourceKind::MetadataXml,
            Some("HTTPServices/exchange_dsl_1_0_0_1")
        )));
        assert!(files.contains(&(
            "Tasks/ЗадачаИсполнителя/Commands/ВсеЗадачи/Ext/CommandModule.bsl",
            SourceKind::Module,
            Some("Tasks/ЗадачаИсполнителя")
        )));
    }

    #[test]
    fn scans_real_register_family_layouts() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-registers-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("AccumulationRegisters/Продажи/Forms/ФормаСписка"))
            .unwrap();
        std::fs::create_dir_all(root.join("AccountingRegisters/Хозрасчеты/Commands/Печать/Ext"))
            .unwrap();
        std::fs::create_dir_all(root.join("CalculationRegisters/Премии/Ext")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfAccounts/ПланСчетов/Forms/ФормаСписка"))
            .unwrap();

        std::fs::write(
            root.join("AccumulationRegisters/Продажи.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <AccumulationRegister uuid="11111111-1111-4111-8111-111111111111">
    <Properties>
      <Name>Продажи</Name>
    </Properties>
  </AccumulationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("AccumulationRegisters/Продажи/Forms/ФормаСписка/Form.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20"/>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("AccountingRegisters/Хозрасчеты.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <AccountingRegister uuid="22222222-2222-4222-8222-222222222222">
    <Properties>
      <Name>Хозрасчеты</Name>
    </Properties>
  </AccountingRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("AccountingRegisters/Хозрасчеты/Commands/Печать/Ext/CommandModule.bsl"),
            "Procedure Test()\nEndProcedure\n",
        )
        .unwrap();
        std::fs::write(
            root.join("CalculationRegisters/Премии.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <CalculationRegister uuid="33333333-3333-4333-8333-333333333333">
    <Properties>
      <Name>Премии</Name>
    </Properties>
  </CalculationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("CalculationRegisters/Премии/Ext/Help.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Help xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20"/>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfAccounts/ПланСчетов.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfAccounts uuid="44444444-4444-4444-8444-444444444444">
    <Properties>
      <Name>ПланСчетов</Name>
    </Properties>
  </ChartOfAccounts>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfAccounts/ПланСчетов/Forms/ФормаСписка/Form.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20"/>
"#,
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "AccumulationRegisters/Продажи.xml",
            SourceKind::MetadataXml,
            Some("AccumulationRegisters/Продажи")
        )));
        assert!(files.contains(&(
            "AccumulationRegisters/Продажи/Forms/ФормаСписка/Form.xml",
            SourceKind::Form,
            Some("AccumulationRegisters/Продажи")
        )));
        assert!(files.contains(&(
            "AccountingRegisters/Хозрасчеты.xml",
            SourceKind::MetadataXml,
            Some("AccountingRegisters/Хозрасчеты")
        )));
        assert!(files.contains(&(
            "AccountingRegisters/Хозрасчеты/Commands/Печать/Ext/CommandModule.bsl",
            SourceKind::Module,
            Some("AccountingRegisters/Хозрасчеты")
        )));
        assert!(files.contains(&(
            "CalculationRegisters/Премии.xml",
            SourceKind::MetadataXml,
            Some("CalculationRegisters/Премии")
        )));
        assert!(files.contains(&(
            "CalculationRegisters/Премии/Ext/Help.xml",
            SourceKind::MetadataXml,
            Some("CalculationRegisters/Премии")
        )));
        assert!(files.contains(&(
            "ChartsOfAccounts/ПланСчетов.xml",
            SourceKind::MetadataXml,
            Some("ChartsOfAccounts/ПланСчетов")
        )));
        assert!(files.contains(&(
            "ChartsOfAccounts/ПланСчетов/Forms/ФормаСписка/Form.xml",
            SourceKind::Form,
            Some("ChartsOfAccounts/ПланСчетов")
        )));
    }

    #[test]
    fn scans_real_chart_of_accounts_layouts() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-chart-of-accounts-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("ChartsOfAccounts/ПланСчетов/Ext")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfAccounts/ПланСчетов/Forms/ФормаСписка"))
            .unwrap();

        std::fs::write(
            root.join("ChartsOfAccounts/ПланСчетов.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfAccounts uuid="44444444-4444-4444-8444-444444444444">
    <Properties>
      <Name>ПланСчетов</Name>
    </Properties>
  </ChartOfAccounts>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfAccounts/ПланСчетов/Ext/Help.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Help xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20"/>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfAccounts/ПланСчетов/Forms/ФормаСписка/Form.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20"/>
"#,
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "ChartsOfAccounts/ПланСчетов.xml",
            SourceKind::MetadataXml,
            Some("ChartsOfAccounts/ПланСчетов")
        )));
        assert!(files.contains(&(
            "ChartsOfAccounts/ПланСчетов/Ext/Help.xml",
            SourceKind::MetadataXml,
            Some("ChartsOfAccounts/ПланСчетов")
        )));
        assert!(files.contains(&(
            "ChartsOfAccounts/ПланСчетов/Forms/ФормаСписка/Form.xml",
            SourceKind::Form,
            Some("ChartsOfAccounts/ПланСчетов")
        )));
    }

    #[test]
    fn scans_real_chart_of_calculation_family_layouts() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-charts-of-calculation-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("ChartsOfCalculationTypes/ВидыРасчета/Forms/ФормаСписка"))
            .unwrap();
        std::fs::create_dir_all(root.join("ChartsOfCalculationRegisters/Начисления/Ext")).unwrap();

        std::fs::write(
            root.join("ChartsOfCalculationTypes/ВидыРасчета.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfCalculationTypes uuid="55555555-5555-4555-8555-555555555555">
    <Properties>
      <Name>ВидыРасчета</Name>
    </Properties>
  </ChartOfCalculationTypes>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfCalculationTypes/ВидыРасчета/Forms/ФормаСписка/Form.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20"/>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfCalculationRegisters/Начисления.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfCalculationRegisters uuid="66666666-6666-4666-8666-666666666666">
    <Properties>
      <Name>Начисления</Name>
    </Properties>
  </ChartOfCalculationRegisters>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfCalculationRegisters/Начисления/Ext/Help.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Help xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20"/>
"#,
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| (file.path.as_str(), file.kind.clone(), file.object_hint.as_deref()))
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "ChartsOfCalculationTypes/ВидыРасчета.xml",
            SourceKind::MetadataXml,
            Some("ChartsOfCalculationTypes/ВидыРасчета")
        )));
        assert!(files.contains(&(
            "ChartsOfCalculationTypes/ВидыРасчета/Forms/ФормаСписка/Form.xml",
            SourceKind::Form,
            Some("ChartsOfCalculationTypes/ВидыРасчета")
        )));
        assert!(files.contains(&(
            "ChartsOfCalculationRegisters/Начисления.xml",
            SourceKind::MetadataXml,
            Some("ChartsOfCalculationRegisters/Начисления")
        )));
        assert!(files.contains(&(
            "ChartsOfCalculationRegisters/Начисления/Ext/Help.xml",
            SourceKind::MetadataXml,
            Some("ChartsOfCalculationRegisters/Начисления")
        )));
    }

    #[test]
    fn scans_real_common_form_template_and_package_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-lab-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("CommonForms")).unwrap();
        std::fs::create_dir_all(root.join("CommonTemplates")).unwrap();
        std::fs::create_dir_all(root.join("CommonForms/АвтономнаяРабота/Ext/Form")).unwrap();
        std::fs::create_dir_all(root.join(
            "CommonTemplates/ИнструкцияДляУстановкиКодаДляПредопределенногоУзла_ru/Ext/Template",
        ))
        .unwrap();
        std::fs::create_dir_all(
            root.join("CommonTemplates/КомпонентаСканированияДокументов_3_0_1_1033/Ext"),
        )
        .unwrap();
        std::fs::create_dir_all(root.join("CommonTemplates/ШтампЭлектроннойПодписиOfficeOpen/Ext"))
            .unwrap();
        std::fs::create_dir_all(
            root.join("XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1/Ext"),
        )
        .unwrap();

        std::fs::copy(
            lab_root.join("CommonForms/АвтономнаяРабота.xml"),
            root.join("CommonForms/АвтономнаяРабота.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonForms/АвтономнаяРабота/Ext/Form.xml"),
            root.join("CommonForms/АвтономнаяРабота/Ext/Form.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonForms/АвтономнаяРабота/Ext/Form/Module.bsl"),
            root.join("CommonForms/АвтономнаяРабота/Ext/Form/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonTemplates/ВидыДокументовУдостоверяющихЛичность.xml"),
            root.join("CommonTemplates/ВидыДокументовУдостоверяющихЛичность.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonTemplates/ИнструкцияДляУстановкиКодаДляПредопределенногоУзла_ru/Ext/Template.xml"),
            root.join("CommonTemplates/ИнструкцияДляУстановкиКодаДляПредопределенногоУзла_ru/Ext/Template.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonTemplates/ИнструкцияДляУстановкиКодаДляПредопределенногоУзла_ru/Ext/Template/ru.html"),
            root.join("CommonTemplates/ИнструкцияДляУстановкиКодаДляПредопределенногоУзла_ru/Ext/Template/ru.html"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "CommonTemplates/КомпонентаСканированияДокументов_3_0_1_1033/Ext/Template.bin",
            ),
            root.join(
                "CommonTemplates/КомпонентаСканированияДокументов_3_0_1_1033/Ext/Template.bin",
            ),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonTemplates/ШтампЭлектроннойПодписиOfficeOpen/Ext/Template.txt"),
            root.join("CommonTemplates/ШтампЭлектроннойПодписиOfficeOpen/Ext/Template.txt"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1.xml"),
            root.join("XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1/Ext/Package.bin"),
            root.join("XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1/Ext/Package.bin"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "CommonForms/АвтономнаяРабота.xml",
            SourceKind::MetadataXml,
            Some("CommonForms/АвтономнаяРабота")
        )));
        assert!(files.contains(&(
            "CommonForms/АвтономнаяРабота/Ext/Form.xml",
            SourceKind::Form,
            Some("CommonForms/АвтономнаяРабота")
        )));
        assert!(files.contains(&(
            "CommonForms/АвтономнаяРабота/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("CommonForms/АвтономнаяРабота")
        )));
        assert!(files.contains(&(
            "CommonTemplates/ВидыДокументовУдостоверяющихЛичность.xml",
            SourceKind::MetadataXml,
            Some("CommonTemplates/ВидыДокументовУдостоверяющихЛичность")
        )));
        assert!(files.contains(&(
            "CommonTemplates/ИнструкцияДляУстановкиКодаДляПредопределенногоУзла_ru/Ext/Template.xml",
            SourceKind::Template,
            Some("CommonTemplates/ИнструкцияДляУстановкиКодаДляПредопределенногоУзла_ru")
        )));
        assert!(files.contains(&(
            "CommonTemplates/ИнструкцияДляУстановкиКодаДляПредопределенногоУзла_ru/Ext/Template/ru.html",
            SourceKind::Other,
            Some("CommonTemplates/ИнструкцияДляУстановкиКодаДляПредопределенногоУзла_ru")
        )));
        assert!(files.contains(&(
            "CommonTemplates/КомпонентаСканированияДокументов_3_0_1_1033/Ext/Template.bin",
            SourceKind::Binary,
            Some("CommonTemplates/КомпонентаСканированияДокументов_3_0_1_1033")
        )));
        assert!(files.contains(&(
            "CommonTemplates/ШтампЭлектроннойПодписиOfficeOpen/Ext/Template.txt",
            SourceKind::Other,
            Some("CommonTemplates/ШтампЭлектроннойПодписиOfficeOpen")
        )));
        assert!(files.contains(&(
            "XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1.xml",
            SourceKind::MetadataXml,
            Some("XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1")
        )));
        assert!(files.contains(&(
            "XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1/Ext/Package.bin",
            SourceKind::Binary,
            Some("XDTOPackages/АдминистрированиеОбменаДанными_2_4_5_1")
        )));
    }

    #[test]
    fn scans_real_chart_of_characteristic_types_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-chart-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join(
            "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаЭлемента/Ext/Form",
        ))
        .unwrap();
        std::fs::create_dir_all(
            root.join(
                "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаСписка/Ext/Form",
            ),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/Help"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаЭлемента/Ext/Form"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаСписка/Ext/Form"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/РазблокированиеРеквизитов/Ext/Form"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Ext/Help"),
        )
        .unwrap();

        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач.xml"),
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root
                .join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаЭлемента.xml"),
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаЭлемента.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаЭлемента/Ext/Form.xml"),
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаЭлемента/Ext/Form.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаЭлемента/Ext/Form/Module.bsl"),
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаЭлемента/Ext/Form/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root
                .join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаСписка.xml"),
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаСписка.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаСписка/Ext/Form.xml",
            ),
            root.join(
                "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаСписка/Ext/Form.xml",
            ),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаСписка/Ext/Form/Module.bsl"),
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаСписка/Ext/Form/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/Predefined.xml"),
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/Predefined.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/ObjectModule.bsl"),
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/ObjectModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root
                .join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/ManagerModule.bsl"),
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/ManagerModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/Help.xml"),
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/Help.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/Help/ru.html"),
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/Help/ru.html"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения.xml"),
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаЭлемента.xml"),
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаЭлемента.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаЭлемента/Ext/Form.xml"),
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаЭлемента/Ext/Form.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаЭлемента/Ext/Form/Module.bsl"),
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаЭлемента/Ext/Form/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаСписка.xml"),
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаСписка.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаСписка/Ext/Form.xml"),
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаСписка/Ext/Form.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаСписка/Ext/Form/Module.bsl"),
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаСписка/Ext/Form/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/РазблокированиеРеквизитов/Ext/Form.xml"),
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/РазблокированиеРеквизитов/Ext/Form.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Ext/ObjectModule.bsl"),
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Ext/ObjectModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Ext/ManagerModule.bsl"),
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Ext/ManagerModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Ext/Help.xml"),
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Ext/Help.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Ext/Help/ru.html"),
            root.join("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Ext/Help/ru.html"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач.xml",
            SourceKind::MetadataXml,
            Some("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаЭлемента.xml",
            SourceKind::Form,
            Some("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаЭлемента/Ext/Form.xml",
            SourceKind::Form,
            Some("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаЭлемента/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаСписка.xml",
            SourceKind::Form,
            Some("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаСписка/Ext/Form.xml",
            SourceKind::Form,
            Some("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Forms/ФормаСписка/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/Predefined.xml",
            SourceKind::MetadataXml,
            Some("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/ObjectModule.bsl",
            SourceKind::Module,
            Some("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/ManagerModule.bsl",
            SourceKind::Module,
            Some("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/Help.xml",
            SourceKind::MetadataXml,
            Some("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач/Ext/Help/ru.html",
            SourceKind::Other,
            Some("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения.xml",
            SourceKind::MetadataXml,
            Some("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаЭлемента.xml",
            SourceKind::Form,
            Some("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаЭлемента/Ext/Form.xml",
            SourceKind::Form,
            Some("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаЭлемента/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаСписка.xml",
            SourceKind::Form,
            Some("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаСписка/Ext/Form.xml",
            SourceKind::Form,
            Some("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/ФормаСписка/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Forms/РазблокированиеРеквизитов/Ext/Form.xml",
            SourceKind::Form,
            Some("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Ext/ObjectModule.bsl",
            SourceKind::Module,
            Some("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Ext/ManagerModule.bsl",
            SourceKind::Module,
            Some("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Ext/Help.xml",
            SourceKind::MetadataXml,
            Some("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения")
        )));
        assert!(files.contains(&(
            "ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения/Ext/Help/ru.html",
            SourceKind::Other,
            Some("ChartsOfCharacteristicTypes/ДополнительныеРеквизитыИСведения")
        )));
    }

    #[test]
    fn scans_real_document_journal_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-docjournal-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(
            root.join("DocumentJournals/Взаимодействия/Forms/ФормаСписка/Ext/Form"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействия/Ext"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join(
                "DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействияКонтакт/Ext",
            ),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("DocumentJournals/Взаимодействия/Commands/ПозвонитьПоКонтакту/Ext"),
        )
        .unwrap();

        std::fs::copy(
            lab_root.join("DocumentJournals/Взаимодействия.xml"),
            root.join("DocumentJournals/Взаимодействия.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DocumentJournals/Взаимодействия/Forms/ФормаСписка.xml"),
            root.join("DocumentJournals/Взаимодействия/Forms/ФормаСписка.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DocumentJournals/Взаимодействия/Forms/ФормаСписка/Ext/Form.xml"),
            root.join("DocumentJournals/Взаимодействия/Forms/ФормаСписка/Ext/Form.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DocumentJournals/Взаимодействия/Forms/ФормаСписка/Ext/Help.xml"),
            root.join("DocumentJournals/Взаимодействия/Forms/ФормаСписка/Ext/Help.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DocumentJournals/Взаимодействия/Forms/ФормаСписка/Ext/Form/Module.bsl"),
            root.join("DocumentJournals/Взаимодействия/Forms/ФормаСписка/Ext/Form/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействия.xml"),
            root.join("DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействия.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействияКонтакт.xml",
            ),
            root.join(
                "DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействияКонтакт.xml",
            ),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействия/Ext/Template.xml",
            ),
            root.join("DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействия/Ext/Template.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействияКонтакт/Ext/Template.xml",
            ),
            root.join(
                "DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействияКонтакт/Ext/Template.xml",
            ),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DocumentJournals/Взаимодействия/Commands/ПозвонитьПоКонтакту/Ext/CommandModule.bsl"),
            root.join("DocumentJournals/Взаимодействия/Commands/ПозвонитьПоКонтакту/Ext/CommandModule.bsl"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "DocumentJournals/Взаимодействия.xml",
            SourceKind::MetadataXml,
            Some("DocumentJournals/Взаимодействия")
        )));
        assert!(files.contains(&(
            "DocumentJournals/Взаимодействия/Forms/ФормаСписка.xml",
            SourceKind::Form,
            Some("DocumentJournals/Взаимодействия")
        )));
        assert!(files.contains(&(
            "DocumentJournals/Взаимодействия/Forms/ФормаСписка/Ext/Form.xml",
            SourceKind::Form,
            Some("DocumentJournals/Взаимодействия")
        )));
        assert!(files.contains(&(
            "DocumentJournals/Взаимодействия/Forms/ФормаСписка/Ext/Help.xml",
            SourceKind::Form,
            Some("DocumentJournals/Взаимодействия")
        )));
        assert!(files.contains(&(
            "DocumentJournals/Взаимодействия/Forms/ФормаСписка/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("DocumentJournals/Взаимодействия")
        )));
        assert!(files.contains(&(
            "DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействия.xml",
            SourceKind::Template,
            Some("DocumentJournals/Взаимодействия")
        )));
        assert!(files.contains(&(
            "DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействияКонтакт.xml",
            SourceKind::Template,
            Some("DocumentJournals/Взаимодействия")
        )));
        assert!(files.contains(&(
            "DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействия/Ext/Template.xml",
            SourceKind::Template,
            Some("DocumentJournals/Взаимодействия")
        )));
        assert!(files.contains(&(
            "DocumentJournals/Взаимодействия/Templates/СхемаОтборВзаимодействияКонтакт/Ext/Template.xml",
            SourceKind::Template,
            Some("DocumentJournals/Взаимодействия")
        )));
        assert!(files.contains(&(
            "DocumentJournals/Взаимодействия/Commands/ПозвонитьПоКонтакту/Ext/CommandModule.bsl",
            SourceKind::Module,
            Some("DocumentJournals/Взаимодействия")
        )));
    }

    #[test]
    fn scans_real_subsystem_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-subsystem-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(
            root.join("Subsystems/СтандартныеПодсистемы/Subsystems/УправлениеДоступом/Ext/Help"),
        )
        .unwrap();

        std::fs::copy(
            lab_root.join("Subsystems/СтандартныеПодсистемы.xml"),
            root.join("Subsystems/СтандартныеПодсистемы.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Subsystems/СтандартныеПодсистемы/Subsystems/УправлениеДоступом.xml"),
            root.join("Subsystems/СтандартныеПодсистемы/Subsystems/УправлениеДоступом.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "Subsystems/СтандартныеПодсистемы/Subsystems/УправлениеДоступом/Ext/Help.xml",
            ),
            root.join(
                "Subsystems/СтандартныеПодсистемы/Subsystems/УправлениеДоступом/Ext/Help.xml",
            ),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "Subsystems/СтандартныеПодсистемы/Subsystems/УправлениеДоступом/Ext/Help/ru.html",
            ),
            root.join(
                "Subsystems/СтандартныеПодсистемы/Subsystems/УправлениеДоступом/Ext/Help/ru.html",
            ),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "Subsystems/СтандартныеПодсистемы.xml",
            SourceKind::MetadataXml,
            Some("Subsystems/СтандартныеПодсистемы")
        )));
        assert!(files.contains(&(
            "Subsystems/СтандартныеПодсистемы/Subsystems/УправлениеДоступом.xml",
            SourceKind::MetadataXml,
            Some("Subsystems/СтандартныеПодсистемы")
        )));
        assert!(files.contains(&(
            "Subsystems/СтандартныеПодсистемы/Subsystems/УправлениеДоступом/Ext/Help.xml",
            SourceKind::MetadataXml,
            Some("Subsystems/СтандартныеПодсистемы")
        )));
        assert!(files.contains(&(
            "Subsystems/СтандартныеПодсистемы/Subsystems/УправлениеДоступом/Ext/Help/ru.html",
            SourceKind::Other,
            Some("Subsystems/СтандартныеПодсистемы")
        )));
    }

    #[test]
    fn scans_real_task_form_and_command_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-task-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext"))
            .unwrap();
        std::fs::create_dir_all(root.join("Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Form"))
            .unwrap();
        std::fs::create_dir_all(root.join("Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Help"))
            .unwrap();
        std::fs::create_dir_all(root.join("Tasks/ЗадачаИсполнителя/Commands/ВсеЗадачи/Ext"))
            .unwrap();

        std::fs::copy(
            lab_root.join("Tasks/ЗадачаИсполнителя.xml"),
            root.join("Tasks/ЗадачаИсполнителя.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Form.xml"),
            root.join("Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Form.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Help.xml"),
            root.join("Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Help.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Form/Module.bsl"),
            root.join("Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Form/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Help/ru.html"),
            root.join("Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Help/ru.html"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Tasks/ЗадачаИсполнителя/Commands/ВсеЗадачи/Ext/CommandModule.bsl"),
            root.join("Tasks/ЗадачаИсполнителя/Commands/ВсеЗадачи/Ext/CommandModule.bsl"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "Tasks/ЗадачаИсполнителя.xml",
            SourceKind::MetadataXml,
            Some("Tasks/ЗадачаИсполнителя")
        )));
        assert!(files.contains(&(
            "Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Form.xml",
            SourceKind::Form,
            Some("Tasks/ЗадачаИсполнителя")
        )));
        assert!(files.contains(&(
            "Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Help.xml",
            SourceKind::Form,
            Some("Tasks/ЗадачаИсполнителя")
        )));
        assert!(files.contains(&(
            "Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("Tasks/ЗадачаИсполнителя")
        )));
        assert!(files.contains(&(
            "Tasks/ЗадачаИсполнителя/Forms/ФормаЗадачи/Ext/Help/ru.html",
            SourceKind::Form,
            Some("Tasks/ЗадачаИсполнителя")
        )));
        assert!(files.contains(&(
            "Tasks/ЗадачаИсполнителя/Commands/ВсеЗадачи/Ext/CommandModule.bsl",
            SourceKind::Module,
            Some("Tasks/ЗадачаИсполнителя")
        )));
    }

    #[test]
    fn scans_real_data_processor_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-data-processor-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Commands/ЗагрузкаДанныхEnterpriseData/Ext"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Ext/Help"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Form"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Help"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/Форма/Ext/Form"),
        )
        .unwrap();
        std::fs::create_dir_all(root.join(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Templates/СхемаКомпоновкиДанных/Ext",
        ))
        .unwrap();

        std::fs::copy(
            lab_root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData.xml"),
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Ext/Help.xml"),
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Ext/Help.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Ext/ObjectModule.bsl"),
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Ext/ObjectModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Ext/Help/ru.html"),
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Ext/Help/ru.html"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Commands/ЗагрузкаДанныхEnterpriseData/Ext/CommandModule.bsl",
            ),
            root.join(
                "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Commands/ЗагрузкаДанныхEnterpriseData/Ext/CommandModule.bsl",
            ),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора.xml"),
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Form.xml"),
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Form.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Help.xml"),
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Help.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Form/Module.bsl",
            ),
            root.join(
                "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Form/Module.bsl",
            ),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Help/ru.html",
            ),
            root.join(
                "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Help/ru.html",
            ),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/Форма.xml"),
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/Форма.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/Форма/Ext/Form.xml"),
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/Форма/Ext/Form.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/Форма/Ext/Form/Module.bsl",
            ),
            root.join(
                "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/Форма/Ext/Form/Module.bsl",
            ),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Templates/СхемаКомпоновкиДанных.xml",
            ),
            root.join(
                "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Templates/СхемаКомпоновкиДанных.xml",
            ),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Templates/СхемаКомпоновкиДанных/Ext/Template.xml"),
            root.join("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Templates/СхемаКомпоновкиДанных/Ext/Template.xml"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData.xml",
            SourceKind::MetadataXml,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Ext/Help.xml",
            SourceKind::MetadataXml,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Ext/ObjectModule.bsl",
            SourceKind::Module,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Ext/Help/ru.html",
            SourceKind::Other,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Commands/ЗагрузкаДанныхEnterpriseData/Ext/CommandModule.bsl",
            SourceKind::Module,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора.xml",
            SourceKind::Form,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Form.xml",
            SourceKind::Form,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Help.xml",
            SourceKind::Form,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/РедактированиеПериодаИОтбора/Ext/Help/ru.html",
            SourceKind::Form,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/Форма.xml",
            SourceKind::Form,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/Форма/Ext/Form.xml",
            SourceKind::Form,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Forms/Форма/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Templates/СхемаКомпоновкиДанных.xml",
            SourceKind::Template,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
        assert!(files.contains(&(
            "DataProcessors/ВыгрузкаЗагрузкаEnterpriseData/Templates/СхемаКомпоновкиДанных/Ext/Template.xml",
            SourceKind::Template,
            Some("DataProcessors/ВыгрузкаЗагрузкаEnterpriseData")
        )));
    }

    #[test]
    fn scans_real_report_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-report-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("Reports/БизнесПроцессы/Ext/Help")).unwrap();
        std::fs::create_dir_all(root.join("Reports/БизнесПроцессы/Templates/Макет/Ext")).unwrap();

        std::fs::copy(
            lab_root.join("Reports/БизнесПроцессы.xml"),
            root.join("Reports/БизнесПроцессы.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Reports/БизнесПроцессы/Ext/Help.xml"),
            root.join("Reports/БизнесПроцессы/Ext/Help.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Reports/БизнесПроцессы/Ext/ManagerModule.bsl"),
            root.join("Reports/БизнесПроцессы/Ext/ManagerModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Reports/БизнесПроцессы/Ext/ObjectModule.bsl"),
            root.join("Reports/БизнесПроцессы/Ext/ObjectModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Reports/БизнесПроцессы/Ext/Help/ru.html"),
            root.join("Reports/БизнесПроцессы/Ext/Help/ru.html"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Reports/БизнесПроцессы/Templates/Макет.xml"),
            root.join("Reports/БизнесПроцессы/Templates/Макет.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Reports/БизнесПроцессы/Templates/Макет/Ext/Template.xml"),
            root.join("Reports/БизнесПроцессы/Templates/Макет/Ext/Template.xml"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "Reports/БизнесПроцессы.xml",
            SourceKind::MetadataXml,
            Some("Reports/БизнесПроцессы")
        )));
        assert!(files.contains(&(
            "Reports/БизнесПроцессы/Ext/Help.xml",
            SourceKind::MetadataXml,
            Some("Reports/БизнесПроцессы")
        )));
        assert!(files.contains(&(
            "Reports/БизнесПроцессы/Ext/ManagerModule.bsl",
            SourceKind::Module,
            Some("Reports/БизнесПроцессы")
        )));
        assert!(files.contains(&(
            "Reports/БизнесПроцессы/Ext/ObjectModule.bsl",
            SourceKind::Module,
            Some("Reports/БизнесПроцессы")
        )));
        assert!(files.contains(&(
            "Reports/БизнесПроцессы/Ext/Help/ru.html",
            SourceKind::Other,
            Some("Reports/БизнесПроцессы")
        )));
        assert!(files.contains(&(
            "Reports/БизнесПроцессы/Templates/Макет.xml",
            SourceKind::Template,
            Some("Reports/БизнесПроцессы")
        )));
        assert!(files.contains(&(
            "Reports/БизнесПроцессы/Templates/Макет/Ext/Template.xml",
            SourceKind::Template,
            Some("Reports/БизнесПроцессы")
        )));
    }

    #[test]
    fn scans_real_exchange_plan_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-exchange-plan-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("ExchangePlans/ОбновлениеИнформационнойБазы/Ext"))
            .unwrap();

        std::fs::copy(
            lab_root.join("ExchangePlans/ОбновлениеИнформационнойБазы.xml"),
            root.join("ExchangePlans/ОбновлениеИнформационнойБазы.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ExchangePlans/ОбновлениеИнформационнойБазы/Ext/Content.xml"),
            root.join("ExchangePlans/ОбновлениеИнформационнойБазы/Ext/Content.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ExchangePlans/ОбновлениеИнформационнойБазы/Ext/ManagerModule.bsl"),
            root.join("ExchangePlans/ОбновлениеИнформационнойБазы/Ext/ManagerModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ExchangePlans/ОбновлениеИнформационнойБазы/Ext/ObjectModule.bsl"),
            root.join("ExchangePlans/ОбновлениеИнформационнойБазы/Ext/ObjectModule.bsl"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "ExchangePlans/ОбновлениеИнформационнойБазы.xml",
            SourceKind::MetadataXml,
            Some("ExchangePlans/ОбновлениеИнформационнойБазы")
        )));
        assert!(files.contains(&(
            "ExchangePlans/ОбновлениеИнформационнойБазы/Ext/Content.xml",
            SourceKind::MetadataXml,
            Some("ExchangePlans/ОбновлениеИнформационнойБазы")
        )));
        assert!(files.contains(&(
            "ExchangePlans/ОбновлениеИнформационнойБазы/Ext/ManagerModule.bsl",
            SourceKind::Module,
            Some("ExchangePlans/ОбновлениеИнформационнойБазы")
        )));
        assert!(files.contains(&(
            "ExchangePlans/ОбновлениеИнформационнойБазы/Ext/ObjectModule.bsl",
            SourceKind::Module,
            Some("ExchangePlans/ОбновлениеИнформационнойБазы")
        )));
    }

    #[test]
    fn scans_real_enum_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-enum-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("Enums/ВариантыОтображенияМеток/Ext")).unwrap();

        std::fs::copy(
            lab_root.join("Enums/ВариантыОтображенияМеток.xml"),
            root.join("Enums/ВариантыОтображенияМеток.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Enums/ВариантыОтображенияМеток/Ext/ManagerModule.bsl"),
            root.join("Enums/ВариантыОтображенияМеток/Ext/ManagerModule.bsl"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "Enums/ВариантыОтображенияМеток.xml",
            SourceKind::MetadataXml,
            Some("Enums/ВариантыОтображенияМеток")
        )));
        assert!(files.contains(&(
            "Enums/ВариантыОтображенияМеток/Ext/ManagerModule.bsl",
            SourceKind::Module,
            Some("Enums/ВариантыОтображенияМеток")
        )));
    }

    #[test]
    fn scans_real_catalog_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-catalog-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("Catalogs/Валюты/Ext/Help")).unwrap();
        std::fs::create_dir_all(root.join("Catalogs/Валюты/Forms/ФормаСписка/Ext/Form")).unwrap();
        std::fs::create_dir_all(root.join("Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Form")).unwrap();
        std::fs::create_dir_all(root.join("Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Help")).unwrap();

        std::fs::copy(
            lab_root.join("Catalogs/Валюты.xml"),
            root.join("Catalogs/Валюты.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Catalogs/Валюты/Ext/Help.xml"),
            root.join("Catalogs/Валюты/Ext/Help.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Catalogs/Валюты/Ext/ManagerModule.bsl"),
            root.join("Catalogs/Валюты/Ext/ManagerModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Catalogs/Валюты/Ext/ObjectModule.bsl"),
            root.join("Catalogs/Валюты/Ext/ObjectModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Catalogs/Валюты/Ext/Help/ru.html"),
            root.join("Catalogs/Валюты/Ext/Help/ru.html"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Catalogs/Валюты/Forms/ФормаСписка.xml"),
            root.join("Catalogs/Валюты/Forms/ФормаСписка.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Catalogs/Валюты/Forms/ФормаСписка/Ext/Form.xml"),
            root.join("Catalogs/Валюты/Forms/ФормаСписка/Ext/Form.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Catalogs/Валюты/Forms/ФормаСписка/Ext/Form/Module.bsl"),
            root.join("Catalogs/Валюты/Forms/ФормаСписка/Ext/Form/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Catalogs/Валюты/Forms/ФормаЭлемента.xml"),
            root.join("Catalogs/Валюты/Forms/ФормаЭлемента.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Form.xml"),
            root.join("Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Form.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Help.xml"),
            root.join("Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Help.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Form/Module.bsl"),
            root.join("Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Form/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Help/ru.html"),
            root.join("Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Help/ru.html"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "Catalogs/Валюты.xml",
            SourceKind::MetadataXml,
            Some("Catalogs/Валюты")
        )));
        assert!(files.contains(&(
            "Catalogs/Валюты/Ext/Help.xml",
            SourceKind::MetadataXml,
            Some("Catalogs/Валюты")
        )));
        assert!(files.contains(&(
            "Catalogs/Валюты/Ext/ManagerModule.bsl",
            SourceKind::Module,
            Some("Catalogs/Валюты")
        )));
        assert!(files.contains(&(
            "Catalogs/Валюты/Ext/ObjectModule.bsl",
            SourceKind::Module,
            Some("Catalogs/Валюты")
        )));
        assert!(files.contains(&(
            "Catalogs/Валюты/Ext/Help/ru.html",
            SourceKind::Other,
            Some("Catalogs/Валюты")
        )));
        assert!(files.contains(&(
            "Catalogs/Валюты/Forms/ФормаСписка.xml",
            SourceKind::Form,
            Some("Catalogs/Валюты")
        )));
        assert!(files.contains(&(
            "Catalogs/Валюты/Forms/ФормаСписка/Ext/Form.xml",
            SourceKind::Form,
            Some("Catalogs/Валюты")
        )));
        assert!(files.contains(&(
            "Catalogs/Валюты/Forms/ФормаСписка/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("Catalogs/Валюты")
        )));
        assert!(files.contains(&(
            "Catalogs/Валюты/Forms/ФормаЭлемента.xml",
            SourceKind::Form,
            Some("Catalogs/Валюты")
        )));
        assert!(files.contains(&(
            "Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Form.xml",
            SourceKind::Form,
            Some("Catalogs/Валюты")
        )));
        assert!(files.contains(&(
            "Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Help.xml",
            SourceKind::Form,
            Some("Catalogs/Валюты")
        )));
        assert!(files.contains(&(
            "Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("Catalogs/Валюты")
        )));
        assert!(files.contains(&(
            "Catalogs/Валюты/Forms/ФормаЭлемента/Ext/Help/ru.html",
            SourceKind::Form,
            Some("Catalogs/Валюты")
        )));
    }

    #[test]
    fn scans_real_information_register_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-information-register-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("InformationRegisters/ВерсииОбъектов/Ext")).unwrap();
        std::fs::create_dir_all(
            root.join("InformationRegisters/ВерсииОбъектов/Forms/ВыборРеквизитовОбъекта/Ext/Form"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Form"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Help"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("InformationRegisters/ВерсииОбъектов/Templates/СтандартныйМакетПредставленияОбъекта/Ext"),
        )
        .unwrap();

        std::fs::copy(
            lab_root.join("InformationRegisters/ВерсииОбъектов.xml"),
            root.join("InformationRegisters/ВерсииОбъектов.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("InformationRegisters/ВерсииОбъектов/Ext/ManagerModule.bsl"),
            root.join("InformationRegisters/ВерсииОбъектов/Ext/ManagerModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("InformationRegisters/ВерсииОбъектов/Ext/RecordSetModule.bsl"),
            root.join("InformationRegisters/ВерсииОбъектов/Ext/RecordSetModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("InformationRegisters/ВерсииОбъектов/Forms/ВыборРеквизитовОбъекта.xml"),
            root.join("InformationRegisters/ВерсииОбъектов/Forms/ВыборРеквизитовОбъекта.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "InformationRegisters/ВерсииОбъектов/Forms/ВыборРеквизитовОбъекта/Ext/Form.xml",
            ),
            root.join(
                "InformationRegisters/ВерсииОбъектов/Forms/ВыборРеквизитовОбъекта/Ext/Form.xml",
            ),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("InformationRegisters/ВерсииОбъектов/Forms/ВыборРеквизитовОбъекта/Ext/Form/Module.bsl"),
            root.join("InformationRegisters/ВерсииОбъектов/Forms/ВыборРеквизитовОбъекта/Ext/Form/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи.xml"),
            root.join("InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Form.xml"),
            root.join("InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Form.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Help.xml"),
            root.join("InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Help.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root
                .join("InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Form/Module.bsl"),
            root.join("InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Form/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Help/ru.html"),
            root.join("InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Help/ru.html"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "InformationRegisters/ВерсииОбъектов/Templates/СтандартныйМакетПредставленияОбъекта.xml",
            ),
            root.join(
                "InformationRegisters/ВерсииОбъектов/Templates/СтандартныйМакетПредставленияОбъекта.xml",
            ),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join(
                "InformationRegisters/ВерсииОбъектов/Templates/СтандартныйМакетПредставленияОбъекта/Ext/Template.xml",
            ),
            root.join(
                "InformationRegisters/ВерсииОбъектов/Templates/СтандартныйМакетПредставленияОбъекта/Ext/Template.xml",
            ),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "InformationRegisters/ВерсииОбъектов.xml",
            SourceKind::MetadataXml,
            Some("InformationRegisters/ВерсииОбъектов")
        )));
        assert!(files.contains(&(
            "InformationRegisters/ВерсииОбъектов/Ext/ManagerModule.bsl",
            SourceKind::Module,
            Some("InformationRegisters/ВерсииОбъектов")
        )));
        assert!(files.contains(&(
            "InformationRegisters/ВерсииОбъектов/Ext/RecordSetModule.bsl",
            SourceKind::Module,
            Some("InformationRegisters/ВерсииОбъектов")
        )));
        assert!(files.contains(&(
            "InformationRegisters/ВерсииОбъектов/Forms/ВыборРеквизитовОбъекта.xml",
            SourceKind::Form,
            Some("InformationRegisters/ВерсииОбъектов")
        )));
        assert!(files.contains(&(
            "InformationRegisters/ВерсииОбъектов/Forms/ВыборРеквизитовОбъекта/Ext/Form.xml",
            SourceKind::Form,
            Some("InformationRegisters/ВерсииОбъектов")
        )));
        assert!(files.contains(&(
            "InformationRegisters/ВерсииОбъектов/Forms/ВыборРеквизитовОбъекта/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("InformationRegisters/ВерсииОбъектов")
        )));
        assert!(files.contains(&(
            "InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи.xml",
            SourceKind::Form,
            Some("InformationRegisters/ВерсииОбъектов")
        )));
        assert!(files.contains(&(
            "InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Form.xml",
            SourceKind::Form,
            Some("InformationRegisters/ВерсииОбъектов")
        )));
        assert!(files.contains(&(
            "InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Help.xml",
            SourceKind::Form,
            Some("InformationRegisters/ВерсииОбъектов")
        )));
        assert!(files.contains(&(
            "InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("InformationRegisters/ВерсииОбъектов")
        )));
        assert!(files.contains(&(
            "InformationRegisters/ВерсииОбъектов/Forms/ФормаЗаписи/Ext/Help/ru.html",
            SourceKind::Form,
            Some("InformationRegisters/ВерсииОбъектов")
        )));
        assert!(files.contains(&(
            "InformationRegisters/ВерсииОбъектов/Templates/СтандартныйМакетПредставленияОбъекта.xml",
            SourceKind::Template,
            Some("InformationRegisters/ВерсииОбъектов")
        )));
        assert!(files.contains(&(
            "InformationRegisters/ВерсииОбъектов/Templates/СтандартныйМакетПредставленияОбъекта/Ext/Template.xml",
            SourceKind::Template,
            Some("InformationRegisters/ВерсииОбъектов")
        )));
    }

    #[test]
    fn scans_real_common_attribute_and_session_parameter_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-common-attribute-session-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("CommonAttributes")).unwrap();
        std::fs::create_dir_all(root.join("SessionParameters")).unwrap();

        std::fs::copy(
            lab_root.join("CommonAttributes/КомментарийЯзык1.xml"),
            root.join("CommonAttributes/КомментарийЯзык1.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("SessionParameters/АвторизованныйПользователь.xml"),
            root.join("SessionParameters/АвторизованныйПользователь.xml"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "CommonAttributes/КомментарийЯзык1.xml",
            SourceKind::MetadataXml,
            Some("CommonAttributes/КомментарийЯзык1")
        )));
        assert!(files.contains(&(
            "SessionParameters/АвторизованныйПользователь.xml",
            SourceKind::MetadataXml,
            Some("SessionParameters/АвторизованныйПользователь")
        )));
    }

    #[test]
    fn scans_real_common_command_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-common-command-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("CommonCommands/АвтономнаяРабота/Ext")).unwrap();
        std::fs::create_dir_all(root.join("CommonCommands/ЗагрузитьКурсыВалют/Ext")).unwrap();
        std::fs::create_dir_all(root.join("CommonCommands/АвтономнаяРабота/Ext/CommandInterface"))
            .unwrap();

        std::fs::copy(
            lab_root.join("CommonCommands/АвтономнаяРабота.xml"),
            root.join("CommonCommands/АвтономнаяРабота.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonCommands/АвтономнаяРабота/Ext/CommandModule.bsl"),
            root.join("CommonCommands/АвтономнаяРабота/Ext/CommandModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonCommands/ЗагрузитьКурсыВалют.xml"),
            root.join("CommonCommands/ЗагрузитьКурсыВалют.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonCommands/ЗагрузитьКурсыВалют/Ext/CommandModule.bsl"),
            root.join("CommonCommands/ЗагрузитьКурсыВалют/Ext/CommandModule.bsl"),
        )
        .unwrap();
        std::fs::write(
            root.join("CommonCommands/АвтономнаяРабота/Ext/CommandInterface.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20"/>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("CommonCommands/АвтономнаяРабота/Ext/CommandInterface/ru.html"),
            "<html>help</html>",
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "CommonCommands/АвтономнаяРабота.xml",
            SourceKind::MetadataXml,
            Some("CommonCommands/АвтономнаяРабота")
        )));
        assert!(files.contains(&(
            "CommonCommands/АвтономнаяРабота/Ext/CommandModule.bsl",
            SourceKind::Module,
            Some("CommonCommands/АвтономнаяРабота")
        )));
        assert!(files.contains(&(
            "CommonCommands/ЗагрузитьКурсыВалют.xml",
            SourceKind::MetadataXml,
            Some("CommonCommands/ЗагрузитьКурсыВалют")
        )));
        assert!(files.contains(&(
            "CommonCommands/ЗагрузитьКурсыВалют/Ext/CommandModule.bsl",
            SourceKind::Module,
            Some("CommonCommands/ЗагрузитьКурсыВалют")
        )));
        assert!(files.contains(&(
            "CommonCommands/АвтономнаяРабота/Ext/CommandInterface.xml",
            SourceKind::MetadataXml,
            Some("CommonCommands/АвтономнаяРабота")
        )));
        assert!(files.contains(&(
            "CommonCommands/АвтономнаяРабота/Ext/CommandInterface/ru.html",
            SourceKind::Other,
            Some("CommonCommands/АвтономнаяРабота")
        )));
    }

    #[test]
    fn scans_real_business_process_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-business-process-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("BusinessProcesses/Задание/Ext/Help")).unwrap();
        std::fs::create_dir_all(
            root.join("BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Form"),
        )
        .unwrap();
        std::fs::create_dir_all(
            root.join("BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Help"),
        )
        .unwrap();

        std::fs::copy(
            lab_root.join("BusinessProcesses/Задание.xml"),
            root.join("BusinessProcesses/Задание.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("BusinessProcesses/Задание/Ext/Flowchart.xml"),
            root.join("BusinessProcesses/Задание/Ext/Flowchart.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("BusinessProcesses/Задание/Ext/Help.xml"),
            root.join("BusinessProcesses/Задание/Ext/Help.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("BusinessProcesses/Задание/Ext/ManagerModule.bsl"),
            root.join("BusinessProcesses/Задание/Ext/ManagerModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("BusinessProcesses/Задание/Ext/ObjectModule.bsl"),
            root.join("BusinessProcesses/Задание/Ext/ObjectModule.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса.xml"),
            root.join("BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Form.xml"),
            root.join("BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Form.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Help.xml"),
            root.join("BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Help.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root
                .join("BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Form/Module.bsl"),
            root.join("BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Form/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Help/ru.html"),
            root.join("BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Help/ru.html"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "BusinessProcesses/Задание.xml",
            SourceKind::MetadataXml,
            Some("BusinessProcesses/Задание")
        )));
        assert!(files.contains(&(
            "BusinessProcesses/Задание/Ext/Flowchart.xml",
            SourceKind::MetadataXml,
            Some("BusinessProcesses/Задание")
        )));
        assert!(files.contains(&(
            "BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса.xml",
            SourceKind::Form,
            Some("BusinessProcesses/Задание")
        )));
        assert!(files.contains(&(
            "BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Form.xml",
            SourceKind::Form,
            Some("BusinessProcesses/Задание")
        )));
        assert!(files.contains(&(
            "BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Help.xml",
            SourceKind::Form,
            Some("BusinessProcesses/Задание")
        )));
        assert!(files.contains(&(
            "BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Form/Module.bsl",
            SourceKind::Module,
            Some("BusinessProcesses/Задание")
        )));
        assert!(files.contains(&(
            "BusinessProcesses/Задание/Forms/ФормаБизнесПроцесса/Ext/Help/ru.html",
            SourceKind::Form,
            Some("BusinessProcesses/Задание")
        )));
    }

    #[test]
    fn scans_real_xdto_package_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-xdto-package-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("XDTOPackages/Адрес/Ext")).unwrap();

        std::fs::copy(
            lab_root.join("XDTOPackages/Адрес.xml"),
            root.join("XDTOPackages/Адрес.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("XDTOPackages/Адрес/Ext/Package.bin"),
            root.join("XDTOPackages/Адрес/Ext/Package.bin"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "XDTOPackages/Адрес.xml",
            SourceKind::MetadataXml,
            Some("XDTOPackages/Адрес")
        )));
        assert!(files.contains(&(
            "XDTOPackages/Адрес/Ext/Package.bin",
            SourceKind::Binary,
            Some("XDTOPackages/Адрес")
        )));
    }

    #[test]
    fn scans_real_xdto_package_variant_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-xdto-variant-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("XDTOPackages/ApplicationExtensionsManagement_1_0_1_1/Ext"))
            .unwrap();

        std::fs::copy(
            lab_root.join("XDTOPackages/ApplicationExtensionsManagement_1_0_1_1.xml"),
            root.join("XDTOPackages/ApplicationExtensionsManagement_1_0_1_1.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("XDTOPackages/ApplicationExtensionsManagement_1_0_1_1/Ext/Package.bin"),
            root.join("XDTOPackages/ApplicationExtensionsManagement_1_0_1_1/Ext/Package.bin"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "XDTOPackages/ApplicationExtensionsManagement_1_0_1_1.xml",
            SourceKind::MetadataXml,
            Some("XDTOPackages/ApplicationExtensionsManagement_1_0_1_1")
        )));
        assert!(files.contains(&(
            "XDTOPackages/ApplicationExtensionsManagement_1_0_1_1/Ext/Package.bin",
            SourceKind::Binary,
            Some("XDTOPackages/ApplicationExtensionsManagement_1_0_1_1")
        )));
    }

    #[test]
    fn scans_real_role_and_scheduled_job_ext_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-role-job-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("Roles/АдминистраторСистемы/Ext")).unwrap();
        std::fs::create_dir_all(root.join("ScheduledJobs/ЗагрузкаКурсовВалют/Ext")).unwrap();

        std::fs::copy(
            lab_root.join("Roles/АдминистраторСистемы.xml"),
            root.join("Roles/АдминистраторСистемы.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Roles/АдминистраторСистемы/Ext/Rights.xml"),
            root.join("Roles/АдминистраторСистемы/Ext/Rights.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ScheduledJobs/ЗагрузкаКурсовВалют.xml"),
            root.join("ScheduledJobs/ЗагрузкаКурсовВалют.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("ScheduledJobs/ЗагрузкаКурсовВалют/Ext/Schedule.xml"),
            root.join("ScheduledJobs/ЗагрузкаКурсовВалют/Ext/Schedule.xml"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "Roles/АдминистраторСистемы.xml",
            SourceKind::MetadataXml,
            Some("Roles/АдминистраторСистемы")
        )));
        assert!(files.contains(&(
            "Roles/АдминистраторСистемы/Ext/Rights.xml",
            SourceKind::MetadataXml,
            Some("Roles/АдминистраторСистемы")
        )));
        assert!(files.contains(&(
            "ScheduledJobs/ЗагрузкаКурсовВалют.xml",
            SourceKind::MetadataXml,
            Some("ScheduledJobs/ЗагрузкаКурсовВалют")
        )));
        assert!(files.contains(&(
            "ScheduledJobs/ЗагрузкаКурсовВалют/Ext/Schedule.xml",
            SourceKind::MetadataXml,
            Some("ScheduledJobs/ЗагрузкаКурсовВалют")
        )));
    }

    #[test]
    fn scans_real_language_and_settings_storage_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-settings-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("CommonAttributes")).unwrap();
        std::fs::create_dir_all(root.join("Languages")).unwrap();
        std::fs::create_dir_all(root.join("SettingsStorages")).unwrap();
        std::fs::create_dir_all(root.join("StyleItems")).unwrap();

        std::fs::copy(
            lab_root.join("CommonAttributes/ОтредактированныеПредопределенныеРеквизиты.xml"),
            root.join("CommonAttributes/ОтредактированныеПредопределенныеРеквизиты.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("Languages/Русский.xml"),
            root.join("Languages/Русский.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("SettingsStorages/ХранилищеВариантовОтчетов.xml"),
            root.join("SettingsStorages/ХранилищеВариантовОтчетов.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("StyleItems/ВажнаяНадписьШрифт.xml"),
            root.join("StyleItems/ВажнаяНадписьШрифт.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("StyleItems/ЗавершенныйБизнесПроцесс.xml"),
            root.join("StyleItems/ЗавершенныйБизнесПроцесс.xml"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "CommonAttributes/ОтредактированныеПредопределенныеРеквизиты.xml",
            SourceKind::MetadataXml,
            Some("CommonAttributes/ОтредактированныеПредопределенныеРеквизиты")
        )));
        assert!(files.contains(&(
            "Languages/Русский.xml",
            SourceKind::MetadataXml,
            Some("Languages/Русский")
        )));
        assert!(files.contains(&(
            "SettingsStorages/ХранилищеВариантовОтчетов.xml",
            SourceKind::MetadataXml,
            Some("SettingsStorages/ХранилищеВариантовОтчетов")
        )));
        assert!(files.contains(&(
            "StyleItems/ВажнаяНадписьШрифт.xml",
            SourceKind::MetadataXml,
            Some("StyleItems/ВажнаяНадписьШрифт")
        )));
        assert!(files.contains(&(
            "StyleItems/ЗавершенныйБизнесПроцесс.xml",
            SourceKind::MetadataXml,
            Some("StyleItems/ЗавершенныйБизнесПроцесс")
        )));
    }

    #[test]
    fn scans_real_functional_options_parameters_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-functional-options-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("FunctionalOptionsParameters")).unwrap();

        std::fs::copy(
            lab_root.join("FunctionalOptionsParameters/ОбщиеНастройкиУзлов.xml"),
            root.join("FunctionalOptionsParameters/ОбщиеНастройкиУзлов.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("FunctionalOptionsParameters/ТипВерсионируемогоОбъекта.xml"),
            root.join("FunctionalOptionsParameters/ТипВерсионируемогоОбъекта.xml"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "FunctionalOptionsParameters/ОбщиеНастройкиУзлов.xml",
            SourceKind::MetadataXml,
            Some("FunctionalOptionsParameters/ОбщиеНастройкиУзлов")
        )));
        assert!(files.contains(&(
            "FunctionalOptionsParameters/ТипВерсионируемогоОбъекта.xml",
            SourceKind::MetadataXml,
            Some("FunctionalOptionsParameters/ТипВерсионируемогоОбъекта")
        )));
    }

    #[test]
    fn scans_real_service_and_subscription_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-services-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("EventSubscriptions")).unwrap();
        std::fs::create_dir_all(root.join("HTTPServices/exchange_dsl_1_0_0_1/Ext")).unwrap();
        std::fs::create_dir_all(root.join("WebServices/EnterpriseDataExchange_1_0_1_1/Ext"))
            .unwrap();
        std::fs::create_dir_all(root.join("WebServices/RemoteControl/Ext")).unwrap();
        std::fs::create_dir_all(root.join("WebServices/RemoteAdministrationOfExchange/Ext"))
            .unwrap();
        std::fs::create_dir_all(root.join("WebServices/InterfaceVersion/Ext")).unwrap();
        std::fs::create_dir_all(root.join("WebServices/Exchange/Ext")).unwrap();
        std::fs::create_dir_all(root.join("WebServices/EnterpriseDataUpload_1_0_1_1/Ext"))
            .unwrap();
        std::fs::create_dir_all(root.join("WebServices/Exchange_3_0_2_2/Ext")).unwrap();
        std::fs::create_dir_all(root.join("WebServices/Exchange_3_0_2_1/Ext")).unwrap();
        std::fs::create_dir_all(root.join("WebServices/RemoteAdministrationOfExchange_2_4_5_1/Ext"))
            .unwrap();
        std::fs::create_dir_all(root.join("WebServices/Exchange_2_0_1_6/Ext")).unwrap();
        std::fs::create_dir_all(root.join("WebServices/RemoteAdministrationOfExchange_2_1_6_1/Ext"))
            .unwrap();
        std::fs::create_dir_all(root.join("WebServices/Exchange_3_0_1_1/Ext")).unwrap();
        std::fs::create_dir_all(root.join("WebServices/RemoteAdministrationOfExchange_2_0_1_6/Ext"))
            .unwrap();
        std::fs::create_dir_all(root.join("WebServices/Сервис/Ext")).unwrap();

        std::fs::copy(
            lab_root.join("EventSubscriptions/ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных.xml"),
            root.join("EventSubscriptions/ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("HTTPServices/exchange_dsl_1_0_0_1.xml"),
            root.join("HTTPServices/exchange_dsl_1_0_0_1.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("HTTPServices/exchange_dsl_1_0_0_1/Ext/Module.bsl"),
            root.join("HTTPServices/exchange_dsl_1_0_0_1/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/EnterpriseDataExchange_1_0_1_1.xml"),
            root.join("WebServices/EnterpriseDataExchange_1_0_1_1.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/EnterpriseDataExchange_1_0_1_1/Ext/Module.bsl"),
            root.join("WebServices/EnterpriseDataExchange_1_0_1_1/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/RemoteControl.xml"),
            root.join("WebServices/RemoteControl.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/RemoteControl/Ext/Module.bsl"),
            root.join("WebServices/RemoteControl/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/RemoteAdministrationOfExchange.xml"),
            root.join("WebServices/RemoteAdministrationOfExchange.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/RemoteAdministrationOfExchange/Ext/Module.bsl"),
            root.join("WebServices/RemoteAdministrationOfExchange/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/InterfaceVersion.xml"),
            root.join("WebServices/InterfaceVersion.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/InterfaceVersion/Ext/Module.bsl"),
            root.join("WebServices/InterfaceVersion/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/Exchange.xml"),
            root.join("WebServices/Exchange.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/Exchange/Ext/Module.bsl"),
            root.join("WebServices/Exchange/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/EnterpriseDataUpload_1_0_1_1.xml"),
            root.join("WebServices/EnterpriseDataUpload_1_0_1_1.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/EnterpriseDataUpload_1_0_1_1/Ext/Module.bsl"),
            root.join("WebServices/EnterpriseDataUpload_1_0_1_1/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/Exchange_3_0_2_2.xml"),
            root.join("WebServices/Exchange_3_0_2_2.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/Exchange_3_0_2_2/Ext/Module.bsl"),
            root.join("WebServices/Exchange_3_0_2_2/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/Exchange_3_0_2_1.xml"),
            root.join("WebServices/Exchange_3_0_2_1.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/Exchange_3_0_2_1/Ext/Module.bsl"),
            root.join("WebServices/Exchange_3_0_2_1/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/RemoteAdministrationOfExchange_2_4_5_1.xml"),
            root.join("WebServices/RemoteAdministrationOfExchange_2_4_5_1.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/RemoteAdministrationOfExchange_2_4_5_1/Ext/Module.bsl"),
            root.join("WebServices/RemoteAdministrationOfExchange_2_4_5_1/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/Exchange_2_0_1_6.xml"),
            root.join("WebServices/Exchange_2_0_1_6.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/Exchange_2_0_1_6/Ext/Module.bsl"),
            root.join("WebServices/Exchange_2_0_1_6/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/RemoteAdministrationOfExchange_2_1_6_1.xml"),
            root.join("WebServices/RemoteAdministrationOfExchange_2_1_6_1.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/RemoteAdministrationOfExchange_2_1_6_1/Ext/Module.bsl"),
            root.join("WebServices/RemoteAdministrationOfExchange_2_1_6_1/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/Exchange_3_0_1_1.xml"),
            root.join("WebServices/Exchange_3_0_1_1.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/Exchange_3_0_1_1/Ext/Module.bsl"),
            root.join("WebServices/Exchange_3_0_1_1/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/RemoteAdministrationOfExchange_2_0_1_6.xml"),
            root.join("WebServices/RemoteAdministrationOfExchange_2_0_1_6.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("WebServices/RemoteAdministrationOfExchange_2_0_1_6/Ext/Module.bsl"),
            root.join("WebServices/RemoteAdministrationOfExchange_2_0_1_6/Ext/Module.bsl"),
        )
        .unwrap();
        std::fs::write(
            root.join("WebServices/Сервис.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <WebService uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <Properties>
      <Name>Сервис</Name>
      <Synonym/>
      <Comment/>
    </Properties>
  </WebService>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("WebServices/Сервис/Ext/Module.bsl"),
            "Procedure Stub()\r\nEndProcedure\r\n",
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "EventSubscriptions/ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных.xml",
            SourceKind::MetadataXml,
            Some("EventSubscriptions/ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных")
        )));
        assert!(files.contains(&(
            "HTTPServices/exchange_dsl_1_0_0_1.xml",
            SourceKind::MetadataXml,
            Some("HTTPServices/exchange_dsl_1_0_0_1")
        )));
        assert!(files.contains(&(
            "HTTPServices/exchange_dsl_1_0_0_1/Ext/Module.bsl",
            SourceKind::Module,
            Some("HTTPServices/exchange_dsl_1_0_0_1")
        )));
        assert!(files.contains(&(
            "WebServices/EnterpriseDataExchange_1_0_1_1.xml",
            SourceKind::MetadataXml,
            Some("WebServices/EnterpriseDataExchange_1_0_1_1")
        )));
        assert!(files.contains(&(
            "WebServices/EnterpriseDataExchange_1_0_1_1/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/EnterpriseDataExchange_1_0_1_1")
        )));
        assert!(files.contains(&(
            "WebServices/RemoteControl.xml",
            SourceKind::MetadataXml,
            Some("WebServices/RemoteControl")
        )));
        assert!(files.contains(&(
            "WebServices/RemoteControl/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/RemoteControl")
        )));
        assert!(files.contains(&(
            "WebServices/RemoteAdministrationOfExchange.xml",
            SourceKind::MetadataXml,
            Some("WebServices/RemoteAdministrationOfExchange")
        )));
        assert!(files.contains(&(
            "WebServices/RemoteAdministrationOfExchange/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/RemoteAdministrationOfExchange")
        )));
        assert!(files.contains(&(
            "WebServices/InterfaceVersion.xml",
            SourceKind::MetadataXml,
            Some("WebServices/InterfaceVersion")
        )));
        assert!(files.contains(&(
            "WebServices/InterfaceVersion/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/InterfaceVersion")
        )));
        assert!(files.contains(&(
            "WebServices/Exchange.xml",
            SourceKind::MetadataXml,
            Some("WebServices/Exchange")
        )));
        assert!(files.contains(&(
            "WebServices/Exchange/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/Exchange")
        )));
        assert!(files.contains(&(
            "WebServices/EnterpriseDataUpload_1_0_1_1.xml",
            SourceKind::MetadataXml,
            Some("WebServices/EnterpriseDataUpload_1_0_1_1")
        )));
        assert!(files.contains(&(
            "WebServices/EnterpriseDataUpload_1_0_1_1/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/EnterpriseDataUpload_1_0_1_1")
        )));
        assert!(files.contains(&(
            "WebServices/Exchange_3_0_2_2.xml",
            SourceKind::MetadataXml,
            Some("WebServices/Exchange_3_0_2_2")
        )));
        assert!(files.contains(&(
            "WebServices/Exchange_3_0_2_2/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/Exchange_3_0_2_2")
        )));
        assert!(files.contains(&(
            "WebServices/Exchange_3_0_2_1.xml",
            SourceKind::MetadataXml,
            Some("WebServices/Exchange_3_0_2_1")
        )));
        assert!(files.contains(&(
            "WebServices/Exchange_3_0_2_1/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/Exchange_3_0_2_1")
        )));
        assert!(files.contains(&(
            "WebServices/RemoteAdministrationOfExchange_2_4_5_1.xml",
            SourceKind::MetadataXml,
            Some("WebServices/RemoteAdministrationOfExchange_2_4_5_1")
        )));
        assert!(files.contains(&(
            "WebServices/RemoteAdministrationOfExchange_2_4_5_1/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/RemoteAdministrationOfExchange_2_4_5_1")
        )));
        assert!(files.contains(&(
            "WebServices/Exchange_2_0_1_6.xml",
            SourceKind::MetadataXml,
            Some("WebServices/Exchange_2_0_1_6")
        )));
        assert!(files.contains(&(
            "WebServices/Exchange_2_0_1_6/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/Exchange_2_0_1_6")
        )));
        assert!(files.contains(&(
            "WebServices/RemoteAdministrationOfExchange_2_1_6_1.xml",
            SourceKind::MetadataXml,
            Some("WebServices/RemoteAdministrationOfExchange_2_1_6_1")
        )));
        assert!(files.contains(&(
            "WebServices/RemoteAdministrationOfExchange_2_1_6_1/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/RemoteAdministrationOfExchange_2_1_6_1")
        )));
        assert!(files.contains(&(
            "WebServices/Exchange_3_0_1_1.xml",
            SourceKind::MetadataXml,
            Some("WebServices/Exchange_3_0_1_1")
        )));
        assert!(files.contains(&(
            "WebServices/Exchange_3_0_1_1/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/Exchange_3_0_1_1")
        )));
        assert!(files.contains(&(
            "WebServices/RemoteAdministrationOfExchange_2_0_1_6.xml",
            SourceKind::MetadataXml,
            Some("WebServices/RemoteAdministrationOfExchange_2_0_1_6")
        )));
        assert!(files.contains(&(
            "WebServices/RemoteAdministrationOfExchange_2_0_1_6/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/RemoteAdministrationOfExchange_2_0_1_6")
        )));
        assert!(files.contains(&(
            "WebServices/Сервис.xml",
            SourceKind::MetadataXml,
            Some("WebServices/Сервис")
        )));
        assert!(files.contains(&(
            "WebServices/Сервис/Ext/Module.bsl",
            SourceKind::Module,
            Some("WebServices/Сервис")
        )));
    }

    #[test]
    fn scans_real_event_subscription_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-event-subscriptions-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("EventSubscriptions")).unwrap();

        std::fs::copy(
            lab_root.join("EventSubscriptions/ЗаписатьВерсиюОбъекта.xml"),
            root.join("EventSubscriptions/ЗаписатьВерсиюОбъекта.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("EventSubscriptions/ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных.xml"),
            root.join("EventSubscriptions/ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных.xml"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "EventSubscriptions/ЗаписатьВерсиюОбъекта.xml",
            SourceKind::MetadataXml,
            Some("EventSubscriptions/ЗаписатьВерсиюОбъекта")
        )));
        assert!(files.contains(&(
            "EventSubscriptions/ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных.xml",
            SourceKind::MetadataXml,
            Some("EventSubscriptions/ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных")
        )));
    }

    #[test]
    fn scans_real_filter_criteria_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-filter-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("FilterCriteria")).unwrap();

        std::fs::copy(
            lab_root.join("FilterCriteria/СвязанныеДокументы.xml"),
            root.join("FilterCriteria/СвязанныеДокументы.xml"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "FilterCriteria/СвязанныеДокументы.xml",
            SourceKind::MetadataXml,
            Some("FilterCriteria/СвязанныеДокументы")
        )));
    }

    #[test]
    fn scans_real_common_picture_asset_layouts() {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-picture-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("CommonPictures/Адрес/Ext/Picture")).unwrap();
        std::fs::create_dir_all(root.join("CommonPictures/Взаимодействия/Ext/Picture")).unwrap();
        std::fs::create_dir_all(root.join("CommonPictures/ТранспортHTTP/Ext/Picture")).unwrap();
        std::fs::create_dir_all(root.join("CommonPictures/ФорматPDF/Ext/Picture")).unwrap();

        std::fs::copy(
            lab_root.join("CommonPictures/Адрес.xml"),
            root.join("CommonPictures/Адрес.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonPictures/Адрес/Ext/Picture.xml"),
            root.join("CommonPictures/Адрес/Ext/Picture.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonPictures/Адрес/Ext/Picture/Picture.zip"),
            root.join("CommonPictures/Адрес/Ext/Picture/Picture.zip"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonPictures/Взаимодействия.xml"),
            root.join("CommonPictures/Взаимодействия.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonPictures/Взаимодействия/Ext/Picture.xml"),
            root.join("CommonPictures/Взаимодействия/Ext/Picture.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonPictures/Взаимодействия/Ext/Picture/Picture.zip"),
            root.join("CommonPictures/Взаимодействия/Ext/Picture/Picture.zip"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonPictures/ТранспортHTTP.xml"),
            root.join("CommonPictures/ТранспортHTTP.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonPictures/ТранспортHTTP/Ext/Picture.xml"),
            root.join("CommonPictures/ТранспортHTTP/Ext/Picture.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonPictures/ТранспортHTTP/Ext/Picture/Picture.svg"),
            root.join("CommonPictures/ТранспортHTTP/Ext/Picture/Picture.svg"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonPictures/ФорматPDF.xml"),
            root.join("CommonPictures/ФорматPDF.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonPictures/ФорматPDF/Ext/Picture.xml"),
            root.join("CommonPictures/ФорматPDF/Ext/Picture.xml"),
        )
        .unwrap();
        std::fs::copy(
            lab_root.join("CommonPictures/ФорматPDF/Ext/Picture/Picture.zip"),
            root.join("CommonPictures/ФорматPDF/Ext/Picture/Picture.zip"),
        )
        .unwrap();

        let manifest = scan_sources(&root).unwrap();
        let _ = std::fs::remove_dir_all(&root);

        let files = manifest
            .files
            .iter()
            .map(|file| {
                (
                    file.path.as_str(),
                    file.kind.clone(),
                    file.object_hint.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        assert!(files.contains(&(
            "CommonPictures/Адрес.xml",
            SourceKind::MetadataXml,
            Some("CommonPictures/Адрес")
        )));
        assert!(files.contains(&(
            "CommonPictures/Адрес/Ext/Picture.xml",
            SourceKind::MetadataXml,
            Some("CommonPictures/Адрес")
        )));
        assert!(files.contains(&(
            "CommonPictures/Адрес/Ext/Picture/Picture.zip",
            SourceKind::Binary,
            Some("CommonPictures/Адрес")
        )));
        assert!(files.contains(&(
            "CommonPictures/Взаимодействия.xml",
            SourceKind::MetadataXml,
            Some("CommonPictures/Взаимодействия")
        )));
        assert!(files.contains(&(
            "CommonPictures/Взаимодействия/Ext/Picture.xml",
            SourceKind::MetadataXml,
            Some("CommonPictures/Взаимодействия")
        )));
        assert!(files.contains(&(
            "CommonPictures/Взаимодействия/Ext/Picture/Picture.zip",
            SourceKind::Binary,
            Some("CommonPictures/Взаимодействия")
        )));
        assert!(files.contains(&(
            "CommonPictures/ТранспортHTTP.xml",
            SourceKind::MetadataXml,
            Some("CommonPictures/ТранспортHTTP")
        )));
        assert!(files.contains(&(
            "CommonPictures/ТранспортHTTP/Ext/Picture.xml",
            SourceKind::MetadataXml,
            Some("CommonPictures/ТранспортHTTP")
        )));
        assert!(files.contains(&(
            "CommonPictures/ТранспортHTTP/Ext/Picture/Picture.svg",
            SourceKind::Binary,
            Some("CommonPictures/ТранспортHTTP")
        )));
        assert!(files.contains(&(
            "CommonPictures/ФорматPDF.xml",
            SourceKind::MetadataXml,
            Some("CommonPictures/ФорматPDF")
        )));
        assert!(files.contains(&(
            "CommonPictures/ФорматPDF/Ext/Picture.xml",
            SourceKind::MetadataXml,
            Some("CommonPictures/ФорматPDF")
        )));
        assert!(files.contains(&(
            "CommonPictures/ФорматPDF/Ext/Picture/Picture.zip",
            SourceKind::Binary,
            Some("CommonPictures/ФорматPDF")
        )));
    }
}
