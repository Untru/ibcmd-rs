//! A dynamic list's settings bag -- member 14 of its attribute's `{9,…}`
//! record -- built from `Form.xml` and the configuration alone.
//!
//! Two parts of the bag are not a function of the source: the field map
//! (`FieldsMapItem…`, `ReqMapFieldId…`) and `ServerState`. The export reads
//! the map only through the references into it, and prints `ServerState` as
//! the `<Settings>` children it decodes, so a map numbered from the form's own
//! references and a `ServerState` re-spelled from those children export back
//! to the same `Form.xml`: 273 of 273 BSP and 3 869 of 3 869 ERP УХ
//! dynamic-list forms (`findings/rt-dynamic-list.md`).

use std::collections::BTreeMap;

use anyhow::{Result, anyhow};

const CORE: &str = "http://v8.1c.ru/8.1/data/core";
const SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
const UI: &str = "http://v8.1c.ru/8.1/data/ui";
const DCSCOR: &str = "http://v8.1c.ru/8.1/data-composition-system/core";
const DCSSET: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
const XS: &str = "http://www.w3.org/2001/XMLSchema";
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
const CFG: &str = "http://v8.1c.ru/8.1/data/enterprise/current-config";
const STYLE: &str = "http://v8.1c.ru/8.1/data/ui/style";
const SYS_FONTS: &str = "http://v8.1c.ru/8.1/data/ui/fonts/system";

/// The namespaces `Form.xml`'s root declares.
const FORM_ROOT_NS: &[(&str, &str)] = &[
    ("", "http://v8.1c.ru/8.3/xcf/logform"),
    ("app", "http://v8.1c.ru/8.2/managed-application/core"),
    ("cfg", CFG),
    ("dcscor", DCSCOR),
    ("dcssch", SCHEMA),
    ("dcsset", DCSSET),
    ("ent", "http://v8.1c.ru/8.1/data/enterprise"),
    ("lf", "http://v8.1c.ru/8.2/managed-application/logform"),
    ("style", STYLE),
    ("sys", SYS_FONTS),
    ("v8", CORE),
    ("v8ui", UI),
    ("web", "http://v8.1c.ru/8.1/data/ui/colors/web"),
    ("win", "http://v8.1c.ru/8.1/data/ui/colors/windows"),
    ("xr", "http://v8.1c.ru/8.3/xcf/readable"),
    ("xs", XS),
    ("xsi", XSI),
];

/// The namespaces the storage writer declares under a fixed prefix; every
/// other one gets a generated `d<depth>p<n>`.
fn fixed_prefix(namespace: &str) -> Option<&'static str> {
    match namespace {
        SCHEMA => Some("dcssch"),
        DCSCOR => Some("dcscor"),
        DCSSET => Some("dcsset"),
        XS => Some("xs"),
        XSI => Some("xsi"),
        _ => None,
    }
}

/// The members of a dynamic list with fixed ids, which are not map entries.
const PSEUDO_MEMBERS: &[(&str, &str)] = &[
    ("DefaultPicture", "10000000"),
    ("Order", "-1"),
    ("Filter", "-2"),
    ("SettingsComposer", "-6"),
    ("Group", "-3"),
];

fn pseudo_id(name: &str) -> Option<&'static str> {
    PSEUDO_MEMBERS
        .iter()
        .find_map(|(candidate, id)| (*candidate == name).then_some(*id))
}

/// A `Form.xml` path into a dynamic list: `N.X[.Y]`, `Items.<T>.CurrentData.X`
/// for a table `T` bound to `N`, and either with the `~` marker the export
/// writes for a field it cannot resolve -- `~N.X`, or `~N.X~N.Y` with a twin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListPath {
    pub(crate) marker: bool,
    pub(crate) rest: String,
    pub(crate) twin: Option<String>,
}

pub(crate) fn parse_list_path(text: &str, list: &str, tables: &[String]) -> Option<ListPath> {
    let mut body = text.trim();
    let mut marker = false;
    let mut twin = None;
    if let Some(stripped) = body.strip_prefix('~') {
        marker = true;
        match stripped.find('~') {
            Some(at) => {
                body = &stripped[..at];
                twin = Some(stripped[at + 1..].to_string());
            }
            None => body = stripped,
        }
    }
    let head = format!("{list}.");
    let rest = match body.strip_prefix(&head) {
        Some(rest) => rest.to_string(),
        None => tables.iter().find_map(|table| {
            body.strip_prefix(&format!("Items.{table}.CurrentData."))
                .map(str::to_string)
        })?,
    };
    if rest.is_empty() {
        return None;
    }
    let twin = twin.map(|twin| match twin.strip_prefix(&head) {
        Some(rest) => rest.to_string(),
        None => twin,
    });
    Some(ListPath { marker, rest, twin })
}

fn prefixes(rest: &str) -> Vec<String> {
    let parts = rest.split('.').collect::<Vec<_>>();
    (1..=parts.len()).map(|count| parts[..count].join(".")).collect()
}

/// A marked entry's variant: `Some(twin)`; `None` for a plain one.
type Variant = Option<Option<String>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldEntry {
    pub(crate) id: usize,
    pub(crate) name: String,
    pub(crate) secondary: Option<String>,
    pub(crate) variant: Variant,
    /// A plain entry no plain reference walks through: it exists only so the
    /// marked (`~`) entry of its name reads as shadowed.
    pub(crate) claim_only: bool,
}

/// The synthetic field map of one dynamic list.
///
/// Entries are keyed by name and variant: plain, or marked with a twin. A
/// marked entry is always preceded by the plain entry of its name, which
/// claims the name, so the export reads the marked one as shadowed -- `~`
/// whatever the list's field universe holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FieldMap {
    pub(crate) entries: Vec<FieldEntry>,
    by_key: BTreeMap<(String, Variant), usize>,
}

impl FieldMap {
    fn add(&mut self, name: &str, variant: Variant, secondary: Option<String>) -> usize {
        self.add_entry(name, variant, secondary, false)
    }

    /// `claim`: the plain entry a marked one needs before it, added for that
    /// alone; a plain reference to the same name later makes it a real one.
    fn add_entry(&mut self, name: &str, variant: Variant, secondary: Option<String>, claim: bool) -> usize {
        let key = (name.to_string(), variant.clone());
        if let Some(index) = self.by_key.get(&key) {
            if !claim {
                self.entries[*index].claim_only = false;
            }
            return self.entries[*index].id;
        }
        if variant.is_some() && !self.by_key.contains_key(&(name.to_string(), None)) {
            self.add_entry(name, None, None, true);
        }
        let id = self.entries.len() + 1;
        let claim_only = claim && variant.is_none();
        self.entries.push(FieldEntry {
            id,
            name: name.to_string(),
            secondary,
            variant,
            claim_only,
        });
        self.by_key.insert(key, self.entries.len() - 1);
        id
    }

