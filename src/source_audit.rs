use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use walkdir::WalkDir;

use crate::module_blob::{
    MetadataSourceContext, pack_moxel_spreadsheet_blob_from_xml_with_source,
    parse_template_type_from_xml,
};

#[derive(Debug, Serialize)]
pub struct SpreadsheetTemplateAuditReport {
    pub root: PathBuf,
    pub template_xml_files: usize,
    pub spreadsheet_templates: usize,
    pub packed: usize,
    pub failed: usize,
    pub errors: Vec<SpreadsheetTemplateAuditError>,
}

#[derive(Debug, Serialize)]
pub struct SpreadsheetTemplateAuditError {
    pub metadata_xml: String,
    pub template_xml: String,
    pub message: String,
}

pub fn audit_spreadsheet_templates(root: &Path) -> Result<SpreadsheetTemplateAuditReport> {
    if !root.is_dir() {
        return Err(anyhow!(
            "source root is not a directory: {}",
            root.display()
        ));
    }

    let root = fs::canonicalize(root)
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    let source = MetadataSourceContext::new(root.clone());
    let mut template_xml_files = 0usize;
    let mut spreadsheet_templates = 0usize;
    let mut packed = 0usize;
    let mut errors = Vec::new();

    for entry in WalkDir::new(&root)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry.path()))
    {
        let entry = entry?;
        if !entry.file_type().is_file() || entry.file_name() != "Template.xml" {
            continue;
        }
        let template_xml = entry.path();
        let Some(metadata_xml) = template_metadata_xml_path(template_xml) else {
            continue;
        };
        template_xml_files += 1;
        let metadata = fs::read(&metadata_xml)
            .with_context(|| format!("failed to read {}", metadata_xml.display()))?;
        if parse_template_type_from_xml(&metadata)?.as_deref() != Some("SpreadsheetDocument") {
            continue;
        }
        spreadsheet_templates += 1;

        let xml = fs::read(template_xml)
            .with_context(|| format!("failed to read {}", template_xml.display()))?;
        match pack_moxel_spreadsheet_blob_from_xml_with_source(&xml, Some(&source)) {
            Ok(_) => packed += 1,
            Err(error) => errors.push(SpreadsheetTemplateAuditError {
                metadata_xml: relative_path_string(&root, &metadata_xml),
                template_xml: relative_path_string(&root, template_xml),
                message: error.to_string(),
            }),
        }
    }

    errors.sort_by(|left, right| left.template_xml.cmp(&right.template_xml));
    Ok(SpreadsheetTemplateAuditReport {
        root,
        template_xml_files,
        spreadsheet_templates,
        packed,
        failed: errors.len(),
        errors,
    })
}

fn template_metadata_xml_path(template_xml: &Path) -> Option<PathBuf> {
    if template_xml.file_name()? != "Template.xml" {
        return None;
    }
    let ext_dir = template_xml.parent()?;
    if ext_dir.file_name()? != "Ext" {
        return None;
    }
    let template_dir = ext_dir.parent()?;
    Some(template_dir.with_extension("xml"))
}

fn relative_path_string(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_ignored(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == ".git" || name == ".vscode" || name == "target")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audits_spreadsheet_templates_and_reports_errors() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-audit-spreadsheet-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(root.join("CommonTemplates/Good/Ext"))?;
        fs::create_dir_all(root.join("CommonTemplates/Broken/Ext"))?;
        fs::write(
            root.join("CommonTemplates/Good.xml"),
            br#"
<MetaDataObject>
  <CommonTemplate uuid="aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa">
    <Properties>
      <Name>Good</Name>
      <Synonym/>
      <Comment/>
      <TemplateType>SpreadsheetDocument</TemplateType>
    </Properties>
  </CommonTemplate>
</MetaDataObject>
"#,
        )?;
        fs::write(
            root.join("CommonTemplates/Good/Ext/Template.xml"),
            br#"
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet">
  <columns><size>1</size></columns>
  <rowsItem>
    <index>0</index>
    <row><empty>true</empty></row>
  </rowsItem>
</document>
"#,
        )?;
        fs::write(
            root.join("CommonTemplates/Broken.xml"),
            br#"
<MetaDataObject>
  <CommonTemplate uuid="bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb">
    <Properties>
      <Name>Broken</Name>
      <Synonym/>
      <Comment/>
      <TemplateType>SpreadsheetDocument</TemplateType>
    </Properties>
  </CommonTemplate>
</MetaDataObject>
"#,
        )?;
        fs::write(
            root.join("CommonTemplates/Broken/Ext/Template.xml"),
            br#"<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet"/>"#,
        )?;

        let report = audit_spreadsheet_templates(&root)?;

        assert_eq!(report.template_xml_files, 2);
        assert_eq!(report.spreadsheet_templates, 2);
        assert_eq!(report.packed, 1);
        assert_eq!(report.failed, 1);
        assert_eq!(
            report.errors[0].template_xml,
            "CommonTemplates/Broken/Ext/Template.xml"
        );
        assert!(
            report.errors[0]
                .message
                .contains("SpreadsheetDocument XML has no rowsItem entries")
        );

        Ok(())
    }
}
