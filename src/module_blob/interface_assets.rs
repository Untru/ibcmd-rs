//! Base-free writers for the command-interface family of configuration
//! assets: a subsystem's and the configuration's `Ext/CommandInterface.xml`,
//! `Ext/MainSectionCommandInterface.xml`, `Ext/HomePageWorkArea.xml`,
//! `Ext/ClientApplicationInterface.xml` and
//! `Ext/StandaloneConfigurationContent.bin`.
//!
//! Every writer is the inverse of the exporter's reader for the same row and
//! reads its tables rather than restating them. Names are resolved against the
//! source tree the file belongs to; a name, an element or a value the writer
//! has no evidence for refuses the whole file -- the loader then keeps the
//! older path for it -- rather than being written from a guess.

use super::*;

use crate::compiler::bodies::command_interface::{
    CommandInterfaceModel, CommandOrder, CommandPlacement, CommandPlacementMode, CommandReference,
    CommandVisibility, SubsystemVisibility, VisibilityValue, command_interface_plaintext,
};
use crate::compiler::bodies::interface_assets::{
    ClientApplicationInterfaceModel, ClientApplicationNode, ClientApplicationPanelDef,
    HomePageWorkAreaItem, HomePageWorkAreaModel, StandaloneContentExtended, StandaloneContentModel,
    client_application_interface_plaintext, home_page_work_area_plaintext,
    standalone_content_plaintext,
};
use crate::mssql_dump::{
    COMMAND_INTERFACE_PLACEMENTS, COMMON_COMMAND_GROUPS, HOME_PAGE_WORK_AREA_TEMPLATES,
    client_application_panel_def_is_standard, command_interface_standard_command_for_code,
};
use anyhow::{bail, ensure};
use ibcmd_core::identity::ObjectUuid;

const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";

/// The asset families a writer exists for.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum InterfaceAssetSource {
    /// `Ext/CommandInterface.xml` of a subsystem or of the configuration, and
    /// `Ext/MainSectionCommandInterface.xml`.
    CommandInterface,
    HomePageWorkArea,
    ClientApplicationInterface,
    StandaloneContent,
}

/// The plain text the platform stores for `xml`, BOM included.
///
/// Without a source tree only raw spellings resolve -- `<code>:<uuid>`
/// commands, bare uuids -- and anything that names an object refuses.
pub fn interface_asset_plaintext(
    kind: InterfaceAssetSource,
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<Vec<u8>> {
    let resolver = Resolver { source };
    let root = parse_document(xml)?;
    match kind {
        InterfaceAssetSource::CommandInterface => {
            let model = command_interface_model(&root, &resolver)?;
            command_interface_plaintext(&model)
                .map_err(|error| anyhow!("the command interface does not serialize: {error}"))
        }
        InterfaceAssetSource::HomePageWorkArea => {
            let model = home_page_work_area_model(&root, &resolver)?;
            home_page_work_area_plaintext(&model)
                .map_err(|error| anyhow!("the home page work area does not serialize: {error}"))
        }
        InterfaceAssetSource::ClientApplicationInterface => {
            let model = client_application_interface_model(&root)?;
            client_application_interface_plaintext(&model).map_err(|error| {
                anyhow!("the client application interface does not serialize: {error}")
            })
        }
        InterfaceAssetSource::StandaloneContent => {
            let model = standalone_content_model(&root, xml, &resolver)?;
            standalone_content_plaintext(&model)
                .map_err(|error| anyhow!("the standalone content does not serialize: {error}"))
        }
    }
}

/// [`interface_asset_plaintext`], deflated the way a row is stored.
pub fn pack_interface_asset_blob(
    kind: InterfaceAssetSource,
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<PackedRawDeflatedBlob> {
    let plain = interface_asset_plaintext(kind, xml, source)?;
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

// ---------------------------------------------------------------------------
// The XML the writers read.
// ---------------------------------------------------------------------------

/// One element: its local name, its attributes (qualified key, value), its
/// character data and its child elements.
#[derive(Debug, Default)]
struct Node {
    name: String,
    attributes: Vec<(String, String)>,
    text: String,
    children: Vec<Node>,
}

impl Node {
    fn from_start(event: &BytesStart<'_>) -> Result<Self> {
        let mut attributes = Vec::new();
        for attribute in event.attributes() {
            let attribute = attribute?;
            attributes.push((
                String::from_utf8_lossy(attribute.key.as_ref()).to_string(),
                attribute.unescape_value()?.into_owned(),
            ));
        }
        Ok(Self {
            name: xml_local_name(event.local_name().as_ref()),
            attributes,
            ..Self::default()
        })
    }

    /// The value of the attribute whose local name is `local`.
    fn attribute(&self, local: &str) -> Option<&str> {
        self.attributes.iter().find_map(|(key, value)| {
            (key.rsplit(':').next() == Some(local)).then_some(value.as_str())
        })
    }

    fn required_attribute(&self, local: &str) -> Result<&str> {
        self.attribute(local)
            .ok_or_else(|| anyhow!("<{}> has no `{local}` attribute", self.name))
    }

    /// Refuses an attribute the writer does not read. Namespace declarations
    /// are the document's, not the element's.
    fn only_attributes(&self, allowed: &[&str]) -> Result<()> {
        for (key, _) in &self.attributes {
            if key == "xmlns" || key.starts_with("xmlns:") {
                continue;
            }
            let local = key.rsplit(':').next().unwrap_or(key);
            ensure!(
                allowed.contains(&local),
                "<{}> carries the attribute `{key}` the writer does not encode",
                self.name
            );
        }
        Ok(())
    }

    /// The element's child elements; character data between them is only
    /// the layout's whitespace.
    fn elements(&self) -> Result<&[Node]> {
        ensure!(
            self.text.trim().is_empty(),
            "<{}> mixes text with its elements",
            self.name
        );
        Ok(&self.children)
    }

    /// A leaf element's text.
    fn leaf(&self) -> Result<&str> {
        ensure!(
            self.children.is_empty(),
            "<{}> has elements where the writer expects text",
            self.name
        );
        Ok(self.text.as_str())
    }

    fn expect(&self, name: &str) -> Result<&Self> {
        ensure!(
            self.name == name,
            "<{}> stands where the writer expects <{name}>",
            self.name
        );
        Ok(self)
    }
}

/// The child elements of one element, consumed in document order.
struct Children<'a> {
    owner: &'a str,
    nodes: &'a [Node],
    next: usize,
}

impl<'a> Children<'a> {
    fn of(node: &'a Node) -> Result<Self> {
        Ok(Self {
            owner: &node.name,
            nodes: node.elements()?,
            next: 0,
        })
    }

    fn optional(&mut self, name: &str) -> Option<&'a Node> {
        let node = self.nodes.get(self.next)?;
        (node.name == name).then(|| {
            self.next += 1;
            node
        })
    }

    fn required(&mut self, name: &str) -> Result<&'a Node> {
        match self.optional(name) {
            Some(node) => Ok(node),
            None => match self.nodes.get(self.next) {
                Some(found) => bail!(
                    "<{}> has <{}> where the writer expects <{name}>",
                    self.owner,
                    found.name
                ),
                None => bail!("<{}> ends before <{name}>", self.owner),
            },
        }
    }

    fn finish(&self) -> Result<()> {
        match self.nodes.get(self.next) {
            Some(extra) => bail!(
                "<{}> has <{}>, which the writer cannot encode",
                self.owner,
                extra.name
            ),
            None => Ok(()),
        }
    }
}