    /// Every name a reference walks through, one entry per dotted prefix.
    pub(crate) fn add_path(&mut self, path: &ListPath) {
        let parts = prefixes(&path.rest);
        for (index, prefix) in parts.iter().enumerate() {
            let last = index + 1 == parts.len();
            if parts.len() == 1 && pseudo_id(prefix).is_some() {
                continue;
            }
            if last && path.marker {
                self.add(prefix, Some(path.twin.clone()), path.twin.clone());
            } else {
                self.add(prefix, None, None);
            }
        }
    }

    /// The id a dotted prefix stores: its marked entry for the terminal of a
    /// `~` path, its plain entry otherwise, a pseudo member's fixed id.
    pub(crate) fn id_of(&self, name: &str, variant: &Variant) -> Option<String> {
        if variant.is_none()
            && let Some(id) = pseudo_id(name)
            && !self.by_key.contains_key(&(name.to_string(), None))
        {
            return Some(id.to_string());
        }
        self.by_key
            .get(&(name.to_string(), variant.clone()))
            .map(|index| self.entries[*index].id.to_string())
    }

    /// Plain entries named by an English standard attribute take the
    /// Russian spelling as their secondary name: a manual query's field
    /// universe holds only that spelling, and without it the export marks the
    /// field `~`.
    ///
    /// Not a claim-only entry whose Russian spelling is an entry of its own:
    /// the form keeps the broken `~Список.Ref` (only marked references walk
    /// into `Ref`) beside the valid `Список.Ссылка` of a manual query that
    /// selects `Спр.Ссылка КАК Ссылка`. Native ibcmd lets each field of the
    /// list's universe be claimed once, by the first entry in map order that
    /// names it by its name or its secondary name, and reads every later
    /// claimant as shadowed -- so a claim-only `Ref` twinned `Ссылка` ahead of
    /// the `Ссылка` entry made native export `~Список.Ссылка` (7 ERP УХ forms
    /// in the first real cycle: `Ссылка`, `ПометкаУдаления`, `Владелец`). The
    /// platform's own maps give such an entry no twin. A `Ref` a plain
    /// reference walks through keeps its twin: a manual query's universe
    /// holds only `Ссылка`, and `Список.Ref` resolves through it.
    pub(crate) fn apply_std_twins(&mut self, twins: &BTreeMap<String, String>) {
        let names = self
            .entries
            .iter()
            .map(|entry| entry.name.clone())
            .collect::<std::collections::BTreeSet<_>>();
        for entry in &mut self.entries {
            if entry.secondary.is_some() || entry.variant.is_some() {
                continue;
            }
            if let Some(twin) = twins.get(&entry.name)
                && !(entry.claim_only && names.contains(twin))
            {
                entry.secondary = Some(twin.clone());
            }
        }
    }

    /// The field-map block of the bag.
    pub(crate) fn pairs(&self, required: &[String]) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            pairs.push((format!("FieldsMapItemId{index}"), format!("{{\"N\",{}}}", entry.id)));
            pairs.push((format!("FieldsMapItemName{index}"), format!("{{\"S\",{}}}", quoted(&entry.name))));
            if let Some(secondary) = entry.secondary.as_deref().filter(|value| !value.is_empty()) {
                pairs.push((
                    format!("FieldsMapItemSecondaryName{index}"),
                    format!("{{\"S\",{}}}", quoted(secondary)),
                ));
            }
            pairs.push((format!("FiledsMapItemId{index}"), format!("{{\"N\",{}}}", entry.id)));
            pairs.push((format!("FiledsMapItemName{index}"), format!("{{\"S\",{}}}", quoted(&entry.name))));
        }
        pairs.push(("FieldsMapSecondaryNamesLoaded".to_string(), "{\"B\",1}".to_string()));
        let mut required = required.to_vec();
        required.sort_by_key(|id| id.parse::<i64>().unwrap_or(i64::MAX));
        for (index, id) in required.iter().enumerate() {
            pairs.push((format!("ReqMapFieldId{index}"), format!("{{\"N\",{id}}}")));
        }
        pairs
    }

    /// The ids a reference stores after its head, one per dotted prefix.
    pub(crate) fn chain_ids(&self, path: &ListPath) -> Option<Vec<String>> {
        let parts = prefixes(&path.rest);
        let mut ids = Vec::with_capacity(parts.len());
        for (index, prefix) in parts.iter().enumerate() {
            let last = index + 1 == parts.len();
            let pseudo_single = parts.len() == 1 && pseudo_id(prefix).is_some();
            let variant = if last && path.marker && !pseudo_single {
                Some(path.twin.clone())
            } else {
                None
            };
            ids.push(self.id_of(prefix, &variant)?);
        }
        Some(ids)
    }

    /// The id a `<UseAlways><Field>` stores in `ReqMapFieldId`.
    pub(crate) fn required_id(&self, path: &ListPath) -> Option<String> {
        if !path.rest.contains('.')
            && let Some(id) = pseudo_id(&path.rest)
        {
            return Some(id.to_string());
        }
        let variant = path.marker.then(|| path.twin.clone());
        self.id_of(&path.rest, &variant)
    }
}

fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

// ----------------------------------------------------------------- XML text

/// One tag of a `Form.xml` fragment.
struct Tag {
    start: usize,
    end: usize,
    closing: bool,
    self_closing: bool,
    name: String,
    attributes: Vec<(String, String)>,
}

fn scan_tags(text: &str) -> Result<Vec<Tag>> {
    let bytes = text.as_bytes();
    let mut tags = Vec::new();
    let mut position = 0;
    while let Some(offset) = text[position..].find('<') {
        let start = position + offset;
        let close = text[start..]
            .find('>')
            .map(|offset| start + offset + 1)
            .ok_or_else(|| anyhow!("an unclosed tag"))?;
        let raw = &text[start..close];
        position = close;
        if raw.starts_with("<?") || raw.starts_with("<!") {
            continue;
        }
        let closing = raw.starts_with("</");
        let self_closing = raw.ends_with("/>");
        let inner = if closing {
            &raw[2..raw.len() - 1]
        } else if self_closing {
            &raw[1..raw.len() - 2]
        } else {
            &raw[1..raw.len() - 1]
        };
        let name_len = inner
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .unwrap_or(inner.len());
        let name = inner[..name_len].to_string();
        let mut attributes = Vec::new();
        let mut rest = &inner[name_len..];
        loop {
            rest = rest.trim_start();
            if rest.is_empty() || rest.starts_with('/') {
                break;
            }
            let eq = rest.find('=').ok_or_else(|| anyhow!("an attribute without a value"))?;
            let key = rest[..eq].trim().to_string();
            let after = rest[eq + 1..].trim_start();
            let after = after
                .strip_prefix('"')
                .ok_or_else(|| anyhow!("an attribute value is not double-quoted"))?;
            let value_end = after.find('"').ok_or_else(|| anyhow!("an unterminated attribute value"))?;
            attributes.push((key, after[..value_end].to_string()));
            rest = &after[value_end + 1..];
        }
        let _ = bytes;
        tags.push(Tag {
            start,
            end: close,
            closing,
            self_closing,
            name,
            attributes,
        });
    }
    Ok(tags)
}

