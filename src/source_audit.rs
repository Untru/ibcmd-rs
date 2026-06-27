use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use rayon::prelude::*;
use serde::Serialize;
use walkdir::WalkDir;

use crate::module_blob::{
    MetadataSourceContext, pack_moxel_spreadsheet_blob_from_xml_with_source,
    parse_simple_metadata_xml_properties, parse_template_type_from_xml,
};
use crate::mssql_dump::extract_moxel_spreadsheet_xml;
use crate::parallel;

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

#[derive(Debug, Serialize)]
pub struct SpreadsheetTemplateRoundTripAuditReport {
    pub root: PathBuf,
    pub template_xml_files: usize,
    pub spreadsheet_templates: usize,
    pub packed: usize,
    pub extracted: usize,
    pub repacked: usize,
    pub matched: usize,
    pub different: usize,
    pub failed: usize,
    pub errors: Vec<SpreadsheetTemplateRoundTripAuditError>,
}

#[derive(Debug, Serialize)]
pub struct SpreadsheetTemplateRoundTripAuditError {
    pub metadata_xml: String,
    pub template_xml: String,
    pub phase: String,
    pub message: String,
}

#[derive(Debug, Default)]
struct SpreadsheetTemplateRoundTripItemAudit {
    packed: usize,
    extracted: usize,
    repacked: usize,
    matched: usize,
    different: usize,
    errors: Vec<SpreadsheetTemplateRoundTripAuditError>,
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

pub fn audit_spreadsheet_template_roundtrip(
    root: &Path,
) -> Result<SpreadsheetTemplateRoundTripAuditReport> {
    if !root.is_dir() {
        return Err(anyhow!(
            "source root is not a directory: {}",
            root.display()
        ));
    }

    let root = fs::canonicalize(root)
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    let source = MetadataSourceContext::new(root.clone());
    let object_refs = common_picture_object_refs(&root)?;
    let mut template_xml_files = 0usize;
    let mut templates = Vec::new();

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
        templates.push((metadata_xml, template_xml.to_path_buf()));
    }

    let item_reports = parallel::install(|| {
        templates
            .par_iter()
            .map(|(metadata_xml, template_xml)| {
                audit_one_spreadsheet_template_roundtrip(
                    &root,
                    &source,
                    &object_refs,
                    metadata_xml,
                    template_xml,
                )
            })
            .collect::<Vec<_>>()
    })?;
    let spreadsheet_templates = templates.len();
    let mut packed = 0usize;
    let mut extracted = 0usize;
    let mut repacked = 0usize;
    let mut matched = 0usize;
    let mut different = 0usize;
    let mut errors = Vec::new();
    for report in item_reports {
        packed += report.packed;
        extracted += report.extracted;
        repacked += report.repacked;
        matched += report.matched;
        different += report.different;
        errors.extend(report.errors);
    }

    errors.sort_by(|left, right| {
        left.template_xml
            .cmp(&right.template_xml)
            .then(left.phase.cmp(&right.phase))
    });
    Ok(SpreadsheetTemplateRoundTripAuditReport {
        root,
        template_xml_files,
        spreadsheet_templates,
        packed,
        extracted,
        repacked,
        matched,
        different,
        failed: errors.len(),
        errors,
    })
}

