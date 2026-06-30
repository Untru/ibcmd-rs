use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::Serialize;

use crate::cli::FormDiffCandidatesArgs;
use crate::module_blob::parse_form_body_blob;

#[derive(Debug, Serialize)]
pub struct FormDiffCandidateReport {
    pub base_xml: String,
    pub variant_xml: String,
    pub base_blob: String,
    pub variant_blob: String,
    pub xml_differences: Vec<FormDiffValue>,
    pub layout_differences: Vec<FormDiffValue>,
    pub candidate_mappings: Vec<FormDiffCandidateMapping>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FormDiffValue {
    pub path: String,
    pub base: Option<String>,
    pub variant: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FormDiffCandidateMapping {
    pub xml_path: String,
    pub layout_path: String,
    pub confidence: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BraceNode {
    List(Vec<BraceNode>),
    Atom(String),
}

pub fn analyze_form_diff_candidates(
    args: &FormDiffCandidatesArgs,
) -> Result<FormDiffCandidateReport> {
    let base_xml_bytes = fs::read(&args.base_xml)
        .with_context(|| format!("failed to read base XML {}", args.base_xml.display()))?;
    let variant_xml_bytes = fs::read(&args.variant_xml)
        .with_context(|| format!("failed to read variant XML {}", args.variant_xml.display()))?;
    let base_blob_bytes = fs::read(&args.base_blob)
        .with_context(|| format!("failed to read base blob {}", args.base_blob.display()))?;
    let variant_blob_bytes = fs::read(&args.variant_blob).with_context(|| {
        format!(
            "failed to read variant blob {}",
            args.variant_blob.display()
        )
    })?;

    let base_xml = xml_leaf_values(&base_xml_bytes).context("failed to parse base XML")?;
    let variant_xml = xml_leaf_values(&variant_xml_bytes).context("failed to parse variant XML")?;
    let xml_differences = diff_values(&base_xml, &variant_xml);

    let base_form =
        parse_form_body_blob(&base_blob_bytes).context("failed to parse base Form body blob")?;
    let variant_form = parse_form_body_blob(&variant_blob_bytes)
        .context("failed to parse variant Form body blob")?;
    let base_layout = brace_leaf_values("layout", &base_form.layout)
        .context("failed to parse base Form layout tree")?;
    let variant_layout = brace_leaf_values("layout", &variant_form.layout)
        .context("failed to parse variant Form layout tree")?;
    let layout_differences = diff_values(&base_layout, &variant_layout);
    let candidate_mappings = candidate_mappings(&xml_differences, &layout_differences);

    Ok(FormDiffCandidateReport {
        base_xml: path_text(&args.base_xml),
        variant_xml: path_text(&args.variant_xml),
        base_blob: path_text(&args.base_blob),
        variant_blob: path_text(&args.variant_blob),
        xml_differences,
        layout_differences,
        candidate_mappings,
    })
}

fn path_text(path: &Path) -> String {
    path.display().to_string()
}

pub fn write_form_diff_candidate_report(
    report: &FormDiffCandidateReport,
    output: &Path,
) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    fs::write(output, json).with_context(|| {
        format!(
            "failed to write Form diff candidate report {}",
            output.display()
        )
    })
}

fn candidate_mappings(
    xml_differences: &[FormDiffValue],
    layout_differences: &[FormDiffValue],
) -> Vec<FormDiffCandidateMapping> {
    if xml_differences.is_empty() || layout_differences.is_empty() {
        return Vec::new();
    }
    if xml_differences.len() == 1 && layout_differences.len() == 1 {
        return vec![FormDiffCandidateMapping {
            xml_path: xml_differences[0].path.clone(),
            layout_path: layout_differences[0].path.clone(),
            confidence: "high".to_string(),
            reason: "single XML leaf changed and single layout atom changed".to_string(),
        }];
    }
    let mut mappings = Vec::new();
    for xml in xml_differences {
        for layout in layout_differences {
            if changed_to_same_value(xml, layout) {
                mappings.push(FormDiffCandidateMapping {
                    xml_path: xml.path.clone(),
                    layout_path: layout.path.clone(),
                    confidence: "medium".to_string(),
                    reason: "variant values match after trimming quotes".to_string(),
                });
            }
        }
    }
    mappings
}

fn changed_to_same_value(left: &FormDiffValue, right: &FormDiffValue) -> bool {
    match (&left.variant, &right.variant) {
        (Some(left), Some(right)) => normalize_value(left) == normalize_value(right),
        _ => false,
    }
}

fn normalize_value(value: &str) -> String {
    value.trim().trim_matches('"').to_string()
}

fn diff_values(
    base: &BTreeMap<String, String>,
    variant: &BTreeMap<String, String>,
) -> Vec<FormDiffValue> {
    let mut paths = base
        .keys()
        .chain(variant.keys())
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter_map(|path| {
            let base_value = base.get(&path);
            let variant_value = variant.get(&path);
            if base_value == variant_value {
                None
            } else {
                Some(FormDiffValue {
                    path,
                    base: base_value.cloned(),
                    variant: variant_value.cloned(),
                })
            }
        })
        .collect()
}

fn xml_leaf_values(xml: &[u8]) -> Result<BTreeMap<String, String>> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut text_by_path = BTreeMap::<String, String>::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = String::from_utf8_lossy(event.local_name().as_ref()).to_string();
                path.push(local);
                let current = path.join("/");
                for attribute in event.attributes() {
                    let attribute = attribute.context("failed to read XML attribute")?;
                    let name =
                        String::from_utf8_lossy(attribute.key.local_name().as_ref()).to_string();
                    let value = attribute
                        .decode_and_unescape_value(reader.decoder())
                        .context("failed to decode XML attribute")?
                        .into_owned();
                    text_by_path.insert(format!("{current}/@{name}"), value);
                }
            }
            Ok(Event::Empty(event)) => {
                let local = String::from_utf8_lossy(event.local_name().as_ref()).to_string();
                path.push(local);
                let current = path.join("/");
                let mut has_attribute = false;
                for attribute in event.attributes() {
                    let attribute = attribute.context("failed to read XML attribute")?;
                    let name =
                        String::from_utf8_lossy(attribute.key.local_name().as_ref()).to_string();
                    let value = attribute
                        .decode_and_unescape_value(reader.decoder())
                        .context("failed to decode XML attribute")?
                        .into_owned();
                    text_by_path.insert(format!("{current}/@{name}"), value);
                    has_attribute = true;
                }
                if !has_attribute {
                    text_by_path.insert(current, String::new());
                }
                path.pop();
            }
            Ok(Event::Text(event)) => {
                let text = event
                    .xml_content()
                    .context("failed to decode XML text")?
                    .trim()
                    .to_string();
                if !text.is_empty() {
                    text_by_path.insert(path.join("/"), text);
                }
            }
            Ok(Event::End(_)) => {
                path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(anyhow!("failed to parse XML: {error}")),
        }
        buffer.clear();
    }

    Ok(text_by_path)
}

