//! The attribute body every attribute-like object shares: attributes of
//! objects and tabular sections, register dimensions/resources/attributes,
//! accounting flags, constants, common attributes. Owned by the
//! simple-objects track.
//!
//! Public API (stable; extended additively):
//! - [`typed_header`]: `{2,<md_base>,<type pattern>}`, the named-and-typed
//!   head (a session parameter's whole payload; the head of every body).
//! - [`attribute_body`]: the 23-slot `{27,...}` body read from an element's
//!   `<Properties>`:
//!
//!   ```text
//!   {27,{2,<md_base>,<pattern>},
//!    [2] PasswordMode, [3] Format, [4] ToolTip, [5] MarkNegatives, [6] Mask,
//!    [7] MultiLine, [8] MinValue, [9] MaxValue, [10] ChoiceFoldersAndItems,
//!    [11] ChoiceForm, [12] QuickChoice, [13] FillChecking,
//!    [14] ChoiceParameterLinks, [15] LinkByType, [16] ChoiceParameters,
//!    [17] ExtendedEdit, [18] EditFormat, [19] FillValue,
//!    [20] FillFromFillingValue, [21] CreateOnInput, [22] ChoiceHistoryOnInput}
//!   ```
//!
//!   Owners wrap it with their own slots: a catalog/document attribute is
//!   `{{5,<body>,<Use>,<Indexing>,<FullTextSearch>,<DataHistory>},0}` (see the
//!   objects track), a constant `{16,<body>,<generated ids>...}`.
//! - Single-property encoders for owners that write the same properties in
//!   their own slots: [`choice_parameter_links`], [`link_by_type`],
//!   [`choice_parameters`], [`fill_checking`], [`quick_choice`],
//!   [`create_on_input`], [`choice_history_on_input`],
//!   [`choice_folders_and_items`], [`bool_prop`], [`localized_prop`],
//!   [`string_prop`].

use anyhow::{Result, anyhow, bail};

use super::brace::Brace;
use super::types::{data_path, empty_string_value, form_uuid, type_pattern, typed_value};
use super::xml::Element;
use super::{DescriptorContext, localized, md_base, native_text, parse_bool};
use ibcmd_restructure::brace_list;

#[path = "attribute_export.rs"]
pub(crate) mod export;

// The stored codes of the body's enumerations, read by both directions.
pub(crate) const CHOICE_FOLDERS_AND_ITEMS: &[(&str, i64)] =
    &[("Items", 0), ("Folders", 1), ("FoldersAndItems", 2)];
pub(crate) const QUICK_CHOICE: &[(&str, i64)] = &[("DontUse", 0), ("Use", 1), ("Auto", 2)];
pub(crate) const FILL_CHECKING: &[(&str, i64)] = &[("DontCheck", 0), ("ShowError", 1)];
pub(crate) const CREATE_ON_INPUT: &[(&str, i64)] = &[("Auto", 0), ("DontUse", 1), ("Use", 2)];
pub(crate) const CHOICE_HISTORY_ON_INPUT: &[(&str, i64)] = &[("Auto", 0), ("DontUse", 1)];

/// `{2,<md_base>,<type pattern>}`.
pub fn typed_header(
    uuid: &str,
    properties: &Element,
    context: &DescriptorContext,
) -> Result<Brace> {
    Ok(brace_list![
        Brace::num(2),
        md_base(uuid, properties),
        type_pattern(properties.child("Type"), context)?,
    ])
}

/// The `{27,...}` attribute body of an element's `<Properties>`.
pub fn attribute_body(
    uuid: &str,
    properties: &Element,
    context: &DescriptorContext,
) -> Result<Brace> {
    let fill_value = match properties.child("FillValue") {
        // Owners without a fill value (constants) store the empty string.
        None => empty_string_value(),
        Some(value) => typed_value(Some(value), context)?,
    };
    Ok(brace_list![
        Brace::num(27),
        typed_header(uuid, properties, context)?,
        bool_prop(properties, "PasswordMode")?,
        localized_prop(properties, "Format"),
        localized_prop(properties, "ToolTip"),
        bool_prop(properties, "MarkNegatives")?,
        string_prop(properties, "Mask"),
        bool_prop(properties, "MultiLine")?,
        typed_value(properties.child("MinValue"), context)?,
        typed_value(properties.child("MaxValue"), context)?,
        choice_folders_and_items(properties.child_text("ChoiceFoldersAndItems"))?,
        Brace::uuid(&form_uuid(
            properties.child_text("ChoiceForm").unwrap_or_default(),
            context
        )?),
        quick_choice(properties.child_text("QuickChoice"))?,
        fill_checking(properties.child_text("FillChecking"))?,
        choice_parameter_links(properties.child("ChoiceParameterLinks"), context)?,
        link_by_type(properties.child("LinkByType"), context)?,
        choice_parameters(properties.child("ChoiceParameters"), context)?,
        bool_prop(properties, "ExtendedEdit")?,
        localized_prop(properties, "EditFormat"),
        fill_value,
        bool_prop(properties, "FillFromFillingValue")?,
        create_on_input(properties.child_text("CreateOnInput"))?,
        choice_history_on_input(properties.child_text("ChoiceHistoryOnInput"))?,
    ])
}

