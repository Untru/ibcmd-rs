//! Strict embedded text-cell settings. Other embedded cohorts retain their
//! existing writer; unclassified cell or formatting content is refused.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, anyhow, bail, ensure};
use ibcmd_1c_text::format_1c_string;
use ibcmd_form_schema::embedded_spreadsheet as stored;
use ibcmd_xml::{AttributeKind, XmlElement, XmlNode, XmlReader};

const SHEET_NS: &str = "http://v8.1c.ru/8.2/data/spreadsheet";
const CORE_NS: &str = "http://v8.1c.ru/8.1/data/core";
const FORM_NS: &str = "http://v8.1c.ru/8.3/xcf/logform";
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

type Namespaces = BTreeMap<String, String>;

fn namespaces(element: &XmlElement, inherited: &Namespaces) -> Namespaces {
    let mut scope = inherited.clone();
    for attribute in element.attributes() {
        if let AttributeKind::Namespace(prefix) = attribute.kind() {
            scope.insert(
                prefix.clone().unwrap_or_default(),
                attribute.value().to_string(),
            );
        }
    }
    scope
}

fn namespace<'a>(element: &XmlElement, scope: &'a Namespaces) -> Option<&'a str> {
    scope
        .get(element.name().prefix().unwrap_or_default())
        .map(String::as_str)
}

fn elements(element: &XmlElement) -> Result<Vec<&XmlElement>> {
    element
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(value) => Some(Ok(value)),
            XmlNode::Text(text) if text.value().trim().is_empty() => None,
            _ => Some(Err(anyhow!(
                "embedded spreadsheet contains non-element content"
            ))),
        })
        .collect()
}

fn selection_elements(element: &XmlElement) -> impl Iterator<Item = &XmlElement> {
    element.children().iter().filter_map(|node| match node {
        XmlNode::Element(element) => Some(element),
        _ => None,
    })
}

fn attribute<'a>(element: &'a XmlElement, name: &str) -> Option<&'a str> {
    element
        .attributes()
        .iter()
        .find_map(|attribute| match attribute.kind() {
            AttributeKind::Ordinary(key) if key.raw() == name => Some(attribute.value()),
            _ => None,
        })
}

struct Node {
    name: String,
    attributes: BTreeMap<String, String>,
    children: Vec<Node>,
    text: String,
}

impl Node {
    fn read(element: &XmlElement, inherited: &Namespaces) -> Result<Self> {
        Self::read_bounded(element, inherited, 0)
    }

    fn read_bounded(element: &XmlElement, inherited: &Namespaces, depth: usize) -> Result<Self> {
        ensure!(
            depth < 64,
            "embedded spreadsheet nesting exceeds codec limit"
        );
        let scope = namespaces(element, inherited);
        let name = match namespace(element, &scope) {
            Some(SHEET_NS) => element.name().local().to_string(),
            Some(CORE_NS) => format!("v8:{}", element.name().local()),
            _ => bail!("embedded spreadsheet has an unknown element namespace"),
        };
        let mut node = Self {
            name,
            attributes: BTreeMap::new(),
            children: Vec::new(),
            text: String::new(),
        };
        for attribute in element.attributes() {
            if let AttributeKind::Ordinary(key) = attribute.kind() {
                ensure!(
                    key.prefix().is_none(),
                    "embedded spreadsheet has an unsupported qualified attribute"
                );
                node.attributes
                    .insert(key.raw().to_string(), attribute.value().to_string());
            }
        }
        for child in element.children() {
            match child {
                XmlNode::Element(child) => {
                    node.children
                        .push(Self::read_bounded(child, &scope, depth + 1)?)
                }
                XmlNode::Text(text) => node.text.push_str(text.value()),
                _ => bail!("embedded spreadsheet contains unsupported XML content"),
            }
        }
        Ok(node)
    }

    fn only(&self, allowed: &[&str]) -> Result<()> {
        ensure!(
            self.attributes.is_empty() && self.text.trim().is_empty(),
            "<{}> has unexpected attributes or text",
            self.name
        );
        ensure!(
            self.children
                .iter()
                .all(|child| allowed.contains(&child.name.as_str())),
            "<{}> has unsupported children",
            self.name
        );
        Ok(())
    }