fn brace_leaf_values(root: &str, text: &str) -> Result<BTreeMap<String, String>> {
    let mut parser = BraceParser::new(text);
    let node = parser.parse_node().context("failed to parse brace tree")?;
    parser.skip_ws();
    if !parser.is_eof() {
        bail!("unexpected trailing text at byte {}", parser.index);
    }
    let mut values = BTreeMap::new();
    collect_brace_leaf_values(root.to_string(), &node, &mut values);
    Ok(values)
}

fn collect_brace_leaf_values(
    path: String,
    node: &BraceNode,
    values: &mut BTreeMap<String, String>,
) {
    match node {
        BraceNode::Atom(value) => {
            values.insert(path, value.clone());
        }
        BraceNode::List(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_brace_leaf_values(format!("{path}/{index}"), item, values);
            }
        }
    }
}

struct BraceParser<'a> {
    text: &'a str,
    index: usize,
}

impl<'a> BraceParser<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, index: 0 }
    }

    fn parse_node(&mut self) -> Result<BraceNode> {
        self.skip_ws();
        if self.peek_char() == Some('{') {
            self.parse_list()
        } else {
            self.parse_atom()
        }
    }

    fn parse_list(&mut self) -> Result<BraceNode> {
        self.expect_char('{')?;
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            match self.peek_char() {
                Some('}') => {
                    self.index += 1;
                    break;
                }
                Some(_) => {
                    items.push(self.parse_node()?);
                    self.skip_ws();
                    match self.peek_char() {
                        Some(',') => {
                            self.index += 1;
                        }
                        Some('}') => {}
                        Some(value) => {
                            bail!("expected ',' or '}}' at byte {}, got {value}", self.index)
                        }
                        None => bail!("unterminated list"),
                    }
                }
                None => bail!("unterminated list"),
            }
        }
        Ok(BraceNode::List(items))
    }

    fn parse_atom(&mut self) -> Result<BraceNode> {
        self.skip_ws();
        if self.peek_char() == Some('"') {
            return self.parse_quoted_atom();
        }
        let start = self.index;
        while let Some(ch) = self.peek_char() {
            if ch == ',' || ch == '}' {
                break;
            }
            self.index += ch.len_utf8();
        }
        if start == self.index {
            bail!("expected atom at byte {}", self.index);
        }
        Ok(BraceNode::Atom(
            self.text[start..self.index].trim().to_string(),
        ))
    }

    fn parse_quoted_atom(&mut self) -> Result<BraceNode> {
        let start = self.index;
        self.index += 1;
        while let Some(ch) = self.peek_char() {
            self.index += ch.len_utf8();
            if ch == '"' {
                if self.peek_char() == Some('"') {
                    self.index += 1;
                    continue;
                }
                return Ok(BraceNode::Atom(self.text[start..self.index].to_string()));
            }
        }
        bail!("unterminated quoted atom at byte {start}")
    }

    fn expect_char(&mut self, expected: char) -> Result<()> {
        match self.peek_char() {
            Some(ch) if ch == expected => {
                self.index += ch.len_utf8();
                Ok(())
            }
            Some(ch) => bail!("expected {expected} at byte {}, got {ch}", self.index),
            None => bail!("expected {expected} at eof"),
        }
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.index += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.text[self.index..].chars().next()
    }

    fn is_eof(&self) -> bool {
        self.index >= self.text.len()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::DeflateEncoder;

    use super::*;

    #[test]
    fn maps_single_xml_change_to_single_layout_atom_change() -> anyhow::Result<()> {
        let root = temp_root("ibcmd-rs-form-diff-candidates");
        fs::create_dir_all(&root)?;
        let base_xml = root.join("base.xml");
        let variant_xml = root.join("variant.xml");
        let base_blob = root.join("base.bin");
        let variant_blob = root.join("variant.bin");

        fs::write(
            &base_xml,
            br#"<Form><ChildItems><InputField name="Name"><ReadOnly>false</ReadOnly></InputField></ChildItems></Form>"#,
        )?;
        fs::write(
            &variant_xml,
            br#"<Form><ChildItems><InputField name="Name"><ReadOnly>true</ReadOnly></InputField></ChildItems></Form>"#,
        )?;
        fs::write(&base_blob, deflate(br#"{4,{59,{7,0}},"",{0}}"#)?)?;
        fs::write(&variant_blob, deflate(br#"{4,{59,{7,1}},"",{0}}"#)?)?;

        let report = analyze_form_diff_candidates(&FormDiffCandidatesArgs {
            base_xml,
            variant_xml,
            base_blob,
            variant_blob,
            output: None,
        })?;

        assert_eq!(report.xml_differences.len(), 1);
        assert_eq!(
            report.xml_differences[0].path,
            "Form/ChildItems/InputField/ReadOnly"
        );
        assert_eq!(report.layout_differences.len(), 1);
        assert_eq!(report.layout_differences[0].path, "layout/1/1");
        assert_eq!(report.candidate_mappings.len(), 1);
        assert_eq!(report.candidate_mappings[0].confidence, "high");

        Ok(())
    }

    #[test]
    fn parses_nested_brace_tree_with_quoted_commas() -> anyhow::Result<()> {
        let values = brace_leaf_values("layout", r#"{59,{"Name, with comma",1},{0}}"#)?;

        assert_eq!(values.get("layout/0").map(String::as_str), Some("59"));
        assert_eq!(
            values.get("layout/1/0").map(String::as_str),
            Some(r#""Name, with comma""#)
        );
        assert_eq!(values.get("layout/1/1").map(String::as_str), Some("1"));

        Ok(())
    }

    fn deflate(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes)?;
        Ok(encoder.finish()?)
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        let mut root = std::env::temp_dir();
        root.push(format!(
            "{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        root
    }
}