fn format_tag(name: &str, attributes: &[(String, String)], self_closing: bool) -> String {
    let mut out = format!("<{name}");
    for (key, value) in attributes {
        out.push_str(&format!(" {key}=\"{value}\""));
    }
    if self_closing {
        out.push('/');
    }
    out.push('>');
    out
}

/// The block's own indentation and every run of layout whitespace between two
/// tags lose `shift` tabs; character data is untouched.
fn deindent(text: &str, shift: usize) -> Result<String> {
    let lead = "\t".repeat(shift);
    let body = text
        .strip_prefix(&lead)
        .ok_or_else(|| anyhow!("a settings block is not indented as measured"))?;
    let mut out = String::with_capacity(body.len());
    let chars = body.char_indices().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let (_, c) = chars[index];
        out.push(c);
        index += 1;
        if c != '>' {
            continue;
        }
        // `>` then CR? LF, tabs, `<`
        let mut probe = index;
        if probe < chars.len() && chars[probe].1 == '\r' {
            probe += 1;
        }
        if probe < chars.len() && chars[probe].1 == '\n' {
            let newline_end = probe + 1;
            let mut tabs = newline_end;
            while tabs < chars.len() && chars[tabs].1 == '\t' {
                tabs += 1;
            }
            if tabs < chars.len() && chars[tabs].1 == '<' {
                let count = tabs - newline_end;
                if count < shift {
                    return Err(anyhow!("a settings block is not indented as measured"));
                }
                for (_, whitespace) in &chars[index..newline_end] {
                    out.push(*whitespace);
                }
                out.push_str(&"\t".repeat(count - shift));
                index = tabs;
            }
        }
    }
    Ok(out)
}

struct Scope {
    frames: Vec<Vec<(String, String)>>,
}

impl Scope {
    fn new(first: &[(&str, &str)]) -> Self {
        Self {
            frames: vec![
                first
                    .iter()
                    .map(|(prefix, namespace)| (prefix.to_string(), namespace.to_string()))
                    .collect(),
            ],
        }
    }

    fn namespace_of(&self, prefix: &str) -> Option<&str> {
        for frame in self.frames.iter().rev() {
            for (candidate, namespace) in frame.iter().rev() {
                if candidate == prefix {
                    return Some(namespace.as_str());
                }
            }
        }
        None
    }

    fn prefix_of(&self, namespace: &str) -> Option<String> {
        for frame in self.frames.iter().rev() {
            for (prefix, candidate) in frame.iter().rev() {
                if candidate == namespace && self.namespace_of(prefix) == Some(namespace) {
                    return Some(prefix.clone());
                }
            }
        }
        None
    }

    fn expand(&self, qname: &str) -> Result<(String, String)> {
        let (prefix, local) = match qname.rsplit_once(':') {
            Some((prefix, local)) => (prefix, local),
            None => ("", qname),
        };
        let namespace = self
            .namespace_of(prefix)
            .ok_or_else(|| anyhow!("an undeclared prefix {prefix}"))?;
        Ok((namespace.to_string(), local.to_string()))
    }
}

/// What the re-spelling needs from the configuration.
pub(crate) struct ListConfiguration<'a> {
    /// The `<xr:TypeId>` of a generated type (`CatalogRef.X`), or the id of a
    /// type set when the flag says it is one.
    pub(crate) type_id: &'a dyn Fn(&str, bool) -> Result<String>,
    /// The uuid of a style item the configuration declares.
    pub(crate) style_item: &'a dyn Fn(&str) -> Option<String>,
    /// The uuid of a metadata object, `Kind.Name`.
    pub(crate) object: &'a dyn Fn(&str) -> Result<String>,
}

/// Platform type-set ids, for a `<v8:TypeSet>cfg:<X>` naming a whole family.
fn platform_type_set(name: &str) -> Option<&'static str> {
    Some(match name {
        "ExchangePlanRef" => "0a52f9de-73ea-4507-81e8-66217bead73a",
        "BusinessProcessRoutePointRef" => "11e5f865-1501-40c6-b4d4-022095a296a5",
        "BusinessProcessRef" => "214fa4d8-6ba4-4748-a5e1-6332b5887780",
        "DocumentRef" => "38bfd075-3e63-4aaa-a93e-94521380d579",
        "EnumRef" => "474c3bf6-08b5-4ddc-a2ad-989cedf11583",
        "ChartOfCalculationTypesRef" => "593cd424-0877-470d-91f9-b90a982059b4",
        "TaskRef" => "6291e9b3-8df5-44e1-b6b2-d9fe008016c0",
        "ChartOfCharacteristicTypesRef" => "99892482-ed55-4fb5-a7f7-20888820a758",
        "ChartOfAccountsRef" => "ac606d60-0209-4159-8e4c-794bc091ce38",
        "CatalogRef" => "e61ef7b8-f3e1-4f4b-8ac7-676e90524997",
        "AnyIBRef" => "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63",
        _ => return None,
    })
}