fn parse_document(xml: &[u8]) -> Result<Node> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut stack = vec![Node::default()];
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => stack.push(Node::from_start(&event)?),
            Ok(Event::Empty(event)) => {
                let node = Node::from_start(&event)?;
                stack
                    .last_mut()
                    .ok_or_else(|| anyhow!("unbalanced XML"))?
                    .children
                    .push(node);
            }
            Ok(Event::End(_)) => {
                let node = stack.pop().ok_or_else(|| anyhow!("unbalanced XML"))?;
                stack
                    .last_mut()
                    .ok_or_else(|| anyhow!("unbalanced XML"))?
                    .children
                    .push(node);
            }
            Ok(Event::Text(text)) => {
                let value = text.xml_content()?;
                let value = unescape(value.as_ref())?;
                stack
                    .last_mut()
                    .ok_or_else(|| anyhow!("unbalanced XML"))?
                    .text
                    .push_str(value.as_ref());
            }
            Ok(Event::CData(text)) => {
                stack
                    .last_mut()
                    .ok_or_else(|| anyhow!("unbalanced XML"))?
                    .text
                    .push_str(text.xml_content()?.as_ref());
            }
            Ok(Event::GeneralRef(reference)) => {
                let value = if let Some(ch) = reference.resolve_char_ref()? {
                    ch.to_string()
                } else {
                    let entity = reference.decode()?;
                    resolve_xml_entity(entity.as_ref())
                        .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                        .to_string()
                };
                stack
                    .last_mut()
                    .ok_or_else(|| anyhow!("unbalanced XML"))?
                    .text
                    .push_str(&value);
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error).context("failed to parse the XML document"),
        }
        buffer.clear();
    }
    ensure!(stack.len() == 1, "the XML document is not closed");
    let mut document = stack.pop().expect("the document node");
    ensure!(
        document
            .text
            .trim_matches(|ch: char| ch.is_whitespace() || ch == '\u{feff}')
            .is_empty(),
        "the XML document has text outside its root element"
    );
    ensure!(
        document.children.len() == 1,
        "the XML document must have exactly one root element"
    );
    Ok(document.children.pop().expect("one root element"))
}

fn parse_bool(node: &Node) -> Result<bool> {
    match node.leaf()?.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => bail!("<{}> holds `{other}`, not a boolean", node.name),
    }
}

fn parse_raw_uuid(text: &str) -> Option<ObjectUuid> {
    ObjectUuid::parse(text.trim()).ok()
}

fn nil_uuid() -> ObjectUuid {
    ObjectUuid::parse(NIL_UUID).expect("the nil uuid is canonical")
}

/// `<Visibility><xr:Common>…</xr:Common><xr:Value name="…">…</xr:Value>…`.
fn parse_visibility(node: &Node, resolver: &Resolver<'_>) -> Result<(bool, Vec<VisibilityValue>)> {
    node.only_attributes(&[])?;
    let mut children = Children::of(node)?;
    let common = children.required("Common")?;
    common.only_attributes(&[])?;
    let common = parse_bool(common)?;
    let mut values = Vec::new();
    while let Some(value) = children.optional("Value") {
        value.only_attributes(&["name"])?;
        values.push(VisibilityValue {
            role: resolver.role(value.required_attribute("name")?)?,
            value: parse_bool(value)?,
        });
    }
    children.finish()?;
    Ok((common, values))
}

