//! Reduces the ordinary converters' print of an adopted object to what the
//! platform writes for it.
//!
//! The ordinary converters print an object as if it were the extension's own:
//! every property of its kind. The platform prints an adopted object with
//! `ObjectBelonging`, `Name`, `Comment`, `ExtendedConfigurationObject` when the
//! mapping is not by identity, and only the properties the object's header
//! lists (see the module documentation of the parent module); the blocks
//! (modules, forms, rights, command interfaces) an extension overrides appear
//! as `xr:PropertyState` under `InternalInfo`.
//!
//! The converters write one element per line, indented with tabs, and an
//! element with children closes on a line of its own at the indentation of its
//! opening line. The projection edits that line structure and copies every
//! other byte through.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, anyhow, bail};

use super::properties::{Meaning, meaning};
use super::{AdoptedHeader, ExtensionContext, STATE_EXTENDED};

/// The projected text of `xml`, or `xml` itself when it holds no adopted
/// object.
pub(crate) fn project_object_xml(xml: &str, context: &ExtensionContext) -> Result<String> {
    let lines: Vec<&str> = xml.split("\r\n").collect();
    let projector = Projector {
        lines,
        context,
        adopted_seen: std::cell::Cell::new(0),
    };
    let mut output = Vec::with_capacity(projector.lines.len() + 16);
    projector.emit_range(0, projector.lines.len(), &mut output)?;
    if projector.adopted_seen.get() == 0 {
        return Ok(xml.to_owned());
    }
    Ok(output.join("\r\n"))
}

struct Projector<'a> {
    lines: Vec<&'a str>,
    context: &'a ExtensionContext,
    adopted_seen: std::cell::Cell<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineKind {
    Close,
    Declaration,
    SelfClosing,
    /// `<Tag ...>` and nothing else: children follow.
    Open,
    /// `<Tag>text</Tag>`.
    Leaf,
    /// `<Tag>text` with the text continuing on the next lines.
    LeafOpen,
    Text,
}

fn indent_of(line: &str) -> usize {
    line.bytes().take_while(|byte| *byte == b'\t').count()
}

fn classify(line: &str) -> LineKind {
    let text = line.trim_start_matches('\t');
    if text.starts_with("</") {
        return LineKind::Close;
    }
    if text.starts_with("<?") || text.starts_with("\u{feff}<?") {
        return LineKind::Declaration;
    }
    if !text.starts_with('<') {
        return LineKind::Text;
    }
    let Some(tag_end) = start_tag_end(text) else {
        return LineKind::Text;
    };
    let open_tag = &text[..=tag_end];
    let rest = &text[tag_end + 1..];
    if open_tag.ends_with("/>") {
        LineKind::SelfClosing
    } else if rest.is_empty() {
        LineKind::Open
    } else if rest.contains("</") {
        LineKind::Leaf
    } else {
        LineKind::LeafOpen
    }
}

/// The index of the `>` that ends the start tag at the beginning of `text`.
fn start_tag_end(text: &str) -> Option<usize> {
    let mut quote = None::<u8>;
    for (index, byte) in text.bytes().enumerate() {
        match (quote, byte) {
            (Some(open), byte) if byte == open => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(byte),
            (None, b'>') => return Some(index),
            _ => {}
        }
    }
    None
}

/// `Tag` of `<Tag ...>` / `<prefix:Tag ...>` (the name as written).
fn tag_name(line: &str) -> &str {
    let text = line.trim_start_matches('\t');
    let text = text.strip_prefix('<').unwrap_or(text);
    let end = text
        .find(|character: char| character.is_whitespace() || matches!(character, '>' | '/'))
        .unwrap_or(text.len());
    &text[..end]
}

fn attribute<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let start = line.find(&needle)? + needle.len();
    let end = line[start..].find('"')? + start;
    Some(&line[start..end])
}

