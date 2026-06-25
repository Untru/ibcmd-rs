use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use quick_xml::Reader;
use quick_xml::events::Event;
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
    let mut files = Vec::new();

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
        files.push(scan_file(path, relative)?);
    }

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
    matches!(
        value,
        "Catalogs"
            | "Documents"
            | "InformationRegisters"
            | "AccumulationRegisters"
            | "AccountingRegisters"
            | "CalculationRegisters"
            | "ChartsOfCharacteristicTypes"
            | "ChartsOfAccounts"
            | "ChartsOfCalculationTypes"
            | "ChartsOfCalculationRegisters"
            | "CommonModules"
            | "CommonForms"
            | "CommonPictures"
            | "CommonTemplates"
            | "CommonAttributes"
            | "DocumentJournals"
            | "Reports"
            | "DataProcessors"
            | "Enums"
            | "ExchangePlans"
            | "EventSubscriptions"
            | "FilterCriteria"
            | "FunctionalOptions"
            | "FunctionalOptionsParameters"
            | "HTTPServices"
            | "Languages"
            | "ScheduledJobs"
            | "SessionParameters"
            | "SettingsStorages"
            | "StyleItems"
            | "Subsystems"
            | "Roles"
            | "CommonCommands"
            | "Constants"
            | "WebServices"
            | "XDTOPackages"
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
    relative_lower.contains("/ext/")
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
    use super::{SourceKind, classify, infer_object_hint};

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
                "SessionParameters/АвторизованныйПользователь.xml",
                &SourceKind::MetadataXml,
                Some("SessionParameter")
            ),
            Some("SessionParameters/АвторизованныйПользователь".to_string())
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
    }
}