/// One `Form.xml` element re-spelled the way the platform's storage writer
/// keeps it (`findings/rt-dynamic-list.md` §5.1):
///
/// * the block element is unqualified and, in a `ServerState`, declares
///   `xmlns:dcssch` first; `<CalculatedField>` is written
///   `<ExpressionField xsi:type="dcssch:CalculatedField">` and a
///   `<Parameter>` gets `xsi:type="dcssch:Parameter"`;
/// * an element whose namespace has a prefix in scope uses it; otherwise it
///   declares that namespace as its default and its descendants in it are
///   unprefixed;
/// * a QName -- an `xsi:type`, the body of a core `Type`/`TypeSet` or of an
///   element typed `v8:Type` or `v8ui:Color` -- whose namespace has no prefix
///   in scope declares one on its element: the fixed ones by name, any other
///   as `d<depth>p<n>`;
/// * `cfg:` types become `<TypeId>` uuids, builtins first in a run;
/// * a style item the configuration declares is written `0:<uuid>`.
fn convert_block(
    text: &str,
    configuration: &ListConfiguration<'_>,
    root_default: &str,
    shift: usize,
    server_state: bool,
) -> Result<String> {
    let base_depth = 2usize;
    let text = deindent(text, shift)?;
    let tags = scan_tags(&text)?;
    let mut scope_in = Scope::new(FORM_ROOT_NS);
    let mut scope_out = Scope::new(&[("", root_default), ("xs", XS), ("xsi", XSI)]);
    let mut out = String::with_capacity(text.len() + 256);
    let mut names = Vec::<String>::new();
    let mut position = 0usize;
    let mut depth = base_depth - 1;
    let mut replace_next_text: Option<String> = None;
    for (tag_index, tag) in tags.iter().enumerate() {
        let between = &text[position..tag.start];
        match replace_next_text.take() {
            Some(replacement) => out.push_str(&replacement),
            None => out.push_str(between),
        }
        position = tag.end;
        if tag.closing {
            scope_in.frames.pop();
            scope_out.frames.pop();
            depth -= 1;
            let name = names.pop().ok_or_else(|| anyhow!("an unbalanced settings block"))?;
            out.push_str(&format!("</{name}>"));
            continue;
        }
        depth += 1;
        let top = depth == base_depth && server_state;
        let mut in_decls = Vec::new();
        let mut attributes = Vec::new();
        for (key, value) in &tag.attributes {
            if key == "xmlns" {
                in_decls.push((String::new(), value.clone()));
            } else if let Some(prefix) = key.strip_prefix("xmlns:") {
                in_decls.push((prefix.to_string(), value.clone()));
            } else {
                attributes.push((key.clone(), value.clone()));
            }
        }
        scope_in.frames.push(in_decls);
        let (element_namespace, mut element_local) = if top {
            if tag.name.contains(':') {
                return Err(anyhow!("a prefixed settings block element"));
            }
            (String::new(), tag.name.clone())
        } else {
            scope_in.expand(&tag.name)?
        };
        if top {
            if tag.name == "CalculatedField" {
                element_local = "ExpressionField".to_string();
                attributes.retain(|(key, _)| key != "xsi:type");
                attributes.insert(0, ("xsi:type".to_string(), "dcssch:CalculatedField".to_string()));
            } else if tag.name == "Parameter" && !attributes.iter().any(|(key, _)| key == "xsi:type") {
                attributes.insert(0, ("xsi:type".to_string(), "dcssch:Parameter".to_string()));
            }
        }
        scope_out.frames.push(Vec::new());
        let mut declarations = Vec::<(String, String)>::new();
        let mut generated = 0usize;
        let mut declare = |scope_out: &mut Scope, declarations: &mut Vec<(String, String)>, namespace: &str| {
            let prefix = match fixed_prefix(namespace) {
                Some(prefix) => prefix.to_string(),
                None => {
                    generated += 1;
                    format!("d{depth}p{generated}")
                }
            };
            declarations.push((prefix.clone(), namespace.to_string()));
            if let Some(frame) = scope_out.frames.last_mut() {
                frame.push((prefix.clone(), namespace.to_string()));
            }
            prefix
        };
        let mut default_declaration = None;
        let current_default = scope_out.namespace_of("").unwrap_or("").to_string();
        let out_name = if element_namespace == current_default {
            element_local.clone()
        } else {
            let prefix = if element_namespace.is_empty() {
                None
            } else {
                scope_out.prefix_of(&element_namespace)
            };
            match prefix {
                Some(prefix) if !prefix.is_empty() => format!("{prefix}:{element_local}"),
                _ if !element_namespace.is_empty() => {
                    default_declaration = Some(element_namespace.clone());
                    if let Some(frame) = scope_out.frames.last_mut() {
                        frame.push((String::new(), element_namespace.clone()));
                    }
                    element_local.clone()
                }
                _ => return Err(anyhow!("an unqualified element under a default namespace")),
            }
        };
        if top {
            declare(&mut scope_out, &mut declarations, SCHEMA);
        }
        let mut out_attributes = Vec::new();
        for (key, value) in &attributes {
            if key == "xsi:type" {
                let (namespace, local) = scope_in.expand(value)?;
                let prefix = match scope_out.prefix_of(&namespace) {
                    Some(prefix) => prefix,
                    None => declare(&mut scope_out, &mut declarations, &namespace),
                };
                out_attributes.push((
                    key.clone(),
                    if prefix.is_empty() { local } else { format!("{prefix}:{local}") },
                ));
            } else if key.contains(':') && !key.starts_with("xsi:") {
                return Err(anyhow!("an attribute {key} is not measured"));
            } else if key == "ref" && value.contains(':') {
                let (namespace, local) = scope_in.expand(value)?;
                let fixed = match namespace.as_str() {
                    STYLE => "style",
                    SYS_FONTS => "sys",
                    _ => return Err(anyhow!("a font reference {value} is not measured")),
                };
                let configured = (namespace == STYLE)
                    .then(|| (configuration.style_item)(&local))
                    .flatten();
                let prefix = match scope_out.prefix_of(&namespace) {
                    Some(prefix) => prefix,
                    None => {
                        declarations.push((fixed.to_string(), namespace.clone()));
                        if let Some(frame) = scope_out.frames.last_mut() {
                            frame.push((fixed.to_string(), namespace.clone()));
                        }
                        fixed.to_string()
                    }
                };
                out_attributes.push((
                    key.clone(),
                    match configured {
                        Some(uuid) => format!("0:{uuid}"),
                        None => format!("{prefix}:{local}"),
                    },
                ));
            } else {
                out_attributes.push((key.clone(), value.clone()));
            }
        }
        let typed = |namespace: &str, local: &str| {
            attributes.iter().any(|(key, value)| {
                key == "xsi:type"
                    && scope_in
                        .expand(value)
                        .is_ok_and(|(ns, name)| ns == namespace && name == local)
            })
        };
        let next_text = if !tag.self_closing && tag_index + 1 < tags.len() {
            Some(&text[tag.end..tags[tag_index + 1].start])
        } else {
            None
        };
        // A colour naming a style item is stored by that item's uuid; a
        // platform colour stays a QName.
        if typed(UI, "Color")
            && let Some(raw) = next_text
        {
            let value = raw.trim();
            let configured = value
                .strip_prefix("style:")
                .and_then(|name| (configuration.style_item)(name));
            if let Some(uuid) = configured {
                replace_next_text = Some(raw.replace(value, &format!("0:{uuid}")));
            } else if let Some((prefix, _)) = value.split_once(':')
                && !prefix.is_empty()
                && scope_in.namespace_of(prefix).is_some()
            {
                let (namespace, local) = scope_in.expand(value)?;
                let prefix = match scope_out.prefix_of(&namespace) {
                    Some(prefix) => prefix,
                    None => declare(&mut scope_out, &mut declarations, &namespace),
                };
                let rendered = if prefix.is_empty() { local } else { format!("{prefix}:{local}") };
                replace_next_text = Some(raw.replace(value, &rendered));
            }
        }
        let type_body = (element_namespace == CORE
            && matches!(element_local.as_str(), "Type" | "TypeSet"))
            || typed(CORE, "Type");
        if type_body
            && let Some(raw) = next_text
        {
            let value = raw.trim();
            if !value.is_empty() {
                let (namespace, local) = scope_in.expand(value)?;
                let rendered = if namespace == CFG {
                    format!("cfg:{local}")
                } else {
                    let prefix = match scope_out.prefix_of(&namespace) {
                        Some(prefix) => prefix,
                        None => declare(&mut scope_out, &mut declarations, &namespace),
                    };
                    if prefix.is_empty() { local } else { format!("{prefix}:{local}") }
                };
                replace_next_text = Some(raw.replace(value, &rendered));
            }
        }
        let mut all_attributes = Vec::new();
        if let Some(namespace) = default_declaration {
            all_attributes.push(("xmlns".to_string(), namespace));
        }
        for (prefix, namespace) in &declarations {
            all_attributes.push((format!("xmlns:{prefix}"), namespace.clone()));
        }
        all_attributes.extend(out_attributes);
        if tag.self_closing {
            out.push_str(&format_tag(&out_name, &all_attributes, true));
            scope_in.frames.pop();
            scope_out.frames.pop();
            depth -= 1;
        } else {
            out.push_str(&format_tag(&out_name, &all_attributes, false));
            names.push(out_name);
        }
    }
    out.push_str(&text[position..]);
    if !names.is_empty() {
        return Err(anyhow!("an unbalanced settings block"));
    }
    retype(&out, configuration)
}