// ---------------------------------------------------------------------------
// Names.
// ---------------------------------------------------------------------------

/// What resolving names reads more than once, kept for the life of the
/// source context: the class and uuid of a metadata file, and the commands an
/// owner declares.
#[derive(Debug, Default)]
pub(super) struct InterfaceResolutionMemo {
    headers: BTreeMap<PathBuf, Option<(String, String)>>,
    commands: BTreeMap<PathBuf, Option<Arc<BTreeMap<String, String>>>>,
}

struct Resolver<'a> {
    source: Option<&'a MetadataSourceContext>,
}

impl Resolver<'_> {
    fn source(&self, name: &str) -> Result<&MetadataSourceContext> {
        self.source.ok_or_else(|| {
            anyhow!("`{name}` names an object, and no source tree was given to resolve it")
        })
    }

    /// `(class, uuid)` of the metadata file at `path`, read up to its first
    /// element.
    fn header(&self, source: &MetadataSourceContext, path: &Path) -> Option<(String, String)> {
        if let Ok(memo) = source.interface_memo.lock()
            && let Some(found) = memo.headers.get(path)
        {
            return found.clone();
        }
        let found = read_metadata_file_header(path);
        if let Ok(mut memo) = source.interface_memo.lock() {
            memo.headers.insert(path.to_path_buf(), found.clone());
        }
        found
    }

    /// The uuid of the metadata file at `path`, which must declare `class`.
    fn file_uuid(&self, name: &str, path: &Path, class: &str) -> Result<ObjectUuid> {
        let source = self.source(name)?;
        let (found, uuid) = self
            .header(source, path)
            .ok_or_else(|| anyhow!("`{name}` names no file of the source tree"))?;
        ensure!(found == class, "`{name}` names a {found}, not a {class}");
        ObjectUuid::parse(&uuid).map_err(|_| anyhow!("`{name}` has a malformed uuid `{uuid}`"))
    }

    /// A top-level object (`Catalog.X`) or a subsystem at any depth
    /// (`Subsystem.A.Subsystem.B`).
    fn object(&self, name: &str) -> Result<ObjectUuid> {
        let source = self.source(name)?;
        let parts = name.split('.').collect::<Vec<_>>();
        if parts.first() == Some(&"Subsystem") {
            ensure!(
                parts.len() % 2 == 0
                    && parts.iter().step_by(2).all(|part| *part == "Subsystem")
                    && parts.iter().skip(1).step_by(2).all(|part| !part.is_empty()),
                "`{name}` is not a subsystem path"
            );
            let names = parts.iter().skip(1).step_by(2).collect::<Vec<_>>();
            let mut path = source.source_root.join("Subsystems");
            for (index, part) in names.iter().enumerate() {
                path = if index + 1 == names.len() {
                    path.join(format!("{part}.xml"))
                } else {
                    path.join(part).join("Subsystems")
                };
            }
            return self.file_uuid(name, &path, "Subsystem");
        }
        let [class, object] = parts.as_slice() else {
            bail!("`{name}` is not a `<class>.<name>` reference");
        };
        let (_, folder) = metadata_reference_source_folder(name)
            .ok_or_else(|| anyhow!("the writer has no source folder for `{class}` objects"))?;
        ensure!(!object.is_empty(), "`{name}` names no object");
        self.file_uuid(
            name,
            &source
                .source_root
                .join(folder)
                .join(format!("{object}.xml")),
            class,
        )
    }

    /// A file below its owner's directory: `<folder>/<owner>/<kind>/<name>.xml`.
    fn owned_file(
        &self,
        name: &str,
        class: &str,
        owner: &str,
        directory: &str,
        child: &str,
        child_class: &str,
    ) -> Result<ObjectUuid> {
        let source = self.source(name)?;
        let (_, folder) = metadata_reference_source_folder(&format!("{class}.{owner}"))
            .ok_or_else(|| anyhow!("the writer has no source folder for `{class}` objects"))?;
        self.file_uuid(
            name,
            &source
                .source_root
                .join(folder)
                .join(owner)
                .join(directory)
                .join(format!("{child}.xml")),
            child_class,
        )
    }

    /// A command an object declares: `<class>.<owner>.Command.<name>`.
    fn nested_command(
        &self,
        name: &str,
        class: &str,
        owner: &str,
        command: &str,
    ) -> Result<ObjectUuid> {
        let source = self.source(name)?;
        let (_, folder) = metadata_reference_source_folder(&format!("{class}.{owner}"))
            .ok_or_else(|| anyhow!("the writer has no source folder for `{class}` objects"))?;
        let path = source.source_root.join(folder).join(format!("{owner}.xml"));
        let cached = source
            .interface_memo
            .lock()
            .ok()
            .and_then(|memo| memo.commands.get(&path).cloned());
        let commands = match cached {
            Some(commands) => commands,
            None => {
                let commands = fs::read(&path)
                    .ok()
                    .and_then(|xml| read_declared_commands(&xml).ok())
                    .map(Arc::new);
                if let Ok(mut memo) = source.interface_memo.lock() {
                    memo.commands.insert(path.clone(), commands.clone());
                }
                commands
            }
        };
        let uuid = commands
            .as_ref()
            .and_then(|commands| commands.get(command))
            .ok_or_else(|| anyhow!("`{name}` names no command its owner declares"))?;
        ObjectUuid::parse(uuid).map_err(|_| anyhow!("`{name}` has a malformed uuid `{uuid}`"))
    }

    /// A command as `<Command name="…">` spells it.
    fn command(&self, name: &str) -> Result<CommandReference> {
        if name == "0" {
            return Ok(CommandReference::Empty);
        }
        if let Some((code, uuid)) = name.split_once(':')
            && !code.is_empty()
            && code.bytes().all(|byte| byte.is_ascii_digit())
        {
            let code = code
                .parse::<u32>()
                .map_err(|_| anyhow!("`{name}` has an out-of-range command code"))?;
            let uuid = ObjectUuid::parse(uuid)
                .map_err(|_| anyhow!("`{name}` is not a `<code>:<uuid>` command"))?;
            return Ok(CommandReference::resolved(code, uuid));
        }
        let parts = name.split('.').collect::<Vec<_>>();
        match parts.as_slice() {
            ["CommonCommand", _] => Ok(CommandReference::resolved(0, self.object(name)?)),
            [class, owner, "Command", command] => Ok(CommandReference::resolved(
                0,
                self.nested_command(name, class, owner, command)?,
            )),
            [class, owner, "StandardCommand", standard] => {
                self.source(name)?;
                let code = standard_command_code(class, standard)?;
                let owner = self
                    .object(&format!("{class}.{owner}"))
                    .with_context(|| format!("the command `{name}`"))?;
                Ok(CommandReference::resolved(code, owner))
            }
            _ => bail!("the writer cannot name the command `{name}`"),
        }
    }

    fn command_group(&self, name: &str) -> Result<ObjectUuid> {
        if let Some((uuid, _)) = COMMON_COMMAND_GROUPS
            .iter()
            .find(|(_, standard)| *standard == name)
        {
            return Ok(ObjectUuid::parse(uuid).expect("standard group uuids are canonical"));
        }
        if name.starts_with("CommandGroup.") {
            return self.object(name);
        }
        parse_raw_uuid(name)
            .ok_or_else(|| anyhow!("the writer cannot name the command group `{name}`"))
    }

    fn subsystem(&self, name: &str) -> Result<ObjectUuid> {
        if name.is_empty() {
            return Ok(nil_uuid());
        }
        if let Some(uuid) = parse_raw_uuid(name) {
            return Ok(uuid);
        }
        ensure!(
            name.starts_with("Subsystem."),
            "the writer cannot name the subsystem `{name}`"
        );
        self.object(name)
    }

    fn role(&self, name: &str) -> Result<ObjectUuid> {
        if let Some(uuid) = parse_raw_uuid(name) {
            return Ok(uuid);
        }
        ensure!(
            name.starts_with("Role."),
            "the writer cannot name the role `{name}`"
        );
        self.object(name)
    }

    /// `CommonForm.<name>` or `<class>.<owner>.Form.<name>`.
    fn form(&self, name: &str) -> Result<ObjectUuid> {
        if let Some(uuid) = parse_raw_uuid(name) {
            return Ok(uuid);
        }
        let parts = name.split('.').collect::<Vec<_>>();
        match parts.as_slice() {
            ["CommonForm", _] => self.object(name),
            [class, owner, "Form", form] => {
                self.owned_file(name, class, owner, "Forms", form, "Form")
            }
            _ => bail!("the writer cannot name the form `{name}`"),
        }
    }

    /// A `<Metadata>` of the standalone content: an object, a subsystem at any
    /// depth, or one of an object's forms, templates, commands or fields.
    fn standalone_item(&self, name: &str) -> Result<ObjectUuid> {
        let parts = name.split('.').collect::<Vec<_>>();
        if parts.first() == Some(&"Subsystem") || parts.len() == 2 {
            return self.object(name);
        }
        match parts.as_slice() {
            [class, owner, "Form", form] => {
                self.owned_file(name, class, owner, "Forms", form, "Form")
            }
            [class, owner, "Template", template] => {
                self.owned_file(name, class, owner, "Templates", template, "Template")
            }
            [class, owner, "Command", command] => self.nested_command(name, class, owner, command),
            [class, owner, tag, field] if is_configuration_field_tag(tag) => {
                let object = self.configuration_object(name, class, owner)?;
                single_field(name, object.fields.get(*field), tag)
            }
            [class, owner, "TabularSection", section, "Attribute", field] => {
                let object = self.configuration_object(name, class, owner)?;
                let section = object.sections.get(*section).ok_or_else(|| {
                    anyhow!("`{name}` names no tabular section its owner declares")
                })?;
                single_field(name, section.get(*field), "Attribute")
            }
            _ => bail!("the writer cannot name the standalone item `{name}`"),
        }
    }

    fn configuration_object(
        &self,
        name: &str,
        class: &str,
        owner: &str,
    ) -> Result<Arc<ConfigurationObject>> {
        let source = self.source(name)?;
        source
            .configuration_object(&format!("{class}.{owner}"))
            .ok_or_else(|| anyhow!("`{name}` names an owner the writer cannot read"))
    }
}

