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
    let mut entries = Vec::new();

    for entry in WalkDir::new(&canonical_root)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry.path()))
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let relative = path
            .strip_prefix(&canonical_root)
            .with_context(|| format!("failed to make relative path for {}", path.display()))?;
        entries.push((path.to_path_buf(), relative.to_path_buf()));
    }

    let mut files = entries
        .into_par_iter()
        .map(|(path, relative)| scan_file(&path, &relative))
        .collect::<Result<Vec<_>>>()?;
    files.sort_by(|left, right| left.path.cmp(&right.path));

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
        first_xml_element(path).unwrap_or(None)
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
        std::fs::create_dir_all(root.join("FunctionalOptions")).unwrap();
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
            "FunctionalOptions/ВыполнятьЗамерыПроизводительности.xml",
            SourceKind::MetadataXml,
            Some("FunctionalOptions/ВыполнятьЗамерыПроизводительности")
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
            lab_root.join("CommonTemplates/ВидыДокументовУдостоверяющихЛичность.xml"),
            root.join("CommonTemplates/ВидыДокументовУдостоверяющихЛичность.xml"),
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
            "CommonTemplates/ВидыДокументовУдостоверяющихЛичность.xml",
            SourceKind::MetadataXml,
            Some("CommonTemplates/ВидыДокументовУдостоверяющихЛичность")
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
        std::fs::create_dir_all(root.join("Languages")).unwrap();
        std::fs::create_dir_all(root.join("SettingsStorages")).unwrap();
        std::fs::create_dir_all(root.join("StyleItems")).unwrap();

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