/// A `true`/`false` property as `1`/`0`; absent is `0`.
pub fn bool_prop(properties: &Element, name: &str) -> Result<Brace> {
    match properties.child_text(name) {
        None => Ok(Brace::flag(false)),
        Some(text) => Ok(Brace::flag(
            parse_bool(text.trim()).map_err(|error| anyhow!("<{name}>: {error}"))?,
        )),
    }
}

/// A localized string property (`{N,"lang","text",...}` or `{0}`).
pub fn localized_prop(properties: &Element, name: &str) -> Brace {
    localized(properties.child(name))
}

/// A plain string property as a quoted string; absent is `""`.
pub fn string_prop(properties: &Element, name: &str) -> Brace {
    Brace::str(native_text(properties.child_text(name).unwrap_or_default()))
}

fn enumerated(
    value: Option<&str>,
    name: &str,
    table: &[(&str, i64)],
    absent: i64,
) -> Result<Brace> {
    let Some(value) = value else {
        return Ok(Brace::num(absent));
    };
    table
        .iter()
        .find(|(candidate, _)| *candidate == value.trim())
        .map(|(_, code)| Brace::num(*code))
        .ok_or_else(|| anyhow!("unknown {name} {value:?}"))
}

/// `DontCheck` 0, `ShowError` 1.
pub fn fill_checking(value: Option<&str>) -> Result<Brace> {
    enumerated(value, "FillChecking", FILL_CHECKING, 0)
}

/// `DontUse` 0, `Use` 1, `Auto` 2.
pub fn quick_choice(value: Option<&str>) -> Result<Brace> {
    enumerated(value, "QuickChoice", QUICK_CHOICE, 2)
}

/// `Auto` 0, `DontUse` 1, `Use` 2.
pub fn create_on_input(value: Option<&str>) -> Result<Brace> {
    enumerated(value, "CreateOnInput", CREATE_ON_INPUT, 0)
}

/// `Auto` 0, `DontUse` 1.
pub fn choice_history_on_input(value: Option<&str>) -> Result<Brace> {
    enumerated(value, "ChoiceHistoryOnInput", CHOICE_HISTORY_ON_INPUT, 0)
}

/// `Items` 0, `Folders` 1, `FoldersAndItems` 2.
pub fn choice_folders_and_items(value: Option<&str>) -> Result<Brace> {
    enumerated(value, "ChoiceFoldersAndItems", CHOICE_FOLDERS_AND_ITEMS, 0)
}

/// `{5006,<count>,"<name>",<segment count>,<segment>...,<0 clear | 1 don't change>,...}`.
pub fn choice_parameter_links(
    links: Option<&Element>,
    context: &DescriptorContext,
) -> Result<Brace> {
    let mut items = vec![Brace::num(5006), Brace::num(0)];
    let mut count = 0;
    if let Some(links) = links {
        for link in links.children_named("Link") {
            count += 1;
            items.push(Brace::str(link.child_text("Name").unwrap_or_default()));
            let segments = data_path(link.child_text("DataPath").unwrap_or_default(), context)?;
            items.push(Brace::num(segments.len() as i64));
            items.extend(segments);
            items.push(match link.child_text("ValueChange").unwrap_or("Clear") {
                "Clear" => Brace::num(0),
                "DontChange" => Brace::num(1),
                other => bail!("unknown ValueChange {other:?}"),
            });
        }
    }
    items[1] = Brace::num(count);
    Ok(Brace::List(items))
}