fn single_field(
    name: &str,
    fields: Option<&Vec<ConfigurationField>>,
    tag: &str,
) -> Result<ObjectUuid> {
    let mut matching = fields
        .into_iter()
        .flatten()
        .filter(|field| field.tag == tag);
    let field = matching
        .next()
        .ok_or_else(|| anyhow!("`{name}` names no field its owner declares"))?;
    ensure!(
        matching.next().is_none(),
        "`{name}` names two fields of its owner"
    );
    ObjectUuid::parse(&field.uuid)
        .map_err(|_| anyhow!("`{name}` has a malformed uuid `{}`", field.uuid))
}

/// The code a standard command is stored under.
///
/// The exporter reads `0` and `100` alike, so its table alone cannot choose:
/// the platform stores `Open` of a constant and of a common form under `100`
/// and every other open/list command under `0` -- 209 and 3 101 entries
/// across БСП and ERP УХ, without an exception. Every other code comes from the
/// exporter's own table.
fn standard_command_code(class: &str, standard: &str) -> Result<u32> {
    let candidates: &[&str] = if matches!(class, "Constant" | "CommonForm") {
        &["100", "1", "2"]
    } else {
        &["0", "1", "2"]
    };
    candidates
        .iter()
        .find(|code| command_interface_standard_command_for_code(class, code) == Some(standard))
        .map(|code| {
            code.parse::<u32>()
                .expect("the candidate codes are numbers")
        })
        .ok_or_else(|| anyhow!("the exporter names no `{class}` standard command `{standard}`"))
}