/// `cfg:` types become `<TypeId>` uuids; within a run of type siblings the
/// builtin `<Type>`s come first and the `TypeId`s follow, sorted.
fn retype(document: &str, configuration: &ListConfiguration<'_>) -> Result<String> {
    let type_open = format!("<Type xmlns=\"{CORE}\">");
    let set_open = format!("<TypeSet xmlns=\"{CORE}\">");
    let mut out = String::with_capacity(document.len());
    let mut position = 0usize;
    loop {
        let next = [type_open.as_str(), set_open.as_str()]
            .iter()
            .filter_map(|open| document[position..].find(open).map(|at| position + at))
            .min();
        let Some(start) = next else {
            out.push_str(&document[position..]);
            break;
        };
        out.push_str(&document[position..start]);
        // Collect the run of consecutive type siblings.
        let mut members = Vec::<(bool, String, String)>::new(); // (is set, value, separator)
        let mut cursor = start;
        loop {
            let (is_set, open) = if document[cursor..].starts_with(&type_open) {
                (false, &type_open)
            } else if document[cursor..].starts_with(&set_open) {
                (true, &set_open)
            } else {
                break;
            };
            let close = if is_set { "</TypeSet>" } else { "</Type>" };
            let value_start = cursor + open.len();
            let value_end = document[value_start..]
                .find(close)
                .map(|at| value_start + at)
                .ok_or_else(|| anyhow!("an unclosed type"))?;
            let value = document[value_start..value_end].to_string();
            let after = value_end + close.len();
            let separator_len = document[after..]
                .find(|c: char| !c.is_whitespace())
                .unwrap_or(document.len() - after);
            let separator = document[after..after + separator_len].to_string();
            members.push((is_set, value, separator));
            cursor = after + separator_len;
        }
        let separators = members.iter().map(|(_, _, separator)| separator.clone()).collect::<Vec<_>>();
        let mut builtin = Vec::new();
        let mut references = Vec::<(String, String)>::new();
        for (is_set, value, _) in &members {
            let value = value.trim();
            if let Some(name) = value.strip_prefix("cfg:") {
                let id = if *is_set {
                    match platform_type_set(name) {
                        Some(id) => id.to_string(),
                        None => (configuration.type_id)(name, true)?,
                    }
                } else {
                    (configuration.type_id)(name, false)?
                };
                references.push((id.clone(), format!("<TypeId xmlns=\"{CORE}\">{id}</TypeId>")));
            } else if *is_set {
                return Err(anyhow!("a builtin type set {value} is not measured"));
            } else {
                builtin.push(format!("<Type xmlns=\"{CORE}\">{value}</Type>"));
            }
        }
        references.sort_by(|left, right| left.0.cmp(&right.0));
        let ordered = builtin
            .into_iter()
            .chain(references.into_iter().map(|(_, text)| text))
            .collect::<Vec<_>>();
        for (text, separator) in ordered.iter().zip(separators.iter()) {
            out.push_str(text);
            out.push_str(separator);
        }
        position = cursor;
    }
    Ok(out)
}

// ----------------------------------------------------------------- the bag

/// The `<Settings xsi:type="DynamicList">` block of an attribute, inner text.
fn settings_inner<'a>(form: &'a str, attribute: &str) -> Result<&'a str> {
    let marker = format!("<Attribute name=\"{attribute}\"");
    let at = form
        .find(&marker)
        .ok_or_else(|| anyhow!("the dynamic list {attribute} is not in Form.xml"))?;
    let open = "<Settings xsi:type=\"DynamicList\">";
    let start = form[at..]
        .find(open)
        .map(|offset| at + offset + open.len())
        .ok_or_else(|| anyhow!("the dynamic list {attribute} has no settings"))?;
    let end = form[start..]
        .find("</Settings>")
        .map(|offset| start + offset)
        .ok_or_else(|| anyhow!("the dynamic list {attribute}'s settings do not close"))?;
    Ok(&form[start..end])
}

/// The depth-0 elements of a fragment: name and raw text, with the leading
/// indentation of their line.
fn direct_children(inner: &str) -> Result<Vec<(String, String)>> {
    let tags = scan_tags(inner)?;
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut current: Option<(String, usize)> = None;
    for tag in &tags {
        if tag.closing {
            depth = depth.saturating_sub(1);
            if depth == 0
                && let Some((name, start)) = current.take()
            {
                let line = inner[..start].rfind('\n').map_or(0, |at| at + 1);
                out.push((name, inner[line..tag.end].to_string()));
            }
            continue;
        }
        if depth == 0 {
            if tag.self_closing {
                let line = inner[..tag.start].rfind('\n').map_or(0, |at| at + 1);
                out.push((tag.name.clone(), inner[line..tag.end].to_string()));
                continue;
            }
            current = Some((tag.name.clone(), tag.start));
        }
        if !tag.self_closing {
            depth += 1;
        }
    }
    Ok(out)
}