/// `{3,<segment count>,<segment>...,<link item>}`; `{3,0,0}` when empty.
pub fn link_by_type(link: Option<&Element>, context: &DescriptorContext) -> Result<Brace> {
    let Some(link) = link.filter(|link| link.child("DataPath").is_some()) else {
        return Ok(brace_list![Brace::num(3), Brace::num(0), Brace::num(0)]);
    };
    let segments = data_path(link.child_text("DataPath").unwrap_or_default(), context)?;
    let mut items = vec![Brace::num(3), Brace::num(segments.len() as i64)];
    items.extend(segments);
    let link_item = link.child_text("LinkItem").unwrap_or("0").trim();
    items.push(Brace::atom(link_item));
    Ok(Brace::List(items))
}

/// `{0,<count>,"<name>",<typed value>,...}`; `{0,0}` when empty.
pub fn choice_parameters(
    parameters: Option<&Element>,
    context: &DescriptorContext,
) -> Result<Brace> {
    let mut items = vec![Brace::num(0), Brace::num(0)];
    let mut count = 0;
    if let Some(parameters) = parameters {
        for item in parameters.children_named("item") {
            count += 1;
            items.push(Brace::str(item.attr("name").unwrap_or_default()));
            items.push(typed_value(item.child("value"), context)?);
        }
    }
    items[1] = Brace::num(count);
    Ok(Brace::List(items))
}

#[cfg(test)]
mod tests {
    use super::super::brace::{NIL_UUID, serialize};
    use super::super::types::DESIGN_TIME_REF_TYPE;
    use super::super::types::tests::{VALUTA_TYPE, context, element};
    use super::*;

    const UUID: &str = "63eb6a1b-5f48-461a-9a6f-821db663c9d3";

    fn body(properties: &str) -> String {
        let properties = element(properties);
        serialize(&attribute_body(UUID, &properties, &context()).unwrap()).replace("\r\n", "")
    }

    #[test]
    fn a_constant_body_defaults_every_slot() {
        // BSP `Constants/_ДемоИмяКонфигурацииВОбменеСБиблиотекойСтандартныхПодсистем`.
        let actual = body(
            r##"<Properties><Name>Имя</Name><Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Синоним</v8:content></v8:item></Synonym><Comment/><Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>15</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers></Type><PasswordMode>false</PasswordMode><Format/><EditFormat/><ToolTip/><MarkNegatives>false</MarkNegatives><Mask/><MultiLine>false</MultiLine><ExtendedEdit>false</ExtendedEdit><MinValue xsi:nil="true"/><MaxValue xsi:nil="true"/><FillChecking>DontCheck</FillChecking><ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems><ChoiceParameterLinks/><ChoiceParameters/><QuickChoice>Auto</QuickChoice><ChoiceForm/><LinkByType/><ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput></Properties>"##,
        );
        assert_eq!(
            actual,
            format!(
                r##"{{27,{{2,{{3,{{1,0,{UUID}}},"Имя",{{1,"ru","Синоним"}},"",0,0,{NIL_UUID},0}},{{"Pattern",{{"S",15,1}}}}}},0,{{0}},{{0}},0,"",0,{{"U"}},{{"U"}},0,{NIL_UUID},2,0,{{5006,0}},{{3,0,0}},{{0,0}},0,{{0}},{{"S",""}},0,0,0}}"##
            )
        );
    }