/// `(class, uuid)` of a metadata file: the first element under
/// `<MetaDataObject>`, which is all a reference needs.
fn read_metadata_file_header(path: &Path) -> Option<(String, String)> {
    let xml = fs::read(path).ok()?;
    let mut reader = Reader::from_reader(xml.as_slice());
    let mut buffer = Vec::new();
    let mut depth = 0usize;
    let mut root_is_metadata = false;
    loop {
        match reader.read_event_into(&mut buffer).ok()? {
            Event::Start(event) => {
                let local = xml_local_name(event.local_name().as_ref());
                if depth == 1 && root_is_metadata {
                    let uuid = normalize_uuid_text(&xml_attr_value(&event, "uuid")?).ok()?;
                    return Some((local, uuid));
                }
                if depth == 0 {
                    root_is_metadata = local == "MetaDataObject";
                }
                depth += 1;
            }
            Event::Empty(event) => {
                if depth == 1 && root_is_metadata {
                    let local = xml_local_name(event.local_name().as_ref());
                    let uuid = normalize_uuid_text(&xml_attr_value(&event, "uuid")?).ok()?;
                    return Some((local, uuid));
                }
            }
            Event::End(_) => depth = depth.checked_sub(1)?,
            Event::Eof => return None,
            _ => {}
        }
        buffer.clear();
    }
}

/// Every `<ChildObjects><Command uuid="…">` of an object's own file, by name.
fn read_declared_commands(xml: &[u8]) -> Result<BTreeMap<String, String>> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut commands = BTreeMap::new();
    let mut current = None::<(String, Option<String>)>;
    let mut text = String::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "Command" && path.len() == 3 && path[2] == "ChildObjects" {
                    current = xml_attr_value(&event, "uuid")
                        .map(|uuid| normalize_uuid_text(&uuid))
                        .transpose()?
                        .map(|uuid| (uuid, None));
                }
                path.push(local);
                text.clear();
            }
            Ok(Event::Text(value)) => text.push_str(value.xml_content()?.as_ref()),
            Ok(Event::End(_)) => {
                let local = path.pop().unwrap_or_default();
                if local == "Name"
                    && path.len() == 5
                    && path[3] == "Command"
                    && path[4] == "Properties"
                    && let Some((_, name)) = current.as_mut()
                {
                    *name = Some(text.trim().to_string());
                } else if local == "Command" && path.len() == 3 {
                    if let Some((uuid, Some(name))) = current.take() {
                        commands.insert(name, uuid);
                    }
                }
                text.clear();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }
    Ok(commands)
}

// ---------------------------------------------------------------------------
// Command interfaces.
// ---------------------------------------------------------------------------