impl Projector<'_> {
    /// The index of the last line of the element that opens on line `start`.
    fn element_end(&self, start: usize) -> Result<usize> {
        let line = self.lines[start];
        match classify(line) {
            LineKind::SelfClosing | LineKind::Leaf => Ok(start),
            LineKind::Open => {
                let indent = indent_of(line);
                (start + 1..self.lines.len())
                    .find(|&index| {
                        indent_of(self.lines[index]) == indent
                            && self.lines[index].trim_start_matches('\t').starts_with("</")
                    })
                    .ok_or_else(|| anyhow!("element on line {} never closes", start + 1))
            }
            LineKind::LeafOpen => {
                let closing = format!("</{}>", tag_name(line));
                (start + 1..self.lines.len())
                    .find(|&index| self.lines[index].ends_with(&closing))
                    .ok_or_else(|| anyhow!("text element on line {} never closes", start + 1))
            }
            other => bail!("line {} is not an element start: {other:?}", start + 1),
        }
    }

    fn emit_range(&self, from: usize, to: usize, output: &mut Vec<String>) -> Result<()> {
        let mut index = from;
        while index < to {
            let line = self.lines[index];
            match classify(line) {
                LineKind::Open | LineKind::LeafOpen => {
                    let end = self.element_end(index)?;
                    self.emit_element(index, end, output)?;
                    index = end + 1;
                }
                _ => {
                    output.push(line.to_owned());
                    index += 1;
                }
            }
        }
        Ok(())
    }

    fn emit_element(&self, start: usize, end: usize, output: &mut Vec<String>) -> Result<()> {
        let open = self.lines[start];
        if classify(open) == LineKind::LeafOpen {
            output.extend(
                self.lines[start..=end]
                    .iter()
                    .map(|line| (*line).to_owned()),
            );
            return Ok(());
        }
        if let Some(header) = attribute(open, "uuid").and_then(|uuid| self.context.adopted(uuid)) {
            return self.emit_adopted(header, start, end, output);
        }
        output.push(open.to_owned());
        self.emit_range(start + 1, end, output)?;
        output.push(self.lines[end].to_owned());
        Ok(())
    }

    fn emit_adopted(
        &self,
        header: &AdoptedHeader,
        start: usize,
        end: usize,
        output: &mut Vec<String>,
    ) -> Result<()> {
        self.adopted_seen.set(self.adopted_seen.get() + 1);
        let open = self.lines[start];
        let element = tag_name(open).to_owned();
        let indent = indent_of(open);
        let pad = |depth: usize| "\t".repeat(indent + depth);

        // The element's direct children.
        let mut children = Vec::new();
        let mut index = start + 1;
        while index < end {
            let line = self.lines[index];
            if indent_of(line) != indent + 1
                || !matches!(
                    classify(line),
                    LineKind::Open | LineKind::SelfClosing | LineKind::Leaf | LineKind::LeafOpen
                )
            {
                bail!(
                    "adopted {element} {}: unexpected line {} inside the element",
                    header.uuid,
                    index + 1
                );
            }
            let child_end = self.element_end(index)?;
            children.push((tag_name(line).to_owned(), index, child_end));
            index = child_end + 1;
        }

        // What the header lists.
        let mut printed = BTreeSet::<&'static str>::new();
        let mut states = Vec::<(&'static str, &'static str)>::new();
        // Property -> (added value as stored, whether the row's own value is
        // kept beside it as the check value).
        let mut multi_values = BTreeMap::<&'static str, (String, bool)>::new();
        let mut extended_object = false;
        for (guid, state) in &header.properties {
            let meaning = meaning(&element, guid).ok_or_else(|| {
                anyhow!(
                    "adopted {element} {} lists property {guid}, which the extension export does not know",
                    header.uuid
                )
            })?;
            match meaning {
                Meaning::ExtendedObject => extended_object = true,
                Meaning::Property(name) => {
                    printed.insert(name);
                }
                Meaning::Block(name) => {
                    if *state == STATE_EXTENDED {
                        states.push((name, "Extended"));
                    }
                }
                Meaning::Hidden(_) => {}
                Meaning::Group(name) => {
                    bail!(
                        "adopted {element} {}: the property group {name} is only known on the root",
                        header.uuid
                    );
                }
                Meaning::Multi(name) => {
                    let added = header
                        .added_values
                        .iter()
                        .find(|(added, _, _)| added == guid);
                    match added {
                        // The extension adds to the value: it keeps the value
                        // it was made against beside the addition unless the
                        // property is extended outright (state 3).
                        Some((_, _, value)) => {
                            multi_values.insert(name, (value.clone(), *state != STATE_EXTENDED));
                            states.push((name, "MultiState"));
                        }
                        None if *state == STATE_EXTENDED => bail!(
                            "adopted {element} {}: {name} is extended but the header holds no added value",
                            header.uuid
                        ),
                        None => {}
                    }
                    printed.insert(name);
                }
            }
        }
        if extended_object != header.extended_object.is_some() {
            bail!(
                "adopted {element} {}: ExtendedConfigurationObject and its property entry disagree",
                header.uuid
            );
        }
        // The modules and forms in the platform's order, then the multi-state
        // properties, whatever the order of the header list.
        let (mut blocks, multi_states): (Vec<_>, Vec<_>) = states
            .into_iter()
            .partition(|(_, state)| *state == "Extended");
        blocks.sort_by_key(|(name, _)| {
            BLOCK_STATE_ORDER
                .iter()
                .position(|known| known == name)
                .unwrap_or(usize::MAX)
        });
        let mut states = blocks;
        states.extend(multi_states);

        output.push(open.to_owned());
        let internal = children.iter().find(|(tag, _, _)| tag == "InternalInfo");
        let properties = children
            .iter()
            .find(|(tag, _, _)| tag == "Properties")
            .ok_or_else(|| anyhow!("adopted {element} {} has no Properties", header.uuid))?;

        // InternalInfo comes first; states follow whatever it already holds.
        match internal {
            Some(&(_, s, e)) if classify(self.lines[s]) == LineKind::SelfClosing => {
                debug_assert_eq!(s, e);
                self.push_internal_info(&pad(1), &[], &states, output);
            }
            Some(&(_, s, e)) => {
                output.push(self.lines[s].to_owned());
                self.emit_range(s + 1, e, output)?;
                for (name, state) in &states {
                    push_property_state(&pad(2), name, state, output);
                }
                output.push(self.lines[e].to_owned());
            }
            None => self.push_internal_info(&pad(1), &[], &states, output),
        }

        // Properties.
        let (_, properties_start, properties_end) = *properties;
        output.push(self.lines[properties_start].to_owned());
        output.push(format!(
            "{}<ObjectBelonging>Adopted</ObjectBelonging>",
            pad(2)
        ));
        let mut seen = BTreeSet::<String>::new();
        let mut index = properties_start + 1;
        while index < properties_end {
            let line = self.lines[index];
            let child_end = self.element_end(index)?;
            let tag = tag_name(line);
            let keep = match tag {
                "ObjectBelonging" => false,
                "Name" | "Comment" => true,
                other => {
                    printed.iter().any(|name| *name == other)
                        || always_printed(&element).contains(&other)
                }
            };
            if keep {
                seen.insert(tag.to_owned());
                match multi_values.get(tag) {
                    Some((value, with_check)) => {
                        // The ordinary converter prints the row's checked type.
                        // Keep its complete type/qualifier body separately from
                        // the type list the adoption header adds. The header's
                        // Recorded/Extended state decides whether to keep it,
                        // including an empty checked type of a DefinedType.
                        let with_check = *with_check;
                        let checked = if child_end > index {
                            &self.lines[index + 1..child_end]
                        } else {
                            &[]
                        };
                        output.extend(self.render_extended(
                            tag,
                            value,
                            &pad(2),
                            with_check,
                            checked,
                        )?);
                    }
                    None if classify(line) == LineKind::Open
                        && child_end == index + 1
                        && line.trim_start_matches('\t') == format!("<{tag}>") =>
                    {
                        // An element without content is written empty.
                        output.push(format!("{}<{tag}/>", pad(2)));
                    }
                    None => {
                        for copied in index..=child_end {
                            output.push(self.lines[copied].to_owned());
                        }
                    }
                }
            }
            if tag == "Comment"
                && let Some(base) = header.extended_object.as_deref()
            {
                output.push(format!(
                    "{}<ExtendedConfigurationObject>{base}</ExtendedConfigurationObject>",
                    pad(2)
                ));
            }
            index = child_end + 1;
        }
        for name in &printed {
            if !seen.contains(*name) {
                bail!(
                    "adopted {element} {}: the header lists {name} but the converter printed none",
                    header.uuid
                );
            }
        }
        // A subsystem lists the extension's objects it holds, empty when none.
        if element == "Subsystem" && !seen.contains("Content") {
            output.push(format!("{}<Content/>", pad(2)));
        }
        output.push(self.lines[properties_end].to_owned());

        // Everything else (ChildObjects, ...) as it is, adopted children
        // projected in turn.
        for (tag, child_start, child_end) in &children {
            if tag == "InternalInfo" || tag == "Properties" {
                continue;
            }
            self.emit_range(*child_start, *child_end + 1, output)?;
        }
        if prints_empty_child_objects(&element)
            && !children.iter().any(|(tag, _, _)| tag == "ChildObjects")
        {
            output.push(format!("{}<ChildObjects/>", pad(1)));
        }
        output.push(self.lines[end].to_owned());
        Ok(())
    }

    /// An `xr:ExtendedProperty` holding the type list an extension adds to a
    /// type-valued property of an adopted object.
    fn render_extended(
        &self,
        name: &str,
        value: &str,
        pad: &str,
        with_check: bool,
        checked: &[&str],
    ) -> Result<Vec<String>> {
        const TYPE_DESCRIPTION_CLASS: &str = "f5c65050-3bbb-11d5-b988-0050bae0a95d";
        let indexes = self
            .context
            .indexes()
            .ok_or_else(|| anyhow!("the reference indexes of the export are not available"))?;
        let typed = crate::mssql_dump::split_1c_braced_fields(value, 0)
            .ok_or_else(|| anyhow!("the added value of {name} is not a braced value"))?;
        if typed.len() != 3
            || typed[0].trim() != "\"#\""
            || typed[1].trim() != TYPE_DESCRIPTION_CLASS
        {
            bail!("the added value of {name} is not a type description");
        }
        let types = crate::mssql_dump::parse_command_parameter_type_pattern(
            typed[2].trim(),
            &indexes.type_index,
        )
        .ok_or_else(|| anyhow!("the added type list of {name} cannot be read"))?;
        let body = crate::mssql_dump::format_type_description_value_types_xml(
            &types,
            &format!("{pad}\t\t"),
        );
        let mut lines = vec![format!("{pad}<{name} xsi:type=\"xr:ExtendedProperty\">")];
        if with_check {
            if checked.is_empty() {
                lines.push(format!(
                    "{pad}\t<xr:CheckValue xsi:type=\"v8:TypeDescription\"/>"
                ));
            } else {
                lines.push(format!(
                    "{pad}\t<xr:CheckValue xsi:type=\"v8:TypeDescription\">"
                ));
                lines.extend(checked.iter().map(|line| format!("\t{line}")));
                lines.push(format!("{pad}\t</xr:CheckValue>"));
            }
        }
        lines.push(format!(
            "{pad}\t<xr:ExtendValue xsi:type=\"v8:TypeDescription\">"
        ));
        lines.extend(
            body.split("\r\n")
                .filter(|line| !line.is_empty())
                .map(str::to_owned),
        );
        lines.push(format!("{pad}\t</xr:ExtendValue>"));
        lines.push(format!("{pad}</{name}>"));
        Ok(lines)
    }

    /// `<InternalInfo/>`, or an `InternalInfo` holding `body` and the states.
    fn push_internal_info(
        &self,
        pad: &str,
        body: &[String],
        states: &[(&str, &str)],
        output: &mut Vec<String>,
    ) {
        if body.is_empty() && states.is_empty() {
            output.push(format!("{pad}<InternalInfo/>"));
            return;
        }
        output.push(format!("{pad}<InternalInfo>"));
        output.extend(body.iter().cloned());
        let inner = format!("{pad}\t");
        for (name, state) in states {
            push_property_state(&inner, name, state, output);
        }
        output.push(format!("{pad}</InternalInfo>"));
    }
}