    #[test]
    fn a_reference_attribute_body_with_links_and_parameters() {
        let actual = body(
            r##"<Properties><Name>Валюта</Name><Synonym/><Comment/><Type><v8:Type>cfg:CatalogRef.Валюты</v8:Type></Type><PasswordMode>false</PasswordMode><Format/><EditFormat/><ToolTip><v8:item><v8:lang>ru</v8:lang><v8:content>Подсказка</v8:content></v8:item></ToolTip><MarkNegatives>false</MarkNegatives><Mask/><MultiLine>false</MultiLine><ExtendedEdit>true</ExtendedEdit><MinValue xsi:nil="true"/><MaxValue xsi:type="xs:string">100</MaxValue><FillFromFillingValue>true</FillFromFillingValue><FillValue xsi:type="xr:DesignTimeRef">Catalog.Валюты.EmptyRef</FillValue><FillChecking>ShowError</FillChecking><ChoiceFoldersAndItems>FoldersAndItems</ChoiceFoldersAndItems><ChoiceParameterLinks><xr:Link><xr:Name>Отбор.Владелец</xr:Name><xr:DataPath xsi:type="xs:string">Catalog.Счета.StandardAttribute.Owner</xr:DataPath><xr:ValueChange>DontChange</xr:ValueChange></xr:Link></ChoiceParameterLinks><ChoiceParameters><app:item name="Отбор.ПометкаУдаления"><app:value xsi:type="xs:boolean">false</app:value></app:item></ChoiceParameters><QuickChoice>DontUse</QuickChoice><CreateOnInput>Use</CreateOnInput><ChoiceForm/><LinkByType><xr:DataPath>Catalog.Счета.Attribute.Банк</xr:DataPath><xr:LinkItem>3</xr:LinkItem></LinkByType><ChoiceHistoryOnInput>DontUse</ChoiceHistoryOnInput></Properties>"##,
        );
        let bank = super::super::types::tests::ATTRIBUTE;
        assert_eq!(
            actual,
            format!(
                r##"{{27,{{2,{{3,{{1,0,{UUID}}},"Валюта",{{0}},"",0,0,{NIL_UUID},0}},{{"Pattern",{{"#",{VALUTA_TYPE}}}}}}},0,{{0}},{{1,"ru","Подсказка"}},0,"",0,{{"U"}},{{"S","100"}},2,{NIL_UUID},0,1,{{5006,1,"Отбор.Владелец",1,{{-5}},1}},{{3,1,{{0,{bank}}},3}},{{0,1,"Отбор.ПометкаУдаления",{{"B",0}}}},1,{{0}},{{"#",{DESIGN_TIME_REF_TYPE},{{0,{VALUTA_TYPE},{NIL_UUID}}}}},1,2,1}}"##
            )
        );
    }
}

/// Offline measurement of [`attribute_body`] against every attribute-like
/// child the owners' stored rows carry (catalog/document/register
/// attributes, tabular-section attributes, dimensions, resources, ...), not
/// only the top-level objects the descriptor audit compiles.
pub mod corpus {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    use anyhow::Result;
    use rayon::prelude::*;
    use serde::Serialize;

    use super::super::DescriptorContext;
    use super::super::audit::{descriptor_xmls, read_stored_row};
    use super::super::brace::{Brace, parse_row, serialize};
    use super::super::xml::{Element, MetadataXml};
    use super::attribute_body;
    use ibcmd_parallel as parallel;

    #[derive(Debug, Default, Serialize)]
    pub struct BodyAuditReport {
        pub total: usize,
        pub identical: usize,
        pub different: usize,
        pub failed: usize,
        /// The stored row names the child but holds no `{27,...}` body for it.
        pub no_body: usize,
        /// First differing slot (`1.p` = type pattern, `1.m` = md_base), by
        /// child path (`Catalog.Attribute`, `Document.TabularSection.Attribute`).
        pub slots: BTreeMap<String, usize>,
        pub failures: BTreeMap<String, usize>,
        pub samples: Vec<BodySample>,
    }

    #[derive(Debug, Serialize)]
    pub struct BodySample {
        pub file: String,
        pub child: String,
        pub slot: String,
        pub expected: String,
        pub actual: String,
    }

    enum Outcome {
        Identical,
        Different(String, String, String, String),
        Failed(String, String),
        NoBody,
    }

    fn typed_children<'a>(
        element: &'a Element,
        parent: &str,
        out: &mut Vec<(&'a Element, String)>,
    ) {
        if let Some(children) = element.child("ChildObjects") {
            for child in &children.children {
                let path = format!("{parent}.{}", child.name);
                let has_type = child
                    .child("Properties")
                    .is_some_and(|properties| properties.child("Type").is_some());
                if has_type && child.attr("uuid").is_some() {
                    out.push((child, path.clone()));
                }
                typed_children(child, &path, out);
            }
        }
    }

    fn base_uuid(node: Option<&Brace>) -> Option<&str> {
        let base = node?.as_list()?;
        if base.first().and_then(Brace::as_atom) != Some("3") {
            return None;
        }
        base.get(1)?.at(&[2])?.as_atom()
    }

