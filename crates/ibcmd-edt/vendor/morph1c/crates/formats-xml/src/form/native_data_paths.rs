//! One closed CURRENT configuration annotation in the existing native manifest.
//! It transports semantic paths, never XML source bytes or stale manifest versions.
use super::{data_path_semantics as paths, FormDialect, FormError, FormProjectionContext};
use morph1c_core::{
    ir::{Configuration, MetadataObject, Uuid},
    version::FormatVersion,
};
use quick_xml::{events::Event, name::ResolveResult, NsReader};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
const NS: &str = "http://v8.1c.ru/8.3/xcf/dumpinfo";
const FAMILY: &str = "ibcmd-configuration-semantics:";
const PREFIX: &str = "ibcmd-configuration-semantics:1:";
const SCHEMA: &str = "urn:ibcmd:source-extension:configuration-semantics:1";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema: String,
    version: u32,
    configuration_uuid: Uuid,
    data_paths: Vec<paths::Resource>,
}
/// Parsed typed annotation; all global declarations are validated at completion.
#[derive(Debug, Clone)]
pub struct NativeDataPathAnnotation {
    envelope: Envelope,
}
fn error(s: impl Into<String>) -> FormError {
    FormError::Frame(format!("configuration DataPath annotation: {}", s.into()))
}
fn configuration_uuid(cfg: &Configuration) -> Result<Uuid, FormError> {
    let mut roots = cfg
        .objects
        .iter()
        .filter(|o| o.kind.as_str() == "Configuration");
    let root = roots
        .next()
        .ok_or_else(|| error("missing declared Configuration"))?;
    if roots.next().is_some() {
        return Err(error("duplicate declared Configuration"));
    }
    Ok(root.uuid)
}
fn form_uuid(obj: &MetadataObject, name: &str) -> Result<Uuid, FormError> {
    if obj.kind.as_str() == "CommonForm" && obj.name == name {
        return Ok(obj.uuid);
    }
    let mut children = obj
        .children
        .iter()
        .filter(|c| c.kind.as_str().ends_with(".FormRef") && c.name == name);
    let child = children
        .next()
        .ok_or_else(|| error("missing declared form UUID"))?;
    if children.next().is_some() {
        return Err(error("duplicate declared form UUID"));
    }
    Ok(child.uuid)
}
fn forms<'a>(
    objects: &'a [MetadataObject],
    f: &mut impl FnMut(Uuid, &'a morph1c_core::ir::FormBody) -> Result<(), FormError>,
) -> Result<(), FormError> {
    for obj in objects {
        for form in &obj.form_bodies {
            if form.ordinary_body.is_none() {
                f(form_uuid(obj, &form.name)?, &form.body)?;
            }
        }
        forms(&obj.children, f)?;
    }
    Ok(())
}
impl NativeDataPathAnnotation {
    pub fn form_resource(&self, uuid: Uuid) -> Result<Option<Vec<u8>>, FormError> {
        self.envelope
            .data_paths
            .iter()
            .find(|r| paths::resource_uuid(r) == uuid)
            .map(paths::resource_bytes)
            .transpose()
    }
    /// All current form UUIDs, current restored paths and actual metadata branches
    /// verify before read_config publishes its local Configuration value.
    pub fn verify_configuration(
        &self,
        cfg: &Configuration,
        profile: FormatVersion,
    ) -> Result<(), FormError> {
        if configuration_uuid(cfg)? != self.envelope.configuration_uuid {
            return Err(error("Configuration UUID differs"));
        }
        let context = FormProjectionContext::new(cfg)?;
        let mut pending: BTreeMap<_, _> = self
            .envelope
            .data_paths
            .iter()
            .map(|r| (paths::resource_uuid(r), r))
            .collect();
        let mut seen = BTreeSet::new();
        forms(&cfg.objects, &mut |uuid, body| {
            if !seen.insert(uuid) {
                return Err(error("duplicate current form UUID"));
            }
            if let Some(resource) = pending.remove(&uuid) {
                paths::verify_restored_data_path_semantics(
                    body,
                    uuid,
                    FormDialect::Designer,
                    profile,
                    &paths::resource_bytes(resource)?,
                    &context,
                )?;
            }
            Ok(())
        })?;
        if !pending.is_empty() {
            return Err(error("annotation has deleted/unknown/ordinary form"));
        }
        Ok(())
    }
}
fn encode(bytes: &[u8]) -> Result<String, FormError> {
    const ALPH: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let capacity = bytes
        .len()
        .checked_add(2)
        .and_then(|n| n.checked_div(3))
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| error("actual encoding length overflow"))?;
    let mut out = String::new();
    out.try_reserve(capacity)
        .map_err(|e| error(format!("encoding allocation: {e}")))?;
    for part in bytes.chunks(3) {
        let a = part[0];
        let b = part.get(1).copied().unwrap_or(0);
        let c = part.get(2).copied().unwrap_or(0);
        out.push(ALPH[(a >> 2) as usize] as char);
        out.push(ALPH[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        out.push(if part.len() > 1 {
            ALPH[(((b & 15) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        out.push(if part.len() > 2 {
            ALPH[(c & 63) as usize] as char
        } else {
            '='
        });
    }
    Ok(out)
}
fn decode(text: &str) -> Result<Vec<u8>, FormError> {
    fn value(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    if text.len() % 4 != 0 {
        return Err(error("truncated base64"));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve(text.len() / 4 * 3)
        .map_err(|e| error(format!("decoding allocation: {e}")))?;
    for (i, p) in text.as_bytes().chunks(4).enumerate() {
        let a = value(p[0]).ok_or_else(|| error("base64 symbol"))?;
        let b = value(p[1]).ok_or_else(|| error("base64 symbol"))?;
        let c = if p[2] == b'=' {
            0
        } else {
            value(p[2]).ok_or_else(|| error("base64 symbol"))?
        };
        let d = if p[3] == b'=' {
            0
        } else {
            value(p[3]).ok_or_else(|| error("base64 symbol"))?
        };
        if (p[2] == b'=' || p[3] == b'=') && i + 1 != text.len() / 4 {
            return Err(error("nonterminal base64 padding"));
        }
        if p[2] == b'=' && p[3] != b'=' {
            return Err(error("base64 padding"));
        }
        bytes.push((a << 2) | (b >> 4));
        if p[2] != b'=' {
            bytes.push((b << 4) | (c >> 2));
        }
        if p[3] != b'=' {
            bytes.push((c << 6) | d);
        }
    }
    if encode(&bytes)? != text {
        return Err(error("noncanonical base64"));
    }
    Ok(bytes)
}
/// Recognize only a direct-child protocol comment in the exact native root.
/// Parse through EOF even after finding the payload; malformed tails never hide.
pub fn read_native_data_path_annotation(
    bytes: &[u8],
    profile: FormatVersion,
) -> Result<Option<NativeDataPathAnnotation>, FormError> {
    let mut reader = NsReader::from_reader(bytes);
    let mut depth = 0usize;
    let mut root = false;
    let mut closed = false;
    let mut payload = None;
    loop {
        let (ns, event) = reader
            .read_resolved_event()
            .map_err(|e| error(e.to_string()))?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(start) | Event::Empty(start) => {
                if depth == 0 {
                    if root || closed {
                        return Err(error("multiple XML roots"));
                    }
                    if !matches!(ns,ResolveResult::Bound(n)if n.as_ref()==NS.as_bytes())
                        || start.local_name().as_ref() != b"ConfigDumpInfo"
                    {
                        return Err(error("wrong native manifest root/namespace"));
                    }
                    root = true;
                    let mut version = None;
                    for attr in start.attributes() {
                        let attr = attr.map_err(|e| error(e.to_string()))?;
                        if attr.key.as_ref() == b"version" {
                            version = Some(
                                attr.unescape_value()
                                    .map_err(|e| error(e.to_string()))?
                                    .into_owned(),
                            );
                        }
                    }
                    if version.as_deref()
                        != Some(format!("{}.{}", profile.major, profile.minor).as_str())
                    {
                        return Err(error("manifest profile differs"));
                    }
                }
                // Empty nodes do not advance ancestry. NsReader already checks end names.
                if !empty {
                    depth = depth
                        .checked_add(1)
                        .ok_or_else(|| error("actual XML ancestry overflow"))?;
                } else if depth == 0 {
                    closed = true;
                }
            }
            Event::End(_) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| error("unbalanced manifest"))?;
                if depth == 0 {
                    closed = true;
                }
            }
            Event::Comment(comment) => {
                let text = std::str::from_utf8(comment.as_ref())
                    .map_err(|e| error(e.to_string()))?
                    .trim();
                if text.starts_with(FAMILY) || text == FAMILY.trim_end_matches(':') {
                    let encoded = text
                        .strip_prefix(PREFIX)
                        .ok_or_else(|| error("unknown/malformed reserved annotation version"))?;
                    if depth != 1 || payload.is_some() {
                        return Err(error("duplicate/wrong-ancestry annotation"));
                    }
                    let raw = decode(encoded)?;
                    let model: Envelope =
                        super::strict_resource::parse(&raw).map_err(|e| error(e.to_string()))?;
                    if model.schema != SCHEMA || model.version != 1 || model.data_paths.is_empty() {
                        return Err(error("annotation schema/empty records"));
                    }
                    let mut seen = BTreeSet::new();
                    for resource in &model.data_paths {
                        let resource = paths::parse_resource(&paths::resource_bytes(resource)?)?;
                        if !paths::resource_native(&resource)
                            || paths::resource_profile(&resource) != (profile.major, profile.minor)
                            || !seen.insert(paths::resource_uuid(&resource))
                        {
                            return Err(error("duplicate/wrong-profile form section"));
                        }
                    }
                    payload = Some(NativeDataPathAnnotation { envelope: model });
                }
            }
            Event::Eof => break,
            Event::DocType(_) => return Err(error("DTD is not supported")),
            Event::Text(text) if depth == 0 => {
                if !text.as_ref().iter().all(u8::is_ascii_whitespace) {
                    return Err(error("text outside manifest root"));
                }
            }
            _ => {}
        }
    }
    if !root || !closed || depth != 0 {
        return Err(error("incomplete manifest"));
    }
    Ok(payload)
}
/// Regenerate the manifest's declaration roster from CURRENT typed metadata.
/// Platform incremental dump checksums are not invented or replayed.
pub fn write_native_data_path_annotation(
    cfg: &Configuration,
    profile: FormatVersion,
) -> Result<Option<Vec<u8>>, FormError> {
    let context = FormProjectionContext::new(cfg)?;
    let mut records = Vec::new();
    let mut seen = BTreeSet::new();
    forms(&cfg.objects, &mut |uuid, body| {
        if !seen.insert(uuid) {
            return Err(error("duplicate current form UUID"));
        }
        if let Some((_, bytes)) = paths::project_data_path_semantics(
            body,
            uuid,
            FormDialect::Designer,
            profile,
            Some(&context),
        )? {
            records.push(paths::parse_resource(&bytes)?);
        }
        Ok(())
    })?;
    if records.is_empty() {
        return Ok(None);
    }
    let model = Envelope {
        schema: SCHEMA.into(),
        version: 1,
        configuration_uuid: configuration_uuid(cfg)?,
        data_paths: records,
    };
    let raw = serde_json::to_vec(&model).map_err(|e| error(e.to_string()))?;
    let comment = encode(&raw)?;
    let mut root = crate::emit::OutElement::branch("", "ConfigDumpInfo")
        .attr("xmlns", NS)
        .attr("format", "Hierarchical")
        .attr("version", format!("{}.{}", profile.major, profile.minor));
    let mut roster = crate::emit::OutElement::branch("", "ConfigVersions");
    let mut names = BTreeSet::new();
    fn rows(
        objects: &[MetadataObject],
        parent: Option<&str>,
        roster: &mut crate::emit::OutElement,
        names: &mut BTreeSet<String>,
    ) -> Result<(), FormError> {
        for obj in objects {
            let kind = obj.kind.as_str();
            let short = kind
                .rsplit('.')
                .next()
                .unwrap_or(kind)
                .trim_end_matches("Ref");
            let name = match parent {
                Some(p) => format!("{p}.{short}.{}", obj.name),
                None => format!("{kind}.{}", obj.name),
            };
            if !names.insert(name.clone()) {
                return Err(error("duplicate CURRENT manifest name"));
            }
            roster.push(
                crate::emit::OutElement::branch("", "Metadata")
                    .attr("name", &name)
                    .attr("id", crate::children::format_uuid(&obj.uuid)),
            );
            rows(&obj.children, Some(&name), roster, names)?;
        }
        Ok(())
    }
    rows(&cfg.objects, None, &mut roster, &mut names)?;
    root.push(roster);
    let mut out = crate::emit::render(&super::designer_envelope(), &root);
    let end = b"</ConfigDumpInfo>";
    let index = out
        .windows(end.len())
        .rposition(|s| s == end)
        .ok_or_else(|| error("manifest emission root missing"))?;
    let annotation = format!("\t<!-- {PREFIX}{comment} -->\r\n");
    out.splice(index..index, annotation.bytes());
    Ok(Some(out))
}

/// A recognized typed control is compared after full current configuration read.
/// None denotes the historical plain storage manifest, retained independently by
/// whole-source provenance; its platform incremental cache is not model data.
pub fn compare_native_data_path_annotations(
    source: &[u8],
    rebuilt: Option<&[u8]>,
    profile: FormatVersion,
) -> Result<Option<bool>, FormError> {
    let source = read_native_data_path_annotation(source, profile)?;
    match source {
        None => Ok(None),
        Some(source) => match rebuilt {
            None => Ok(Some(false)),
            Some(bytes) => Ok(Some(
                read_native_data_path_annotation(bytes, profile)?
                    .is_some_and(|rebuilt| source.envelope == rebuilt.envelope),
            )),
        },
    }
}