fn command_interface_model(root: &Node, resolver: &Resolver<'_>) -> Result<CommandInterfaceModel> {
    root.expect("CommandInterface")?;
    root.only_attributes(&["version"])?;
    let mut model = CommandInterfaceModel::default();
    let mut seen = BTreeSet::new();
    for section in root.elements()? {
        ensure!(
            seen.insert(section.name.as_str()),
            "<CommandInterface> repeats <{}>",
            section.name
        );
        section.only_attributes(&[])?;
        for item in section.elements()? {
            match section.name.as_str() {
                "CommandsVisibility" => {
                    item.expect("Command")?.only_attributes(&["name"])?;
                    let command = resolver.command(item.required_attribute("name")?)?;
                    let mut children = Children::of(item)?;
                    let (common, values) =
                        parse_visibility(children.required("Visibility")?, resolver)?;
                    children.finish()?;
                    model.commands_visibility.push(CommandVisibility {
                        command,
                        common,
                        values,
                    });
                }
                "CommandsPlacement" => {
                    item.expect("Command")?.only_attributes(&["name"])?;
                    let command = resolver.command(item.required_attribute("name")?)?;
                    let mut children = Children::of(item)?;
                    let command_group = resolver
                        .command_group(children.required("CommandGroup")?.leaf()?.trim())?;
                    let placement = children.required("Placement")?.leaf()?.trim();
                    children.finish()?;
                    let placement = match COMMAND_INTERFACE_PLACEMENTS
                        .iter()
                        .find(|(_, name)| *name == placement)
                        .map(|(code, _)| *code)
                    {
                        Some("0") => CommandPlacementMode::Auto,
                        Some("1") => CommandPlacementMode::Manual,
                        _ => bail!("the writer has no code for the placement `{placement}`"),
                    };
                    model.commands_placement.push(CommandPlacement {
                        command,
                        command_group,
                        placement,
                    });
                }
                "CommandsOrder" => {
                    item.expect("Command")?.only_attributes(&["name"])?;
                    let command = resolver.command(item.required_attribute("name")?)?;
                    let mut children = Children::of(item)?;
                    let command_group = resolver
                        .command_group(children.required("CommandGroup")?.leaf()?.trim())?;
                    children.finish()?;
                    model.commands_order.push(CommandOrder {
                        command_group,
                        command,
                    });
                }
                "SubsystemsVisibility" => {
                    item.expect("Subsystem")?.only_attributes(&["name"])?;
                    let name = item.required_attribute("name")?;
                    ensure!(
                        !name.is_empty(),
                        "a <SubsystemsVisibility> entry names no subsystem"
                    );
                    let subsystem = resolver.subsystem(name)?;
                    let mut children = Children::of(item)?;
                    let (common, values) =
                        parse_visibility(children.required("Visibility")?, resolver)?;
                    children.finish()?;
                    model.subsystems_visibility.push(SubsystemVisibility {
                        subsystem,
                        common,
                        values,
                    });
                }
                "SubsystemsOrder" => {
                    item.expect("Subsystem")?.only_attributes(&[])?;
                    model
                        .subsystems_order
                        .push(resolver.subsystem(item.leaf()?.trim())?);
                }
                "GroupsOrder" => {
                    item.expect("Group")?.only_attributes(&[])?;
                    model
                        .groups_order
                        .push(resolver.command_group(item.leaf()?.trim())?);
                }
                other => {
                    bail!("<CommandInterface> has the section <{other}> the writer cannot encode")
                }
            }
        }
    }
    Ok(model)
}

// ---------------------------------------------------------------------------
// The home page.
// ---------------------------------------------------------------------------

fn home_page_work_area_model(
    root: &Node,
    resolver: &Resolver<'_>,
) -> Result<HomePageWorkAreaModel> {
    root.expect("HomePageWorkArea")?;
    root.only_attributes(&["version"])?;
    let mut children = Children::of(root)?;
    let template = children.required("WorkingAreaTemplate")?;
    template.only_attributes(&[])?;
    let template_name = template.leaf()?.trim();
    let template = HOME_PAGE_WORK_AREA_TEMPLATES
        .iter()
        .find(|(_, name)| *name == template_name)
        .map(|(code, _)| *code)
        .ok_or_else(|| {
            anyhow!("the writer has no code for the work area template `{template_name}`")
        })?;
    let left_column = home_page_work_area_column(children.required("LeftColumn")?, resolver)?;
    let right_column = home_page_work_area_column(children.required("RightColumn")?, resolver)?;
    children.finish()?;
    Ok(HomePageWorkAreaModel {
        template,
        left_column,
        right_column,
    })
}

fn home_page_work_area_column(
    column: &Node,
    resolver: &Resolver<'_>,
) -> Result<Vec<HomePageWorkAreaItem>> {
    column.only_attributes(&[])?;
    let mut items = Vec::new();
    for item in column.elements()? {
        item.expect("Item")?.only_attributes(&[])?;
        let mut children = Children::of(item)?;
        let form = children.required("Form")?;
        form.only_attributes(&[])?;
        let form = resolver.form(form.leaf()?.trim())?;
        let height = children.required("Height")?;
        height.only_attributes(&[])?;
        let height = height.leaf()?.trim();
        ensure!(
            !height.is_empty() && height.bytes().all(|byte| byte.is_ascii_digit()),
            "the home page item height `{height}` is not a non-negative integer"
        );
        let (common, values) = parse_visibility(children.required("Visibility")?, resolver)?;
        children.finish()?;
        items.push(HomePageWorkAreaItem {
            form,
            height: height.to_string(),
            common,
            values,
        });
    }
    Ok(items)
}

// ---------------------------------------------------------------------------
// The client application interface.
// ---------------------------------------------------------------------------

fn client_application_interface_model(root: &Node) -> Result<ClientApplicationInterfaceModel> {
    root.expect("ClientApplicationInterface")?;
    root.only_attributes(&["type"])?;
    ensure!(
        root.attribute("type") == Some("InterfaceLayouter"),
        "<ClientApplicationInterface> is not an `InterfaceLayouter`"
    );
    let mut children = Children::of(root)?;
    let mut model = ClientApplicationInterfaceModel {
        top: Vec::new(),
        left: Vec::new(),
        bottom: Vec::new(),
        panel_defs: Vec::new(),
    };
    // The exporter writes the areas it reads -- top, left, bottom -- in
    // that order and nothing else.
    if let Some(area) = children.optional("top") {
        model.top = client_application_area(area)?;
    }
    if let Some(area) = children.optional("left") {
        model.left = client_application_area(area)?;
    }
    if let Some(area) = children.optional("bottom") {
        model.bottom = client_application_area(area)?;
    }
    while let Some(panel_def) = children.optional("panelDef") {
        panel_def.only_attributes(&["id"])?;
        ensure!(
            panel_def.elements()?.is_empty(),
            "a <panelDef> with elements is not something the writer has evidence for"
        );
        let id = panel_def.required_attribute("id")?;
        let uuid =
            parse_raw_uuid(id).ok_or_else(|| anyhow!("<panelDef> id `{id}` is not a uuid"))?;
        model.panel_defs.push(ClientApplicationPanelDef {
            id: uuid,
            standard: client_application_panel_def_is_standard(id),
        });
    }
    children.finish()?;
    Ok(model)
}