    fn one(&self, name: &str) -> Result<Option<&Self>> {
        let mut values = self.children.iter().filter(|child| child.name == name);
        let first = values.next();
        ensure!(values.next().is_none(), "<{}> repeats <{name}>", self.name);
        Ok(first)
    }

    fn value(&self) -> Result<&str> {
        ensure!(
            self.attributes.is_empty() && self.children.is_empty(),
            "<{}> is not a scalar",
            self.name
        );
        Ok(&self.text)
    }

    fn number(&self, name: &str) -> Result<u32> {
        self.one(name)?
            .ok_or_else(|| anyhow!("<{}> has no <{name}>", self.name))?
            .value()?
            .parse()
            .map_err(Into::into)
    }
}

pub(super) fn with_text_cells(form: &str, attribute_name: &str) -> Result<Option<String>> {
    let document = XmlReader::from_slice(form.as_bytes())?;
    let root = document.root();
    let scope = namespaces(root, &Namespaces::new());
    let mut selected = None;
    let mut matched_attribute = false;
    for attributes in
        selection_elements(root).filter(|node| node.name().local() == stored::ATTRIBUTES)
    {
        let scope = namespaces(attributes, &scope);
        ensure!(
            namespace(attributes, &scope) == Some(FORM_NS),
            "unknown form attribute namespace"
        );
        for attribute_node in selection_elements(attributes).filter(|node| {
            node.name().local() == stored::ATTRIBUTE
                && attribute(node, "name") == Some(attribute_name)
        }) {
            ensure!(!matched_attribute, "duplicate spreadsheet attribute name");
            matched_attribute = true;
            let scope = namespaces(attribute_node, &scope);
            ensure!(
                namespace(attribute_node, &scope) == Some(FORM_NS),
                "unknown form attribute namespace"
            );
            let mut settings_seen = false;
            for settings in selection_elements(attribute_node)
                .filter(|node| node.name().local() == stored::SETTINGS)
            {
                ensure!(!settings_seen, "spreadsheet attribute repeats Settings");
                settings_seen = true;
                let scope = namespaces(settings, &scope);
                fn has_cells(element: &XmlElement, inherited: &Namespaces) -> Result<bool> {
                    let mut pending = vec![(element, inherited.clone(), 0)];
                    while let Some((element, inherited, depth)) = pending.pop() {
                        ensure!(
                            depth < 64,
                            "embedded spreadsheet nesting exceeds codec limit"
                        );
                        let scope = namespaces(element, &inherited);
                        if element.name().local() == stored::CELL
                            && namespace(element, &scope) == Some(SHEET_NS)
                        {
                            return Ok(true);
                        }
                        for child in selection_elements(element) {
                            pending.push((child, scope.clone(), depth + 1));
                        }
                    }
                    Ok(false)
                }
                if !has_cells(settings, &scope)? {
                    continue;
                }
                let valid_type = settings.attributes().iter().any(|a| match a.kind() {
                    AttributeKind::Ordinary(key)
                        if key.local() == stored::TYPE
                            && key
                                .prefix()
                                .and_then(|prefix| scope.get(prefix))
                                .map(String::as_str)
                                == Some(XSI_NS) =>
                    {
                        a.value().split_once(':').is_some_and(|(prefix, name)| {
                            name == stored::DOCUMENT_TYPE
                                && scope.get(prefix).map(String::as_str) == Some(SHEET_NS)
                        })
                    }
                    _ => false,
                });
                let ordinary_attributes = settings
                    .attributes()
                    .iter()
                    .filter(|a| matches!(a.kind(), AttributeKind::Ordinary(_)))
                    .count();
                ensure!(
                    ordinary_attributes == 1,
                    "embedded Settings has unknown attributes"
                );
                ensure!(
                    valid_type && namespace(settings, &scope) == Some(FORM_NS),
                    "unknown embedded spreadsheet Settings type"
                );
                let children = elements(settings)?
                    .into_iter()
                    .map(|child| Node::read(child, &scope))
                    .collect::<Result<Vec<_>>>()?;
                selected = Some(Node {
                    name: "Settings".to_string(),
                    attributes: BTreeMap::new(),
                    children,
                    text: String::new(),
                });
            }
        }
    }
    let Some(settings) = selected else {
        return Ok(None);
    };
    let has_cells = settings
        .children
        .iter()
        .filter(|node| node.name == stored::ROW_ITEM)
        .any(|node| {
            node.children.iter().any(|row| {
                row.name == stored::ROW && row.children.iter().any(|cell| cell.name == stored::CELL)
            })
        });
    if !has_cells {
        return Ok(None);
    }
    encode(&settings).map(Some)
}