fn unescape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let Some(end) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        let entity = &tail[1..end];
        let resolved = match entity {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match resolved {
            Some(c) => out.push(c),
            None => out.push_str(&tail[..=end]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The character data of a simple element.
fn text_of(raw: &str) -> String {
    let trimmed = raw.trim();
    let Some(open_end) = trimmed.find('>') else {
        return String::new();
    };
    if trimmed[..open_end].ends_with('/') {
        return String::new();
    }
    let Some(close) = trimmed.rfind("</") else {
        return String::new();
    };
    unescape_text(&trimmed[open_end + 1..close])
}

fn inner_of(raw: &str) -> Option<&str> {
    let open_start = raw.find('<')?;
    let open_end = raw[open_start..].find('>')? + open_start;
    if raw[..open_end].ends_with('/') {
        return None;
    }
    let close = raw.rfind("</")?;
    Some(&raw[open_end + 1..close])
}

fn base64_mime(bytes: &[u8]) -> String {
    let encoded = crate::module_blob::encode_base64(bytes);
    encoded
        .as_bytes()
        .chunks(64)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn document_field(kind: &str, document: &str) -> String {
    let mut payload = vec![0xEF, 0xBB, 0xBF];
    payload.extend_from_slice(document.as_bytes());
    format!("{{\"#\",{kind},{{#base64:{}}}}}", base64_mime(&payload))
}

const DOCUMENT_HEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n";

/// A DCS settings storage document: the `Form.xml` children re-spelled under
/// a root whose default namespace is the settings namespace.
fn dcs_document(
    root: &str,
    blocks: &[(String, usize)],
    configuration: &ListConfiguration<'_>,
) -> Result<String> {
    let open = format!("<{root} xmlns=\"{DCSSET}\" xmlns:xs=\"{XS}\" xmlns:xsi=\"{XSI}\"");
    if blocks.is_empty() {
        return Ok(format!("{DOCUMENT_HEAD}{open}/>"));
    }
    let mut body = String::new();
    for (raw, shift) in blocks {
        body.push_str(&convert_block(raw, configuration, DCSSET, *shift, false)?);
        body.push_str("\r\n");
    }
    Ok(format!("{DOCUMENT_HEAD}{open}>\r\n{body}</{root}>"))
}

fn children_blocks(inner: Option<&str>, shift: usize) -> Result<Vec<(String, usize)>> {
    let Some(inner) = inner else {
        return Ok(Vec::new());
    };
    Ok(direct_children(inner)?
        .into_iter()
        .map(|(_, raw)| (raw, shift))
        .collect())
}

/// `ServerState`: the `<Settings>` children `Field`, `Parameter` and
/// `CalculatedField` as a `<UniversalListServerOnlyState>` document, framed.
fn server_state(children: &[(String, String)], configuration: &ListConfiguration<'_>) -> Result<String> {
    let blocks = children
        .iter()
        .filter(|(name, _)| matches!(name.as_str(), "Field" | "Parameter" | "CalculatedField"))
        .collect::<Vec<_>>();
    let open = format!("<UniversalListServerOnlyState xmlns=\"\" xmlns:xs=\"{XS}\" xmlns:xsi=\"{XSI}\"");
    let document = if blocks.is_empty() {
        format!("{DOCUMENT_HEAD}{open}/>")
    } else {
        let mut body = String::new();
        for (_, raw) in blocks {
            body.push_str(&convert_block(raw, configuration, "", 3, true)?);
            body.push_str("\r\n");
        }
        format!("{DOCUMENT_HEAD}{open}>\r\n{body}</UniversalListServerOnlyState>")
    };
    let mut payload = vec![0xEF, 0xBB, 0xBF];
    payload.extend_from_slice(document.as_bytes());
    // The platform's chunking: a first run of two bytes, then runs of up to
    // 32 768; any chunking decodes the same.
    let mut raw = vec![0x41, 0xC1];
    let mut sizes = vec![payload.len().min(2)];
    let mut position = sizes[0];
    while position < payload.len() {
        let size = (payload.len() - position).min(32_768);
        sizes.push(size);
        position += size;
    }
    let mut position = 0usize;
    for size in sizes {
        if size < 256 {
            raw.push(0x9A);
            raw.push(size as u8);
        } else {
            raw.push(0x9B);
            raw.extend_from_slice(&(size as u16).to_le_bytes());
        }
        raw.extend_from_slice(&payload[position..position + size]);
        position += size;
    }
    raw.extend_from_slice(b"  ");
    Ok(format!("{{\"S\",\"{}\"}}", base64_mime(&raw)))
}

const MAIN_TABLE_REF: &str = "fc01b5df-97fe-449b-83d4-218a090e681e";
const KEY_FIELDS_ARRAY: &str = "51e7a0d2-530b-11d4-b98a-008048da3034";
const LOCALIZED_STRING: &str = "87024738-fc2a-4436-ada1-df79d395c424";
const KIND_FILTER: &str = "f6841c6b-6c71-4c82-ae9e-d08b49db326c";
const KIND_ORDER: &str = "11743ff3-2db3-4cfc-9404-90ed8209437f";
const KIND_APPEARANCE: &str = "93de27ad-a2d8-4b10-a82b-483c9b0648fe";
const KIND_GROUP: &str = "e2e2f2e9-e309-4212-9c70-3ab32dd93b4d";
const KIND_DATA_PARAMETERS: &str = "6217eee1-6289-49d2-9315-d87888cd2d62";
const DEFAULT_FILTER_ID: &str = "dfcece9d-5077-440b-b6b3-45a5cb4538eb";
const DEFAULT_ORDER_ID: &str = "88619765-ccb3-46c6-ac52-38e9c992ebd4";
const DEFAULT_APPEARANCE_ID: &str = "b75fecce-942b-4aed-abc9-e6a02e460fb3";
const ITEMS_DEFAULT_ID: &str = "911b6018-f537-43e8-a417-da56b22f9aec";
const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";

fn boolean(value: bool) -> String {
    format!("{{\"B\",{}}}", u8::from(value))
}

/// A `ListSettings` part that is only `viewMode Normal` and its default
/// `userSettingID` is not written at all.
fn part_is_default(raw: &str, default_id: &str) -> Result<bool> {
    let Some(inner) = inner_of(raw) else {
        return Ok(false);
    };
    let children = direct_children(inner)?
        .into_iter()
        .map(|(name, raw)| (name, text_of(&raw)))
        .collect::<Vec<_>>();
    Ok(children
        == vec![
            ("dcsset:viewMode".to_string(), "Normal".to_string()),
            ("dcsset:userSettingID".to_string(), default_id.to_string()),
        ])
}

fn localized_pairs(raw: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = raw;
    while let Some(at) = rest.find("<v8:lang>") {
        let after = &rest[at + "<v8:lang>".len()..];
        let Some(lang_end) = after.find("</v8:lang>") else {
            break;
        };
        let lang = unescape_text(&after[..lang_end]);
        let after = &after[lang_end..];
        let Some(content_start) = after.find("<v8:content>") else {
            break;
        };
        let after = &after[content_start + "<v8:content>".len()..];
        let Some(content_end) = after.find("</v8:content>") else {
            break;
        };
        out.push((lang, unescape_text(&after[..content_end])));
        rest = &after[content_end..];
    }
    out
}

/// The whole bag of one dynamic list, `{0,<pairs>,"Key",<value>,…}`.
pub(crate) fn dynamic_list_bag(
    form: &str,
    attribute: &str,
    map: &FieldMap,
    required: &[String],
    configuration: &ListConfiguration<'_>,
) -> Result<String> {
    let inner = settings_inner(form, attribute)?;
    let children = direct_children(inner)?;
    let first = |name: &str| children.iter().find(|(child, _)| child == name).map(|(_, raw)| raw.as_str());
    let scalar = |name: &str| first(name).map(text_of);

    let mut pairs = Vec::<(String, String)>::new();
    let query = scalar("QueryText").unwrap_or_default();
    let query = query.replace("\r\n", "\n").replace('\n', "\r\n");
    pairs.push(("QueryText".into(), format!("{{\"S\",{}}}", quoted(&query))));

    let (main_table, category) = match scalar("MainTable") {
        Some(table) if !table.trim().is_empty() => {
            let parts = table.trim().split('.').collect::<Vec<_>>();
            if parts.len() < 2 {
                return Err(anyhow!("the main table {table} is not measured"));
            }
            let uuid = (configuration.object)(&format!("{}.{}", parts[0], parts[1]))?;
            let category = match (parts.len(), parts[0], parts.get(2).copied()) {
                (2, _, _) => 1,
                (3, "Task", Some("TasksByExecutive")) => 2,
                (3, "AccountingRegister", Some("RecordsWithExtDimensions")) => 3,
                (3, "AccountingRegister", Some("Balance")) => 4,
                (3, "AccumulationRegister", Some("Balance")) => 3,
                (3, "AccumulationRegister", Some("Turnovers")) => 4,
                (3, "InformationRegister", Some("SliceLast")) => 3,
                _ => return Err(anyhow!("the virtual table {table} is not measured")),
            };
            (uuid, category)
        }
        _ => (ZERO_UUID.to_string(), 0),
    };
    pairs.push(("MainTable".into(), format!("{{\"#\",{MAIN_TABLE_REF},{main_table}}}")));
    pairs.push(("MainTableCategory".into(), format!("{{\"N\",{category}}}")));
    let key_type = match scalar("KeyType").as_deref() {
        None => 0,
        Some("FieldValue") => 1,
        Some("RowKey") => 2,
        Some(other) => return Err(anyhow!("a dynamic list's <KeyType>{other} is not measured")),
    };
    pairs.push(("KeyType".into(), format!("{{\"N\",{key_type}}}")));
    let key_fields = children
        .iter()
        .filter(|(name, _)| name == "KeyField")
        .map(|(_, raw)| format!("{{\"S\",{}}}", quoted(&text_of(raw))))
        .collect::<Vec<_>>();
    let key_fields = if key_fields.is_empty() {
        "{0}".to_string()
    } else {
        format!("{{{},{}}}", key_fields.len(), key_fields.join(","))
    };
    pairs.push(("KeyFields".into(), format!("{{\"#\",{KEY_FIELDS_ARRAY},{key_fields}}}")));
    let dynamic_data_read = scalar("DynamicDataRead")
        .ok_or_else(|| anyhow!("a dynamic list names no <DynamicDataRead>"))?;
    pairs.push(("DynamicalDataSelection".into(), boolean(dynamic_data_read != "true")));
    pairs.push((
        "AutoFillAvailableFields".into(),
        boolean(scalar("AutoFillAvailableFields").as_deref() != Some("false")),
    ));
    let manual_query = scalar("ManualQuery").ok_or_else(|| anyhow!("a dynamic list names no <ManualQuery>"))?;
    pairs.push(("ManualQuery".into(), boolean(manual_query == "true")));
    pairs.extend(map.pairs(required));
    pairs.push((
        "AutoSaveUserSettings".into(),
        boolean(scalar("AutoSaveUserSettings").as_deref() != Some("false")),
    ));

    // The list settings' parts.
    let list_settings = first("ListSettings").and_then(inner_of);
    let parts = match list_settings {
        Some(inner) => direct_children(inner)?,
        None => Vec::new(),
    };
    let part = |name: &str| parts.iter().find(|(part, _)| part == name).map(|(_, raw)| raw.as_str());
    for (key, name, root, kind, default_id) in [
        ("Filter", "dcsset:filter", "Filter", KIND_FILTER, DEFAULT_FILTER_ID),
        ("Order", "dcsset:order", "Order", KIND_ORDER, DEFAULT_ORDER_ID),
    ] {
        let raw = part(name);
        if let Some(raw) = raw
            && part_is_default(raw, default_id)?
        {
            continue;
        }
        let blocks = children_blocks(raw.and_then(inner_of), 5)?;
        pairs.push((key.into(), document_field(kind, &dcs_document(root, &blocks, configuration)?)));
    }
    // The group block.
    let items = parts
        .iter()
        .filter(|(name, _)| name == "dcsset:item")
        .map(|(_, raw)| raw.as_str())
        .collect::<Vec<_>>();
    let items_view_mode = part("dcsset:itemsViewMode").map(text_of);
    let items_id = part("dcsset:itemsUserSettingID").map(text_of);
    let items_presentation = part("dcsset:itemsUserSettingPresentation");
    if !items.is_empty()
        || items_view_mode.as_deref() != Some("Normal")
        || items_id.as_deref() != Some(ITEMS_DEFAULT_ID)
        || items_presentation.is_some()
    {
        pairs.push((
            "GroupSelectedSettingId".into(),
            format!("{{\"S\",{}}}", quoted(items_id.as_deref().unwrap_or(""))),
        ));
        pairs.push((
            "GroupSelectedSettingViewMode".into(),
            format!("{{\"N\",{}}}", if items_view_mode.as_deref() == Some("Normal") { 0 } else { 1 }),
        ));
        if items.len() > 1 {
            return Err(anyhow!("a dynamic list with more than one structure item is not measured"));
        }
        let mut blocks = Vec::new();
        let mut level = 0usize;
        let mut item = items.first().map(|raw| raw.to_string());
        while let Some(raw) = item {
            let first_line = raw.lines().next().unwrap_or("");
            if !first_line.contains("xsi:type=\"dcsset:StructureItemGroup\"") {
                return Err(anyhow!("a dynamic list's structure item kind is not measured"));
            }
            let kids = direct_children(inner_of(&raw).unwrap_or(""))?;
            let groups = kids.iter().filter(|(name, _)| name == "dcsset:groupItems").count();
            let nested = kids.iter().filter(|(name, _)| name == "dcsset:item").count();
            if kids
                .iter()
                .any(|(name, _)| !matches!(name.as_str(), "dcsset:groupItems" | "dcsset:item"))
                || groups > 1
                || nested > 1
            {
                return Err(anyhow!("a dynamic list's structure item group is not measured"));
            }
            let mut next = None;
            for (name, raw) in &kids {
                if name == "dcsset:groupItems" {
                    if let Some(inner) = inner_of(raw) {
                        for (_, child) in direct_children(inner)? {
                            blocks.push((child, 6 + level));
                        }
                    }
                } else {
                    next = Some(raw.clone());
                }
            }
            item = next;
            level += 1;
        }
        pairs.push((
            "Group".into(),
            document_field(KIND_GROUP, &dcs_document("GroupItems", &blocks, configuration)?),
        ));
        let presentation = match items_presentation {
            Some(raw) => {
                let values = localized_pairs(raw);
                format!(
                    "{{{},{}}}",
                    values.len(),
                    values
                        .iter()
                        .map(|(lang, content)| format!("{},{}", quoted(lang), quoted(content)))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
            None => "{0}".to_string(),
        };
        pairs.push((
            "GroupSelectedSettingPresentation".into(),
            format!("{{\"#\",{LOCALIZED_STRING},{presentation}}}"),
        ));
    }
    let appearance = part("dcsset:conditionalAppearance");
    let default_appearance = match appearance {
        Some(raw) => part_is_default(raw, DEFAULT_APPEARANCE_ID)?,
        None => false,
    };
    if !default_appearance {
        let blocks = children_blocks(appearance.and_then(inner_of), 5)?;
        pairs.push((
            "Appearance".into(),
            document_field(KIND_APPEARANCE, &dcs_document("ConditionalAppearance", &blocks, configuration)?),
        ));
    }
    pairs.push((
        "GetInvisibleFieldPresentations".into(),
        boolean(scalar("GetInvisibleFieldPresentations").as_deref() != Some("false")),
    ));
    pairs.push(("ServerState".into(), server_state(&children, configuration)?));
    let parameters = children_blocks(part("dcsset:dataParameters").and_then(inner_of), 5)?;
    pairs.push((
        "DataParameters".into(),
        document_field(
            KIND_DATA_PARAMETERS,
            &dcs_document("DataParameterValues", &parameters, configuration)?,
        ),
    ));

    let mut out = format!("{{0,{}", pairs.len());
    for (key, value) in pairs {
        out.push(',');
        out.push_str(&quoted(&key));
        out.push(',');
        out.push_str(&value);
    }
    out.push('}');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list_path(text: &str) -> ListPath {
        parse_list_path(text, "Список", &[]).expect("a list path")
    }

    fn twins() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("Ref".to_string(), "Ссылка".to_string()),
            ("Code".to_string(), "Код".to_string()),
            ("DeletionMark".to_string(), "ПометкаУдаления".to_string()),
        ])
    }

    #[test]
    fn a_plain_standard_attribute_takes_its_russian_twin() {
        let mut map = FieldMap::default();
        map.add_path(&list_path("Список.Ref"));
        map.add_path(&list_path("Список.Code"));
        map.apply_std_twins(&twins());
        let secondary = map
            .entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.secondary.as_deref()))
            .collect::<Vec<_>>();
        assert_eq!(secondary, vec![("Ref", Some("Ссылка")), ("Code", Some("Код"))]);
    }

    fn twin_of<'a>(map: &'a FieldMap, name: &str) -> Option<&'a str> {
        map.entries
            .iter()
            .find(|entry| entry.name == name && entry.variant.is_none())
            .and_then(|entry| entry.secondary.as_deref())
    }

    #[test]
    fn a_claim_only_entry_takes_no_twin_that_is_an_entry_of_its_own() {
        // Documents/Лот/Forms/ЛотыДоступныеПоставщику: a manual query selects
        // `Лоты.Ссылка КАК Ссылка`; <UseAlways> keeps `~Список.Ref` and
        // `Список.Ссылка`. Native ibcmd exported `~Список.Ссылка` while the
        // claim-only `Ref` entry carried the secondary name `Ссылка`.
        let mut map = FieldMap::default();
        map.add_path(&list_path("~Список.Ref"));
        map.add_path(&list_path("Список.Ссылка"));
        map.add_path(&list_path("~Список.DeletionMark"));
        map.add_path(&list_path("Список.ПометкаУдаления"));
        map.add_path(&list_path("Список.Code"));
        map.apply_std_twins(&twins());

        assert_eq!(twin_of(&map, "Ref"), None);
        assert_eq!(twin_of(&map, "DeletionMark"), None);
        assert_eq!(twin_of(&map, "Code"), Some("Код"));
        let pairs = map.pairs(&[]);
        let secondary = pairs
            .iter()
            .filter(|(key, _)| key.starts_with("FieldsMapItemSecondaryName"))
            .map(|(_, value)| value.as_str())
            .collect::<Vec<_>>();
        assert_eq!(secondary, vec!["{\"S\",\"Код\"}"]);
        // The references still resolve to their own entries.
        assert_eq!(map.required_id(&list_path("Список.Ссылка")).as_deref(), Some("3"));
        assert_eq!(map.required_id(&list_path("~Список.Ref")).as_deref(), Some("2"));
    }

    #[test]
    fn a_claim_only_entry_keeps_its_twin_when_that_spelling_is_no_entry() {
        let mut map = FieldMap::default();
        map.add_path(&list_path("~Список.Ref"));
        map.apply_std_twins(&twins());
        assert_eq!(twin_of(&map, "Ref"), Some("Ссылка"));
    }

    #[test]
    fn a_plainly_referenced_entry_keeps_its_twin_beside_an_entry_of_that_spelling() {
        // Catalogs/Должности/Forms/ФормаСписка: `Список.Ref` resolves through
        // `Ссылка` in the manual query's universe; the conditional appearance
        // adds a `Ссылка` entry after it. Native reads this map as the source.
        let mut map = FieldMap::default();
        map.add_path(&list_path("Список.Ref"));
        map.add_path(&list_path("Список.Ссылка"));
        map.apply_std_twins(&twins());
        assert_eq!(twin_of(&map, "Ref"), Some("Ссылка"));

        // A marked reference first, a plain one after it: the entry is real.
        let mut map = FieldMap::default();
        map.add_path(&list_path("~Список.Ref"));
        map.add_path(&list_path("Список.Ссылка"));
        map.add_path(&list_path("Список.Ref"));
        map.apply_std_twins(&twins());
        assert_eq!(twin_of(&map, "Ref"), Some("Ссылка"));
    }
}