    fn find_body<'a>(node: &'a Brace, uuid: &str) -> Option<&'a Brace> {
        let items = node.as_list()?;
        // A body is {27,{2,{3,{1,0,<uuid>},...},<pattern>},...}.
        if items.first().and_then(Brace::as_atom) == Some("27")
            && let Some(head) = items.get(1).and_then(Brace::as_list)
            && head.first().and_then(Brace::as_atom) == Some("2")
            && base_uuid(head.get(1)) == Some(uuid)
        {
            return Some(node);
        }
        items.iter().find_map(|item| find_body(item, uuid))
    }

    fn has_md_base(node: &Brace, uuid: &str) -> bool {
        if base_uuid(Some(node)) == Some(uuid) {
            return true;
        }
        node.as_list()
            .is_some_and(|items| items.iter().any(|item| has_md_base(item, uuid)))
    }

    fn first_slot(expected: &Brace, actual: &Brace) -> String {
        let (Some(left), Some(right)) = (expected.as_list(), actual.as_list()) else {
            return "?".into();
        };
        for index in 0..left.len().max(right.len()) {
            if left.get(index) != right.get(index) {
                if index == 1 {
                    let pattern = |node: Option<&Brace>| node.and_then(|n| n.at(&[2])).cloned();
                    return if pattern(left.get(1)) != pattern(right.get(1)) {
                        "1.p".into()
                    } else {
                        "1.m".into()
                    };
                }
                return index.to_string();
            }
        }
        "len".into()
    }

    fn measure(
        root: &Path,
        rows: &Path,
        path: &Path,
        context: &DescriptorContext,
    ) -> Result<Vec<(String, Outcome)>> {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(bytes) = fs::read(path) else {
            return Ok(Vec::new());
        };
        let Ok(doc) = MetadataXml::parse(&bytes) else {
            return Ok(Vec::new());
        };
        let Ok(object) = doc.object() else {
            return Ok(Vec::new());
        };
        let Some(uuid) = object.attr("uuid") else {
            return Ok(Vec::new());
        };
        let mut children = Vec::new();
        typed_children(object, &object.name, &mut children);
        if children.is_empty() {
            return Ok(Vec::new());
        }
        let Some(stored) = read_stored_row(rows, &uuid.to_ascii_lowercase())? else {
            return Ok(Vec::new());
        };
        let Ok(tree) = parse_row(&stored) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for (child, tag) in children {
            let child_uuid = child.attr("uuid").unwrap_or_default().to_ascii_lowercase();
            let Some(expected) = find_body(&tree, &child_uuid) else {
                if has_md_base(&tree, &child_uuid) {
                    out.push((tag, Outcome::NoBody));
                }
                continue;
            };
            let Some(properties) = child.child("Properties") else {
                continue;
            };
            let outcome = match attribute_body(&child_uuid, properties, context) {
                Err(error) => Outcome::Failed(
                    relative.clone(),
                    format!("{error:#}")
                        .lines()
                        .next()
                        .unwrap_or("")
                        .to_string(),
                ),
                Ok(actual) if &actual == expected => Outcome::Identical,
                Ok(actual) => Outcome::Different(
                    relative.clone(),
                    first_slot(expected, &actual),
                    serialize(expected),
                    serialize(&actual),
                ),
            };
            out.push((tag, outcome));
        }
        Ok(out)
    }

    pub fn audit(
        root: &Path,
        rows: &Path,
        version: &str,
        max_samples: usize,
    ) -> Result<BodyAuditReport> {
        let context = DescriptorContext::new(root, version)?;
        let paths = descriptor_xmls(root);
        let outcomes = parallel::install(|| {
            paths
                .par_iter()
                .map(|path| measure(root, rows, path, &context))
                .collect::<Result<Vec<_>>>()
        })??;
        let mut report = BodyAuditReport::default();
        for (tag, outcome) in outcomes.into_iter().flatten() {
            report.total += 1;
            match outcome {
                Outcome::Identical => report.identical += 1,
                Outcome::NoBody => report.no_body += 1,
                Outcome::Failed(file, message) => {
                    report.failed += 1;
                    let key: String = message.chars().take(160).collect();
                    *report.failures.entry(key.clone()).or_default() += 1;
                    let same = report.samples.iter().filter(|s| s.actual == key).count();
                    if report.samples.len() < max_samples && same < 2 {
                        report.samples.push(BodySample {
                            file,
                            child: tag,
                            slot: "fail".into(),
                            expected: String::new(),
                            actual: key,
                        });
                    }
                }
                Outcome::Different(file, slot, expected, actual) => {
                    report.different += 1;
                    *report.slots.entry(format!("{tag} @ {slot}")).or_default() += 1;
                    let same = report.samples.iter().filter(|s| s.slot == slot).count();
                    if report.samples.len() < max_samples && same < 5 {
                        report.samples.push(BodySample {
                            file,
                            child: tag,
                            slot,
                            expected,
                            actual,
                        });
                    }
                }
            }
        }
        Ok(report)
    }
}