/// Properties the platform prints for every adopted object of a kind, beside
/// the ones its header lists.
fn always_printed(element: &str) -> &'static [&'static str] {
    match element {
        "Document" => &["RegisterRecords"],
        // The objects of the extension in the subsystem: `<Content/>` when none
        // (fixtures `external/adopted/props_b*`).
        "Subsystem" => &["Content"],
        _ => &[],
    }
}

/// The order the platform writes the states of an adopted object's modules
/// and forms in, whatever the order of the header list (upstream PR 387,
/// `src/extension/adopted.rs`; `RecordSetModule` sits before the manager module
/// as it does in the two registers of the БСП 8.3.27 extension).
const BLOCK_STATE_ORDER: [&str; 11] = [
    "Predefined",
    "Module",
    "ObjectModule",
    "RecordSetModule",
    "ManagerModule",
    "Form",
    "CommandInterface",
    "CommandModule",
    "Content",
    "Rights",
    "MainSectionCommandInterface",
];

/// Whether an adopted object of this kind prints `<ChildObjects/>` when it
/// has no children: the registers do (`external/adopted/props_*`).
fn prints_empty_child_objects(element: &str) -> bool {
    matches!(element, "InformationRegister" | "AccumulationRegister")
}

fn push_property_state(pad: &str, name: &str, state: &str, output: &mut Vec<String>) {
    output.push(format!("{pad}<xr:PropertyState>"));
    output.push(format!("{pad}\t<xr:Property>{name}</xr:Property>"));
    output.push(format!("{pad}\t<xr:State>{state}</xr:State>"));
    output.push(format!("{pad}</xr:PropertyState>"));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(header: AdoptedHeader) -> ExtensionContext {
        ExtensionContext::new([header])
    }

    fn module_header() -> AdoptedHeader {
        AdoptedHeader {
            uuid: "eb50ccde-ac43-46b8-a693-56b559ca323a".to_owned(),
            properties: vec![
                ("9595ddd6-e72c-47ad-a156-672db811628c".to_owned(), 2),
                ("d5963243-262e-4398-b4d7-fb16d06484f6".to_owned(), 3),
                ("6275a02e-96f0-4347-975a-2d661e6a0675".to_owned(), 2),
            ],
            extended_object: Some("640d7486-8abd-40aa-a244-2ed899b7225a".to_owned()),
            added_values: Vec::new(),
        }
    }

    const MODULE_XML: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<MetaDataObject version=\"2.20\">\r\n\t<CommonModule uuid=\"eb50ccde-ac43-46b8-a693-56b559ca323a\">\r\n\t\t<Properties>\r\n\t\t\t<Name>M</Name>\r\n\t\t\t<Synonym/>\r\n\t\t\t<Comment/>\r\n\t\t\t<Global>false</Global>\r\n\t\t\t<Server>true</Server>\r\n\t\t\t<Privileged>false</Privileged>\r\n\t\t</Properties>\r\n\t</CommonModule>\r\n</MetaDataObject>";

    #[test]
    fn an_adopted_object_keeps_only_what_its_header_lists() {
        let projected = project_object_xml(MODULE_XML, &context(module_header())).unwrap();
        assert_eq!(
            projected,
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<MetaDataObject version=\"2.20\">\r\n\t<CommonModule uuid=\"eb50ccde-ac43-46b8-a693-56b559ca323a\">\r\n\t\t<InternalInfo>\r\n\t\t\t<xr:PropertyState>\r\n\t\t\t\t<xr:Property>Module</xr:Property>\r\n\t\t\t\t<xr:State>Extended</xr:State>\r\n\t\t\t</xr:PropertyState>\r\n\t\t</InternalInfo>\r\n\t\t<Properties>\r\n\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\r\n\t\t\t<Name>M</Name>\r\n\t\t\t<Comment/>\r\n\t\t\t<ExtendedConfigurationObject>640d7486-8abd-40aa-a244-2ed899b7225a</ExtendedConfigurationObject>\r\n\t\t\t<Server>true</Server>\r\n\t\t</Properties>\r\n\t</CommonModule>\r\n</MetaDataObject>"
        );
    }

    fn header(uuid: &str, properties: &[(&str, u8)]) -> AdoptedHeader {
        AdoptedHeader {
            uuid: uuid.to_owned(),
            properties: properties
                .iter()
                .map(|(guid, state)| ((*guid).to_owned(), *state))
                .collect(),
            extended_object: Some("640d7486-8abd-40aa-a244-2ed899b7225a".to_owned()),
            added_values: Vec::new(),
        }
    }

    const EXTENDED_OBJECT: (&str, u8) = ("9595ddd6-e72c-47ad-a156-672db811628c", 2);

    /// A register with no children of its own still prints `<ChildObjects/>`
    /// when adopted (`external/adopted/props_b2`); a subsystem prints its
    /// (empty) `<Content/>` whether the extension controls it or not.
    #[test]
    fn adopted_registers_and_subsystems_print_what_the_platform_prints_when_empty() {
        let register = "<MetaDataObject>\r\n\t<InformationRegister uuid=\"e3000000-0000-4000-8000-000000000006\">\r\n\t\t<Properties>\r\n\t\t\t<Name>R</Name>\r\n\t\t\t<Comment/>\r\n\t\t</Properties>\r\n\t</InformationRegister>\r\n</MetaDataObject>";
        let projected = project_object_xml(
            register,
            &context(header(
                "e3000000-0000-4000-8000-000000000006",
                &[EXTENDED_OBJECT],
            )),
        )
        .unwrap();
        assert!(
            projected
                .contains("\t\t</Properties>\r\n\t\t<ChildObjects/>\r\n\t</InformationRegister>"),
            "{projected}"
        );
        // Children the row has stay as they are.
        let with_children = register.replace(
            "\t\t</Properties>\r\n",
            "\t\t</Properties>\r\n\t\t<ChildObjects>\r\n\t\t\t<Dimension/>\r\n\t\t</ChildObjects>\r\n",
        );
        let projected = project_object_xml(
            &with_children,
            &context(header(
                "e3000000-0000-4000-8000-000000000006",
                &[EXTENDED_OBJECT],
            )),
        )
        .unwrap();
        assert_eq!(projected.matches("<ChildObjects").count(), 1, "{projected}");

        let subsystem = "<MetaDataObject>\r\n\t<Subsystem uuid=\"e3000000-0000-4000-8000-000000000007\">\r\n\t\t<Properties>\r\n\t\t\t<Name>S</Name>\r\n\t\t\t<Comment/>\r\n\t\t</Properties>\r\n\t</Subsystem>\r\n</MetaDataObject>";
        let projected = project_object_xml(
            subsystem,
            &context(header(
                "e3000000-0000-4000-8000-000000000007",
                &[EXTENDED_OBJECT],
            )),
        )
        .unwrap();
        assert!(
            projected.contains(
                "</ExtendedConfigurationObject>\r\n\t\t\t<Content/>\r\n\t\t</Properties>"
            ),
            "{projected}"
        );
    }

    /// The header of a catalog lists its modules in any order; the platform
    /// writes their states in one (fixture `external/adopted/catalog_modules`).
    #[test]
    fn the_states_of_an_adopted_object_come_out_in_the_platforms_order() {
        let catalog = "<MetaDataObject>\r\n\t<Catalog uuid=\"e0000000-0000-4000-8000-000000000002\">\r\n\t\t<Properties>\r\n\t\t\t<Name>C</Name>\r\n\t\t\t<Comment/>\r\n\t\t</Properties>\r\n\t\t<ChildObjects/>\r\n\t</Catalog>\r\n</MetaDataObject>";
        let projected = project_object_xml(
            catalog,
            &context(header(
                "e0000000-0000-4000-8000-000000000002",
                &[
                    EXTENDED_OBJECT,
                    ("d1b64a2c-8078-4982-8190-8f81aefda192", 3),
                    ("a637f77f-3840-441d-a1c3-699c8c5cb7e0", 3),
                ],
            )),
        )
        .unwrap();
        let object_module = projected.find("<xr:Property>ObjectModule<").unwrap();
        let manager_module = projected.find("<xr:Property>ManagerModule<").unwrap();
        assert!(object_module < manager_module, "{projected}");
    }

    /// Native 8.3 widened/DefinedType stores Type as Extended (state 3),
    /// with a nonempty default own type that must not become CheckValue.
    /// The 8.5 demo's two widened DefinedTypes instead record Type (state 2)
    /// with an empty own type; native export keeps their empty CheckValue.
    #[test]
    fn defined_type_checked_value_follows_adoption_state_in_both_dialects() {
        const UUID: &str = "e4000000-0000-4000-8000-000000000003";
        const TYPE: &str = "b1053250-abe6-11d4-9434-004095e12fc7";
        for dialect in ["2.20", "2.21"] {
            for (state, own) in [
                (2, "\t\t\t<Type/>"),
                (
                    3,
                    "\t\t\t<Type>\r\n\t\t\t\t<v8:Type>xs:string</v8:Type>\r\n\t\t\t</Type>",
                ),
            ] {
                let mut adoption = header(UUID, &[EXTENDED_OBJECT, (TYPE, state)]);
                adoption.added_values.push((
                    TYPE.to_owned(),
                    3,
                    "{\"#\",f5c65050-3bbb-11d5-b988-0050bae0a95d,{\"Pattern\",{\"S\",20,0}}}"
                        .to_owned(),
                ));
                let context = context(adoption);
                context.note_indexes(&BTreeMap::new(), &BTreeMap::new());
                let xml = format!(
                    "<MetaDataObject version=\"{dialect}\">\r\n\t<DefinedType uuid=\"{UUID}\">\r\n\t\t<Properties>\r\n\t\t\t<Name>D</Name>\r\n\t\t\t<Comment/>\r\n{own}\r\n\t\t</Properties>\r\n\t</DefinedType>\r\n</MetaDataObject>"
                );
                let projected = project_object_xml(&xml, &context).unwrap();
                assert_eq!(
                    projected.contains("<xr:CheckValue xsi:type=\"v8:TypeDescription\"/>"),
                    state == 2,
                    "{dialect}, state {state}: {projected}"
                );
                assert!(!projected.contains("<xr:CheckValue xsi:type=\"v8:TypeDescription\">"));
                assert!(projected.contains("<xr:ExtendValue xsi:type=\"v8:TypeDescription\">"));
                assert!(projected.contains("<v8:Length>20</v8:Length>"));
                assert!(projected.contains("<xr:State>MultiState</xr:State>"));
                if state == 3 {
                    assert!(!projected.contains("<v8:Type>xs:string</v8:Type>\r\n\t\t\t</Type>"));
                }
            }
        }
    }

    #[test]
    fn xml_without_adopted_objects_is_returned_unchanged() {
        let other = ExtensionContext::new([]);
        assert_eq!(project_object_xml(MODULE_XML, &other).unwrap(), MODULE_XML);
    }

    #[test]
    fn an_unknown_property_id_is_refused() {
        let mut header = module_header();
        header
            .properties
            .push(("00000000-1111-2222-3333-444444444444".to_owned(), 2));
        assert!(project_object_xml(MODULE_XML, &context(header)).is_err());
    }
}