fn audit_one_spreadsheet_template_roundtrip(
    root: &Path,
    source: &MetadataSourceContext,
    object_refs: &BTreeMap<String, String>,
    metadata_xml: &Path,
    template_xml: &Path,
) -> SpreadsheetTemplateRoundTripItemAudit {
    let mut report = SpreadsheetTemplateRoundTripItemAudit::default();
    let xml = match fs::read(template_xml) {
        Ok(xml) => xml,
        Err(error) => {
            push_roundtrip_error(
                root,
                metadata_xml,
                template_xml,
                "read",
                error,
                &mut report.errors,
            );
            return report;
        }
    };
    let first = match pack_moxel_spreadsheet_blob_from_xml_with_source(&xml, Some(source)) {
        Ok(blob) => {
            report.packed += 1;
            blob
        }
        Err(error) => {
            push_roundtrip_error(
                root,
                metadata_xml,
                template_xml,
                "pack",
                error,
                &mut report.errors,
            );
            return report;
        }
    };
    let extracted_xml = match extract_moxel_spreadsheet_xml(&first.blob, object_refs) {
        Some(xml) => {
            report.extracted += 1;
            xml
        }
        None => {
            report.errors.push(SpreadsheetTemplateRoundTripAuditError {
                metadata_xml: relative_path_string(root, metadata_xml),
                template_xml: relative_path_string(root, template_xml),
                phase: "extract".to_string(),
                message: "failed to extract SpreadsheetDocument XML from packed MOXCEL blob"
                    .to_string(),
            });
            return report;
        }
    };
    let second = match pack_moxel_spreadsheet_blob_from_xml_with_source(
        extracted_xml.as_bytes(),
        Some(source),
    ) {
        Ok(blob) => {
            report.repacked += 1;
            blob
        }
        Err(error) => {
            push_roundtrip_error(
                root,
                metadata_xml,
                template_xml,
                "repack",
                error,
                &mut report.errors,
            );
            return report;
        }
    };
    let second_extracted_xml = match extract_moxel_spreadsheet_xml(&second.blob, object_refs) {
        Some(xml) => xml,
        None => {
            report.errors.push(SpreadsheetTemplateRoundTripAuditError {
                metadata_xml: relative_path_string(root, metadata_xml),
                template_xml: relative_path_string(root, template_xml),
                phase: "extract-repacked".to_string(),
                message: "failed to extract SpreadsheetDocument XML from repacked MOXCEL blob"
                    .to_string(),
            });
            return report;
        }
    };
    if extracted_xml == second_extracted_xml {
        report.matched += 1;
    } else {
        report.different += 1;
        report.errors.push(SpreadsheetTemplateRoundTripAuditError {
            metadata_xml: relative_path_string(root, metadata_xml),
            template_xml: relative_path_string(root, template_xml),
            phase: "compare".to_string(),
            message: roundtrip_difference_message(&extracted_xml, &second_extracted_xml),
        });
    }
    report
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

fn common_picture_object_refs(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut refs = BTreeMap::new();
    let common_pictures = root.join("CommonPictures");
    if !common_pictures.is_dir() {
        return Ok(refs);
    }
    for entry in fs::read_dir(&common_pictures)
        .with_context(|| format!("failed to read {}", common_pictures.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("xml") {
            continue;
        }
        let xml = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let properties = parse_simple_metadata_xml_properties(&xml)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        if properties.kind == "CommonPicture" {
            refs.insert(
                properties.uuid,
                format!("CommonPicture.{}", properties.name),
            );
        }
    }
    Ok(refs)
}

fn roundtrip_difference_message(first: &str, second: &str) -> String {
    let first_diff = first
        .bytes()
        .zip(second.bytes())
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| first.len().min(second.len()));
    format!(
        "round-trip SpreadsheetDocument XML differs: first_bytes={}, second_bytes={}, first_diff_offset={first_diff}",
        first.len(),
        second.len()
    )
}

fn push_roundtrip_error(
    root: &Path,
    metadata_xml: &Path,
    template_xml: &Path,
    phase: &str,
    error: impl std::fmt::Display,
    errors: &mut Vec<SpreadsheetTemplateRoundTripAuditError>,
) {
    errors.push(SpreadsheetTemplateRoundTripAuditError {
        metadata_xml: relative_path_string(root, metadata_xml),
        template_xml: relative_path_string(root, template_xml),
        phase: phase.to_string(),
        message: error.to_string(),
    });
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
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core">
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

    #[test]
    fn audits_spreadsheet_template_roundtrip_matches_stable_moxel() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-audit-spreadsheet-roundtrip-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(root.join("CommonTemplates/Good/Ext"))?;
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
		<row>
			<c>
				<c>
					<f>0</f>
					<tl>
						<v8:item>
							<v8:lang>ru</v8:lang>
							<v8:content>Hello</v8:content>
						</v8:item>
					</tl>
				</c>
			</c>
		</row>
	</rowsItem>
</document>
"#,
        )?;

        let report = audit_spreadsheet_template_roundtrip(&root)?;

        assert_eq!(report.template_xml_files, 1);
        assert_eq!(report.spreadsheet_templates, 1);
        assert_eq!(report.packed, 1);
        assert_eq!(report.extracted, 1);
        assert_eq!(report.repacked, 1);
        assert_eq!(report.matched, 1);
        assert_eq!(report.different, 0);
        assert_eq!(report.failed, 0);
        assert!(report.errors.is_empty());

        Ok(())
    }
}