fn client_application_area(area: &Node) -> Result<Vec<ClientApplicationNode>> {
    area.only_attributes(&[])?;
    let mut nodes = Vec::new();
    for node in area.elements()? {
        match node.name.as_str() {
            "group" if node.attribute("id").is_some() => {
                nodes.push(client_application_group(node)?)
            }
            "group" => bail!(
                "an anonymous <group> directly in an area is not something the exporter writes"
            ),
            "panel" => nodes.push(client_application_panel(node)?),
            other => bail!(
                "<{}> has <{other}>, which the writer cannot encode",
                area.name
            ),
        }
    }
    Ok(nodes)
}

/// A group with an id. Its children are groups with ids and anonymous groups
/// holding one panel each -- the exporter's spelling of a panel stored
/// directly in the group.
fn client_application_group(group: &Node) -> Result<ClientApplicationNode> {
    group.only_attributes(&["id"])?;
    let id = group.required_attribute("id")?;
    let id = parse_raw_uuid(id).ok_or_else(|| anyhow!("<group> id `{id}` is not a uuid"))?;
    let mut children = Vec::new();
    for child in group.elements()? {
        child.expect("group")?;
        if child.attribute("id").is_some() {
            children.push(client_application_group(child)?);
            continue;
        }
        child.only_attributes(&[])?;
        let [panel] = child.elements()? else {
            bail!("an anonymous <group> holds other than one <panel>");
        };
        children.push(client_application_panel(panel.expect("panel")?)?);
    }
    Ok(ClientApplicationNode::Group { id, children })
}

fn client_application_panel(panel: &Node) -> Result<ClientApplicationNode> {
    panel.only_attributes(&["id"])?;
    let id = panel.required_attribute("id")?;
    let id = parse_raw_uuid(id).ok_or_else(|| anyhow!("<panel> id `{id}` is not a uuid"))?;
    let mut children = Children::of(panel)?;
    let uuid = children.required("uuid")?;
    uuid.only_attributes(&[])?;
    let text = uuid.leaf()?.trim();
    let uuid =
        parse_raw_uuid(text).ok_or_else(|| anyhow!("<panel> uuid `{text}` is not a uuid"))?;
    children.finish()?;
    Ok(ClientApplicationNode::Panel { id, uuid })
}

// ---------------------------------------------------------------------------
// The standalone configuration content.
// ---------------------------------------------------------------------------

fn standalone_content_model(
    root: &Node,
    xml: &[u8],
    resolver: &Resolver<'_>,
) -> Result<StandaloneContentModel> {
    root.expect("StandaloneContent")?;
    root.only_attributes(&["version"])?;
    let mut children = Children::of(root)?;
    let mut used = Vec::new();
    while let Some(item) = children.optional("UsedItem") {
        used.push(standalone_metadata(item, resolver, false)?);
    }
    let mut unused = Vec::new();
    while let Some(item) = children.optional("UnusedItem") {
        unused.push(standalone_metadata(item, resolver, false)?);
    }
    let mut local_server_priority = Vec::new();
    while let Some(item) = children.optional("PriorityItem") {
        local_server_priority.push(standalone_metadata(item, resolver, true)?);
    }
    let settings = children.optional("DataExchangeSettings");
    children.finish()?;
    if let Some(settings) = settings {
        settings.only_attributes(&[])?;
        let mut fields = Children::of(settings)?;
        for (name, expected) in [
            ("ExchangeOnChangeData", "true"),
            ("ExchangePeriod", "300"),
            ("TransactionCount", "1000"),
            ("InactiveNodesCleanupTimeout", "0"),
        ] {
            let field = fields.required(name)?;
            field.only_attributes(&[])?;
            let value = field.leaf()?.trim();
            ensure!(
                value == expected,
                "<{name}> is `{value}`; the writer has evidence only for `{expected}`"
            );
        }
        fields.finish()?;
    }
    // The exporter's own spelling decides the rest: it writes the data
    // exchange settings exactly when the unused list is not empty, and it
    // ends the file on a line break exactly when the record has no sections
    // beyond the used list.
    ensure!(
        settings.is_some() == !unused.is_empty(),
        "<DataExchangeSettings> is present without unused items, or missing with them"
    );
    let extended = if !unused.is_empty() || !local_server_priority.is_empty() {
        true
    } else {
        !xml.ends_with(b"\n")
    };
    Ok(StandaloneContentModel {
        used,
        extended: extended.then_some(StandaloneContentExtended {
            unused,
            local_server_priority,
        }),
    })
}