fn encode(settings: &Node) -> Result<String> {
    settings.only(&[
        "languageSettings",
        "columns",
        "rowsItem",
        "templateMode",
        "defaultFormatIndex",
        "height",
        "vgRows",
        "merge",
        "font",
        "format",
    ])?;
    let language = settings
        .one("languageSettings")?
        .ok_or_else(|| anyhow!("spreadsheet has no languageSettings"))?;
    language.only(&["currentLanguage", "defaultLanguage", "languageInfo"])?;
    let current = language
        .one("currentLanguage")?
        .ok_or_else(|| anyhow!("spreadsheet has no currentLanguage"))?
        .value()?;
    let default = language
        .one("defaultLanguage")?
        .ok_or_else(|| anyhow!("spreadsheet has no defaultLanguage"))?
        .value()?;
    let mut language_ids = BTreeSet::new();
    let infos = language
        .children
        .iter()
        .filter(|child| child.name == stored::LANGUAGE_INFO)
        .map(|info| {
            info.only(&["id", "code", "description"])?;
            let id = info
                .one("id")?
                .ok_or_else(|| anyhow!("languageInfo lacks id"))?
                .value()?;
            ensure!(
                !id.is_empty() && language_ids.insert(id),
                "duplicate or empty spreadsheet language id"
            );
            ["id", "code", "description"]
                .into_iter()
                .map(|name| {
                    Ok(format_1c_string(
                        info.one(name)?
                            .ok_or_else(|| anyhow!("languageInfo lacks {name}"))?
                            .value()?,
                    ))
                })
                .collect::<Result<Vec<_>>>()
                .map(|parts| parts.join(","))
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        !infos.is_empty() && language_ids.contains(current) && language_ids.contains(default),
        "spreadsheet names an undeclared language"
    );
    let language = format!(
        "{{{},{},1,{}, {},1}}",
        format_1c_string(current),
        format_1c_string(default),
        infos.len(),
        infos.join(",")
    );
    let columns = settings
        .one("columns")?
        .ok_or_else(|| anyhow!("spreadsheet has no columns"))?;
    columns.only(&["size"])?;
    let columns = columns.number("size")?;
    let height = settings.number("height")?;
    ensure!(
        height == settings.number("vgRows")?,
        "spreadsheet vgRows does not match height"
    );
    let fonts = settings
        .children
        .iter()
        .filter(|child| child.name == stored::FONT)
        .map(font)
        .collect::<Result<Vec<_>>>()?;
    let formats = settings
        .children
        .iter()
        .filter(|child| child.name == stored::FORMAT)
        .map(|node| format(node, fonts.len()))
        .collect::<Result<Vec<_>>>()?;
    let default_index = settings.number("defaultFormatIndex")? as usize;
    let default_format = formats
        .get(
            default_index
                .checked_sub(1)
                .ok_or_else(|| anyhow!("default format index is zero"))?,
        )
        .ok_or_else(|| anyhow!("default format index is outside the format table"))?;
    let mut rows = Vec::new();
    let mut previous_row = None;
    for item in settings
        .children
        .iter()
        .filter(|child| child.name == stored::ROW_ITEM)
    {
        item.only(&["index", "row"])?;
        let index = item.number("index")?;
        ensure!(
            index < height && previous_row.is_none_or(|previous| index > previous),
            "duplicate, unordered or out-of-range spreadsheet row"
        );
        previous_row = Some(index);
        let row = item
            .one("row")?
            .ok_or_else(|| anyhow!("rowsItem has no row"))?;
        row.only(&["formatIndex", "c"])?;
        let row_format = row
            .one("formatIndex")?
            .map(|node| node.value()?.parse::<usize>().map_err(anyhow::Error::from))
            .transpose()?
            .unwrap_or(0);
        ensure!(row_format <= formats.len(), "row format is outside table");
        let mut cells = Vec::new();
        let mut next_column = 0;
        for outer in row
            .children
            .iter()
            .filter(|child| child.name == stored::CELL)
        {
            outer.only(&["i", "c"])?;
            let column = outer
                .one("i")?
                .map(|node| node.value()?.parse::<u32>().map_err(anyhow::Error::from))
                .transpose()?
                .unwrap_or(next_column);
            ensure!(
                column >= next_column && column < columns,
                "cell column is duplicate, unordered or outside columns"
            );
            next_column = column
                .checked_add(1)
                .ok_or_else(|| anyhow!("cell column overflow"))?;
            let cell = outer
                .one("c")?
                .ok_or_else(|| anyhow!("cell has no inner c"))?;
            cell.only(&["f", "tl"])?;
            let format = cell
                .one("f")?
                .map(|node| node.value()?.parse::<usize>().map_err(anyhow::Error::from))
                .transpose()?
                .unwrap_or(0);
            ensure!(format <= formats.len(), "cell format is outside table");
            let text = cell
                .one("tl")?
                .ok_or_else(|| anyhow!("embedded cell has no supported localized text"))?;
            text.only(&["v8:item"])?;
            let values = text
                .children
                .iter()
                .map(|item| {
                    item.only(&["v8:lang", "v8:content"])?;
                    let lang = item
                        .one("v8:lang")?
                        .ok_or_else(|| anyhow!("text item has no language"))?
                        .value()?;
                    let value = item
                        .one("v8:content")?
                        .ok_or_else(|| anyhow!("text item has no content"))?
                        .value()?;
                    Ok(format!(
                        "{{{},{}}}",
                        format_1c_string(lang),
                        format_1c_string(value)
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            let text = if values.is_empty() {
                "{1,0}".to_string()
            } else {
                format!("{{1,{},{}}}", values.len(), values.join(","))
            };
            cells.push((column, stored::text_cell(format, &text)));
        }
        rows.push(stored::row(index, row_format, &cells));
    }
    let merges = settings
        .children
        .iter()
        .filter(|child| child.name == stored::MERGE)
        .map(|node| {
            node.only(&["r", "c", "w", "h"])?;
            let row = node.number("r")?;
            let column = node.number("c")?;
            let across = node
                .one("w")?
                .map(|_| node.number("w"))
                .transpose()?
                .unwrap_or(0);
            let down = node
                .one("h")?
                .map(|_| node.number("h"))
                .transpose()?
                .unwrap_or(0);
            let end_column = column
                .checked_add(across)
                .ok_or_else(|| anyhow!("merge overflow"))?;
            let end_row = row
                .checked_add(down)
                .ok_or_else(|| anyhow!("merge overflow"))?;
            ensure!(
                end_column < columns && end_row < height,
                "spreadsheet merge is outside document"
            );
            Ok(format!("{{{column},{row},{end_column},{end_row},0}}"))
        })
        .collect::<Result<Vec<_>>>()?;
    let merges = if merges.is_empty() {
        "{0}".to_string()
    } else {
        format!("{{{},{}}}", merges.len(), merges.join(","))
    };
    let template = match settings.one("templateMode")?.map(Node::value).transpose()? {
        None => false,
        Some(value) => value.parse::<bool>()?,
    };
    Ok(stored::Document {
        language: &language,
        default_format,
        template,
        rows: &rows,
        columns,
        height,
        merges: &merges,
        formats: &formats,
        fonts: &fonts,
    }
    .encode())
}

fn format(node: &Node, fonts: usize) -> Result<String> {
    node.only(&["font", "fillType", "width"])?;
    let font = node
        .one("font")?
        .map(|value| value.value()?.parse::<usize>().map_err(anyhow::Error::from))
        .transpose()?;
    ensure!(
        font.is_none_or(|font| font < fonts),
        "format font is outside table"
    );
    let width = node
        .one("width")?
        .map(|_| node.number("width"))
        .transpose()?;
    let fill = match node.one("fillType")?.map(Node::value).transpose()? {
        None => false,
        Some(stored::TEXT_FILL) => true,
        _ => bail!("unsupported embedded format fillType"),
    };
    Ok(stored::text_format(font, width, fill))
}

fn font(node: &Node) -> Result<String> {
    ensure!(
        node.children.is_empty() && node.text.trim().is_empty(),
        "font has content"
    );
    let allowed = [
        "faceName",
        "height",
        "bold",
        "italic",
        "underline",
        "strikeout",
        "kind",
        "scale",
    ];
    ensure!(
        node.attributes.len() == allowed.len()
            && node
                .attributes
                .keys()
                .all(|key| allowed.contains(&key.as_str())),
        "unsupported embedded font attributes"
    );
    let get = |name: &str| {
        node.attributes
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| anyhow!("font lacks {name}"))
    };
    ensure!(
        get("kind")? == stored::ABSOLUTE_FONT,
        "unsupported embedded font kind"
    );
    let height = get("height")?.parse::<f64>()? * 10.0;
    ensure!(
        height.is_finite()
            && height.fract() == 0.0
            && height >= i32::MIN as f64
            && height <= i32::MAX as f64,
        "font height cannot be represented"
    );
    let flag = |name| get(name)?.parse::<bool>().map_err(anyhow::Error::from);
    Ok(stored::absolute_font(
        &format_1c_string(get("faceName")?),
        height as i32,
        flag("bold")?,
        flag("italic")?,
        flag("underline")?,
        flag("strikeout")?,
        get("scale")?.parse()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form() -> String {
        format!(
            "<Form xmlns=\"{FORM_NS}\" xmlns:mxl=\"{SHEET_NS}\" xmlns:v8=\"{CORE_NS}\" xmlns:xsi=\"{XSI_NS}\">\
             <Attributes><Attribute name=\"Sheet\"><Settings xsi:type=\"mxl:SpreadsheetDocument\">\
             <mxl:languageSettings><mxl:currentLanguage>ru</mxl:currentLanguage><mxl:defaultLanguage>ru</mxl:defaultLanguage>\
             <mxl:languageInfo><mxl:id>ru</mxl:id><mxl:code>Русский</mxl:code><mxl:description>Русский</mxl:description></mxl:languageInfo></mxl:languageSettings>\
             <mxl:columns><mxl:size>2</mxl:size></mxl:columns><mxl:rowsItem><mxl:index>0</mxl:index><mxl:row>\
             <mxl:c><mxl:c><mxl:f>1</mxl:f><mxl:tl><v8:item><v8:lang>ru</v8:lang><v8:content> a &amp; \"b\" </v8:content></v8:item></mxl:tl></mxl:c></mxl:c>\
             </mxl:row></mxl:rowsItem><mxl:templateMode>true</mxl:templateMode><mxl:defaultFormatIndex>1</mxl:defaultFormatIndex><mxl:height>1</mxl:height><mxl:vgRows>1</mxl:vgRows>\
             <mxl:font faceName=\"Arial\" height=\"10\" bold=\"false\" italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"Absolute\" scale=\"100\"/>\
             <mxl:format><mxl:font>0</mxl:font><mxl:fillType>Text</mxl:fillType></mxl:format>\
             </Settings></Attribute></Attributes></Form>"
        )
    }

    #[test]
    fn localized_text_keeps_spaces_entities_and_quotes() {
        let record = with_text_cells(&form(), "Sheet").unwrap().unwrap();
        assert!(
            record.contains("{16,1,{1,1,{\"ru\",\" a & \"\"b\"\" \"}},0}"),
            "{record}"
        );
        let alias = form()
            .replace("mxl:", "sheet:")
            .replace("xmlns:mxl=", "xmlns:sheet=");
        assert_eq!(with_text_cells(&alias, "Sheet").unwrap(), Some(record));
    }

    #[test]
    fn duplicate_languages_rows_and_settings_refuse() {
        let source = form();
        let language = source
            .split_once("<mxl:languageInfo>")
            .unwrap()
            .1
            .split_once("</mxl:languageInfo>")
            .unwrap()
            .0;
        let row = source
            .split_once("<mxl:rowsItem>")
            .unwrap()
            .1
            .split_once("</mxl:rowsItem>")
            .unwrap()
            .0;
        for xml in [
            source.replace(
                "</mxl:languageInfo>",
                &format!("</mxl:languageInfo><mxl:languageInfo>{language}</mxl:languageInfo>"),
            ),
            source.replace(
                &format!("<mxl:languageInfo>{language}</mxl:languageInfo>"),
                "",
            ),
            source.replace("<mxl:currentLanguage>ru", "<mxl:currentLanguage>en"),
            source.replace(
                "</mxl:rowsItem>",
                &format!("</mxl:rowsItem><mxl:rowsItem>{row}</mxl:rowsItem>"),
            ),
            source.replace("</Settings>", "</Settings><Settings/>"),
            source.replace("</Attribute>", "</Attribute><Attribute name=\"Sheet\"/>"),
        ] {
            assert!(
                with_text_cells(&xml, "Sheet").is_err(),
                "duplicate/undeclared content accepted"
            );
        }
        let out_of_order = source
            .replace("<mxl:height>1", "<mxl:height>2")
            .replace("<mxl:vgRows>1", "<mxl:vgRows>2")
            .replace("<mxl:index>0", "<mxl:index>1")
            .replace(
                "</mxl:rowsItem>",
                &format!("</mxl:rowsItem><mxl:rowsItem>{row}</mxl:rowsItem>"),
            );
        assert!(with_text_cells(&out_of_order, "Sheet").is_err());
    }

    #[test]
    fn comments_outside_cell_settings_do_not_change_existing_empty_cohorts() {
        let xml = format!(
            "<Form xmlns=\"{FORM_NS}\"><!--existing form comment--><Attributes><Attribute name=\"Sheet\"><Settings/></Attribute></Attributes></Form>"
        );
        assert_eq!(with_text_cells(&xml, "Sheet").unwrap(), None);
    }

    #[test]
    fn unclassified_cell_content_and_wrong_namespaces_refuse() {
        for (before, after) in [
            ("<mxl:f>1</mxl:f>", "<mxl:f>1</mxl:f><mxl:v>7</mxl:v>"),
            ("<mxl:f>1</mxl:f>", "<mxl:f>1</mxl:f><mxl:f>1</mxl:f>"),
            ("<mxl:tl>", "<mxl:tl extra=\"lost\">"),
            ("<mxl:c><mxl:c>", "<mxl:c><mxl:i>2</mxl:i><mxl:c>"),
            ("<mxl:f>1</mxl:f>", "<mxl:f>2</mxl:f>"),
            ("<mxl:defaultFormatIndex>1", "<mxl:defaultFormatIndex>0"),
            ("<mxl:font>0</mxl:font>", "<mxl:font>1</mxl:font>"),
            ("<mxl:vgRows>1", "<mxl:vgRows>2"),
            ("<v8:content>", "<v8:content xmlns:v8=\"urn:wrong\">"),
            ("<mxl:tl>", "<mxl:tl xmlns:mxl=\"urn:wrong\">"),
            (
                "xsi:type=\"mxl:SpreadsheetDocument\"",
                "xsi:type=\"mxl:SpreadsheetDocument\" extra=\"lost\"",
            ),
            ("<mxl:format>", "<mxl:format extra=\"lost\">"),
            ("<mxl:fillType>Text", "<mxl:fillType>Parameter"),
            ("kind=\"Absolute\"", "kind=\"Unknown\""),
            ("scale=\"100\"", "scale=\"100\" extra=\"lost\""),
            ("<mxl:tl>", "<mxl:tl><!-- lost -->"),
        ] {
            let xml = form().replace(before, after);
            assert!(
                with_text_cells(&xml, "Sheet").is_err(),
                "accepted loss: {before} -> {after}"
            );
        }
    }
}