fn standalone_metadata(item: &Node, resolver: &Resolver<'_>, priority: bool) -> Result<ObjectUuid> {
    item.only_attributes(&[])?;
    let mut children = Children::of(item)?;
    let metadata = children.required("Metadata")?;
    metadata.only_attributes(&[])?;
    let uuid = resolver.standalone_item(metadata.leaf()?.trim())?;
    if priority {
        let value = children.required("Priority")?;
        value.only_attributes(&[])?;
        let value = value.leaf()?.trim();
        ensure!(
            value == "LocalServer",
            "the writer has no code for the priority `{value}`"
        );
    }
    children.finish()?;
    Ok(uuid)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG: &str = "11111111-1111-4111-8111-111111111111";
    const COMMAND: &str = "22222222-2222-4222-8222-222222222222";
    const NESTED: &str = "33333333-3333-4333-8333-333333333333";
    const GROUP: &str = "44444444-4444-4444-8444-444444444444";
    const SUBSYSTEM: &str = "55555555-5555-4555-8555-555555555555";
    const ROLE: &str = "66666666-6666-4666-8666-666666666666";
    const CONSTANT: &str = "77777777-7777-4777-8777-777777777777";

    fn metadata(root: &Path, relative: &str, body: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            format!(
                "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\">{body}</MetaDataObject>"
            ),
        )
        .unwrap();
    }

    /// Every kind of name a command interface spells, resolved against a
    /// source tree and written in the platform's layout.
    #[test]
    fn writes_every_section_with_names_resolved_against_the_source() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-interface-writer-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        metadata(
            &root,
            "Catalogs/Товары.xml",
            &format!(
                "<Catalog uuid=\"{CATALOG}\"><Properties><Name>Товары</Name></Properties><ChildObjects><Command uuid=\"{NESTED}\"><Properties><Name>Печать</Name></Properties></Command></ChildObjects></Catalog>"
            ),
        );
        metadata(
            &root,
            "CommonCommands/Открыть.xml",
            &format!("<CommonCommand uuid=\"{COMMAND}\"/>"),
        );
        metadata(
            &root,
            "CommandGroups/Группа.xml",
            &format!("<CommandGroup uuid=\"{GROUP}\"/>"),
        );
        metadata(
            &root,
            "Subsystems/А/Subsystems/Б.xml",
            &format!("<Subsystem uuid=\"{SUBSYSTEM}\"/>"),
        );
        metadata(&root, "Roles/Роль.xml", &format!("<Role uuid=\"{ROLE}\"/>"));
        metadata(
            &root,
            "Constants/Флаг.xml",
            &format!("<Constant uuid=\"{CONSTANT}\"/>"),
        );
        let xml = r#"<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<CommandsVisibility>
		<Command name="Catalog.Товары.StandardCommand.CreateFolder">
			<Visibility>
				<xr:Common>false</xr:Common>
				<xr:Value name="Role.Роль">true</xr:Value>
			</Visibility>
		</Command>
		<Command name="0">
			<Visibility>
				<xr:Common>true</xr:Common>
			</Visibility>
		</Command>
	</CommandsVisibility>
	<CommandsPlacement>
		<Command name="CommonCommand.Открыть">
			<CommandGroup>CommandGroup.Группа</CommandGroup>
			<Placement>Manual</Placement>
		</Command>
	</CommandsPlacement>
	<CommandsOrder>
		<Command name="Catalog.Товары.Command.Печать">
			<CommandGroup>NavigationPanelOrdinary</CommandGroup>
		</Command>
		<Command name="Constant.Флаг.StandardCommand.Open">
			<CommandGroup>NavigationPanelOrdinary</CommandGroup>
		</Command>
	</CommandsOrder>
	<SubsystemsVisibility>
		<Subsystem name="Subsystem.А.Subsystem.Б">
			<Visibility>
				<xr:Common>false</xr:Common>
			</Visibility>
		</Subsystem>
	</SubsystemsVisibility>
	<SubsystemsOrder>
		<Subsystem>Subsystem.А.Subsystem.Б</Subsystem>
		<Subsystem/>
	</SubsystemsOrder>
	<GroupsOrder>
		<Group>NavigationPanelOrdinary</Group>
	</GroupsOrder>
</CommandInterface>"#;
        let source = MetadataSourceContext::new(root.clone());
        let plain = interface_asset_plaintext(
            InterfaceAssetSource::CommandInterface,
            xml.as_bytes(),
            Some(&source),
        )
        .unwrap();
        let ordinary = "77ea1b8f-dd79-4717-9dba-5628e7f348cf";
        assert_eq!(
            String::from_utf8(plain).unwrap(),
            format!(
                "\u{feff}{{7,1,2,\r\n{{2,{CATALOG}}},\r\n{{0,\r\n{{0,\r\n{{\"B\",0}},1,{ROLE},\r\n{{\"B\",1}}\r\n}}\r\n}},\r\n{{0}},\r\n{{0,\r\n{{0,\r\n{{\"B\",1}},0}}\r\n}},1,1,\r\n{{0,{COMMAND}}},{GROUP},1,1,2,{ordinary},\r\n{{0,{NESTED}}},{ordinary},\r\n{{100,{CONSTANT}}},1,2,{SUBSYSTEM},{NIL_UUID},1,1,{ordinary},1,1,{SUBSYSTEM},\r\n{{0,\r\n{{0,\r\n{{\"B\",0}},0}}\r\n}}\r\n}}"
            )
        );
        // Without the tree only raw spellings resolve.
        assert!(
            interface_asset_plaintext(InterfaceAssetSource::CommandInterface, xml.as_bytes(), None)
                .is_err()
        );
        let _ = fs::remove_dir_all(root);
    }
}
