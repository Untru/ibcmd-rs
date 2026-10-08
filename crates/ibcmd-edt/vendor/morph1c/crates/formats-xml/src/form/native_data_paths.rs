//! One closed CURRENT configuration annotation in the existing native manifest.
//! It transports semantic paths, never XML source bytes or stale manifest versions.
use super::{
    data_path_semantics as paths, form_presence as presence, FormDialect, FormError,
    FormProjectionContext,
};
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
const PREFIX_V2: &str = "ibcmd-configuration-semantics:2:";
const SCHEMA_V2: &str = "urn:ibcmd:source-extension:configuration-semantics:2";
const SCHEMA: &str = "urn:ibcmd:source-extension:configuration-semantics:1";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvelopeV1 {
    schema: String,
    version: u32,
    configuration_uuid: Uuid,
    data_paths: Vec<paths::Resource>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvelopeV2 {
    schema: String,
    version: u32,
    configuration_uuid: Uuid,
    data_paths: Vec<paths::Resource>,
    form_presence: Vec<presence::Resource>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum Envelope {
    V1(EnvelopeV1),
    V2(EnvelopeV2),
}
impl Envelope {
    fn data_paths(&self) -> &[paths::Resource] {
        match self {
            Self::V1(e) => &e.data_paths,
            Self::V2(e) => &e.data_paths,
        }
    }
    fn form_presence(&self) -> &[presence::Resource] {
        match self {
            Self::V1(_) => &[],
            Self::V2(e) => &e.form_presence,
        }
    }
    fn uuid(&self) -> Uuid {
        match self {
            Self::V1(e) => e.configuration_uuid,
            Self::V2(e) => e.configuration_uuid,
        }
    }
    fn bytes(&self) -> Result<Vec<u8>, FormError> {
        match self {
            Self::V1(e) => serde_json::to_vec(e),
            Self::V2(e) => serde_json::to_vec(e),
        }
        .map_err(|e| error(e.to_string()))
    }
}
/// Ephemeral CURRENT output plan. These are newly emitted artifacts, not retained
/// input XML. Each body, chart companion and root record share one invocation.
#[derive(Debug, Clone)]
pub struct NativeFormWritePlan {
    pub manifest: Option<Vec<u8>>,
    forms: BTreeMap<Uuid, presence::PreparedFormPresence>,
}
impl NativeFormWritePlan {
    pub fn form(&self, uuid: Uuid) -> Option<&presence::PreparedFormPresence> {
        self.forms.get(&uuid)
    }
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
            .data_paths()
            .iter()
            .find(|r| paths::resource_uuid(r) == uuid)
            .map(paths::resource_bytes)
            .transpose()
    }
    pub fn form_presence_resource(&self, uuid: Uuid) -> Result<Option<Vec<u8>>, FormError> {
        self.envelope
            .form_presence()
            .iter()
            .find(|r| presence::resource_uuid(r) == uuid)
            .map(presence::resource_bytes)
            .transpose()
    }
    /// All current form UUIDs, current restored paths and actual metadata branches
    /// verify before read_config publishes its local Configuration value.
    pub fn verify_configuration(
        &self,
        cfg: &Configuration,
        profile: FormatVersion,
    ) -> Result<(), FormError> {
        if configuration_uuid(cfg)? != self.envelope.uuid() {
            return Err(error("Configuration UUID differs"));
        }
        let context = FormProjectionContext::new(cfg)?;
        let mut pending: BTreeMap<_, _> = self
            .envelope
            .data_paths()
            .iter()
            .map(|r| (paths::resource_uuid(r), r))
            .collect();
        let mut pending_presence: BTreeMap<_, _> = self
            .envelope
            .form_presence()
            .iter()
            .map(|r| (presence::resource_uuid(r), r))
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
            if let Some(resource) = pending_presence.remove(&uuid) {
                presence::verify_restored(body, uuid, profile, resource, &context)?;
            }
            Ok(())
        })?;
        if !pending.is_empty() || !pending_presence.is_empty() {
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
                    let (encoded, v2) = if let Some(encoded) = text.strip_prefix(PREFIX) {
                        (encoded, false)
                    } else if let Some(encoded) = text.strip_prefix(PREFIX_V2) {
                        (encoded, true)
                    } else {
                        return Err(error("unknown/malformed reserved annotation version"));
                    };
                    if depth != 1 || payload.is_some() {
                        return Err(error("duplicate/wrong-ancestry annotation"));
                    }
                    let raw = decode(encoded)?;
                    let model = if v2 {
                        let model: EnvelopeV2 = super::strict_resource::parse(&raw)
                            .map_err(|e| error(e.to_string()))?;
                        if model.schema != SCHEMA_V2
                            || model.version != 2
                            || model.form_presence.is_empty()
                        {
                            return Err(error("annotation v2 schema/empty new facets"));
                        }
                        Envelope::V2(model)
                    } else {
                        let model: EnvelopeV1 = super::strict_resource::parse(&raw)
                            .map_err(|e| error(e.to_string()))?;
                        if model.schema != SCHEMA
                            || model.version != 1
                            || model.data_paths.is_empty()
                        {
                            return Err(error("annotation v1 schema/empty records"));
                        }
                        Envelope::V1(model)
                    };
                    let mut seen_presence = BTreeSet::new();
                    for resource in model.form_presence() {
                        let resource =
                            presence::parse_resource(&presence::resource_bytes(resource)?)?;
                        if presence::resource_profile(&resource) != (profile.major, profile.minor)
                            || !seen_presence.insert(presence::resource_uuid(&resource))
                        {
                            return Err(error("duplicate/wrong-profile root section"));
                        }
                    }
                    let mut seen = BTreeSet::new();
                    for resource in model.data_paths() {
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
    Ok(prepare_native_form_write_plan(cfg, profile)?.manifest)
}

/// Prepare before any output publication. A native descriptor and its root/chart
/// companions are emitted from this plan, and the manifest uses those same records.
pub fn prepare_native_form_write_plan(
    cfg: &Configuration,
    profile: FormatVersion,
) -> Result<NativeFormWritePlan, FormError> {
    let context = FormProjectionContext::new(cfg)?;
    let mut prepared_forms = BTreeMap::new();
    let mut records = Vec::new();
    let mut root_records = Vec::new();
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
        let prepared = presence::prepare_form_presence(
            body,
            uuid,
            FormDialect::Designer,
            profile,
            Some(&context),
        )?;
        if let Some(bytes) = &prepared.resource {
            root_records.push(presence::parse_resource(bytes)?);
        }
        prepared_forms.insert(uuid, prepared);
        Ok(())
    })?;
    if records.is_empty() && root_records.is_empty() {
        return Ok(NativeFormWritePlan {
            manifest: None,
            forms: prepared_forms,
        });
    }
    let v2 = !root_records.is_empty();
    let model = if v2 {
        Envelope::V2(EnvelopeV2 {
            schema: SCHEMA_V2.into(),
            version: 2,
            configuration_uuid: configuration_uuid(cfg)?,
            data_paths: records,
            form_presence: root_records,
        })
    } else {
        Envelope::V1(EnvelopeV1 {
            schema: SCHEMA.into(),
            version: 1,
            configuration_uuid: configuration_uuid(cfg)?,
            data_paths: records,
        })
    };
    let raw = model.bytes()?;
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
    let prefix = if v2 { PREFIX_V2 } else { PREFIX };
    let annotation = format!("\t<!-- {prefix}{comment} -->\r\n");
    out.splice(index..index, annotation.bytes());
    Ok(NativeFormWritePlan {
        manifest: Some(out),
        forms: prepared_forms,
    })
}

/// Replace/remove only our closed direct-root protocol comment. Existing platform
/// roster, attributes, unrelated comments and whitespace remain byte-identical.
/// Both inputs are validated completely before any destination publication.
pub fn update_native_data_path_annotation(
    existing: Option<&[u8]>,
    current: Option<&[u8]>,
    profile: FormatVersion,
    expected_configuration_uuid: Option<Uuid>,
) -> Result<Option<Vec<u8>>, FormError> {
    let fresh = current
        .map(|bytes| read_native_data_path_annotation(bytes, profile))
        .transpose()?
        .flatten();
    if current.is_some() && fresh.is_none() {
        return Err(error("CURRENT manifest has no owned annotation"));
    }
    if let Some(new) = &fresh {
        if Some(new.envelope.uuid()) != expected_configuration_uuid {
            return Err(error("foreign CURRENT Configuration UUID"));
        }
    }
    let Some(existing) = existing else {
        return Ok(current.map(<[u8]>::to_vec));
    };
    let previous = read_native_data_path_annotation(existing, profile)?;
    if let Some(old) = &previous {
        if Some(old.envelope.uuid()) != expected_configuration_uuid {
            return Err(error("foreign existing Configuration UUID"));
        }
    }
    if previous.is_none() && current.is_none() {
        return Ok(None);
    }
    struct EmptyRoot {
        span: std::ops::Range<usize>,
        name: Vec<u8>,
    }
    struct CommentPositions {
        owned: Option<std::ops::Range<usize>>,
        close: usize,
        empty_root: Option<EmptyRoot>,
    }
    fn positions(bytes: &[u8]) -> Result<CommentPositions, FormError> {
        let mut reader = NsReader::from_reader(bytes);
        let mut depth = 0usize;
        let mut owned = None;
        let mut close = None;
        let mut empty_root = None;
        loop {
            let start = usize::try_from(reader.buffer_position())
                .map_err(|_| error("actual XML offset overflow"))?;
            let event = reader.read_event().map_err(|e| error(e.to_string()))?;
            let end = usize::try_from(reader.buffer_position())
                .map_err(|_| error("actual XML offset overflow"))?;
            match event {
                Event::Start(_) => depth += 1,
                Event::Empty(root) if depth == 0 => {
                    empty_root = Some(EmptyRoot {
                        span: start..end,
                        name: root.name().as_ref().to_vec(),
                    });
                    close = Some(start);
                }
                Event::End(_) => {
                    if depth == 1 {
                        close = Some(start);
                    }
                    depth -= 1;
                }
                Event::Comment(comment) if depth == 1 => {
                    let text = std::str::from_utf8(comment.as_ref())
                        .map_err(|e| error(e.to_string()))?
                        .trim();
                    if text.starts_with(FAMILY) {
                        owned = Some(start..end);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        Ok(CommentPositions {
            owned,
            close: close.ok_or_else(|| error("manifest has no closing root"))?,
            empty_root,
        })
    }
    let previous_positions = positions(existing)?;
    let comment = if let Some(bytes) = current {
        let current_positions = positions(bytes)?;
        Some(
            &bytes[current_positions
                .owned
                .ok_or_else(|| error("CURRENT annotation comment missing"))?],
        )
    } else {
        None
    };
    let mut out = existing.to_vec();
    if let Some(span) = previous_positions.owned {
        out.splice(span, comment.into_iter().flatten().copied());
    } else if let Some(comment) = comment {
        if let Some(EmptyRoot { span, name }) = previous_positions.empty_root {
            let suffix = span
                .end
                .checked_sub(2)
                .ok_or_else(|| error("empty manifest root malformed"))?;
            if &out[suffix..span.end] != b"/>" {
                return Err(error("empty manifest root malformed"));
            }
            let mut expanded = b">".to_vec();
            expanded.extend_from_slice(comment);
            expanded.extend_from_slice(b"</");
            expanded.extend_from_slice(&name);
            expanded.push(b'>');
            out.splice(suffix..span.end, expanded);
        } else {
            out.splice(
                previous_positions.close..previous_positions.close,
                comment.iter().copied(),
            );
        }
    }
    read_native_data_path_annotation(&out, profile)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use morph1c_core::{
        ir::{form::DataPathSpec, FormBody, FormControlKind, FormItem, PropertyValue},
        spec::forms::controls::form_field as ff,
    };
    fn manifest(raw: &[u8], prefix: &str, profile: FormatVersion) -> Vec<u8> {
        format!("<ConfigDumpInfo xmlns=\"{NS}\" version=\"{}.{}\"><!-- {prefix}{} --><ConfigVersions/></ConfigDumpInfo>",
            profile.major, profile.minor, encode(raw).unwrap()).into_bytes()
    }
    fn path_resource(profile: FormatVersion) -> paths::Resource {
        let mut body = FormBody::new();
        let mut field = FormItem::new(FormControlKind::new("LabelField"), "Field", 1);
        field.properties.push((
            ff::F_DATA_PATH,
            PropertyValue::DataPath(DataPathSpec {
                segments: vec!["A".into(), "B~literal".into()],
                extra_paths: vec!["C.D".into()],
            }),
        ));
        body.items.push(field);
        let (_, bytes) = paths::project_data_path_semantics(
            &body,
            Uuid([7; 16]),
            FormDialect::Designer,
            profile,
            None,
        )
        .unwrap()
        .unwrap();
        paths::parse_resource(&bytes).unwrap()
    }
    fn root_resource(profile: FormatVersion) -> presence::Resource {
        let xml = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><autoCommandBar><name>FormCommandBar</name><id>-1</id></autoCommandBar></form:Form>\r\n"
        );
        let body = morph1c_core::version::with_source_version(Some(profile), || {
            super::super::read_form(FormDialect::Edt, xml.as_bytes())
        })
        .unwrap();
        let package = presence::prepare_form_presence(
            &body,
            Uuid([7; 16]),
            FormDialect::Designer,
            profile,
            None,
        )
        .unwrap();
        presence::parse_resource(package.resource.as_ref().unwrap()).unwrap()
    }
    #[test]
    fn closed_v1_stays_exact_and_v2_discriminants_and_new_facets_are_strict() {
        for minor in [20, 21] {
            let profile = FormatVersion::new(2, minor);
            let v1 = EnvelopeV1 {
                schema: SCHEMA.into(),
                version: 1,
                configuration_uuid: Uuid([1; 16]),
                data_paths: vec![path_resource(profile)],
            };
            let raw = serde_json::to_vec(&v1).unwrap();
            let wire = manifest(&raw, PREFIX, profile);
            let parsed = read_native_data_path_annotation(&wire, profile)
                .unwrap()
                .unwrap();
            assert_eq!(parsed.envelope.bytes().unwrap(), raw);
            assert!(matches!(parsed.envelope, Envelope::V1(_)));
            let mut wrong = serde_json::to_value(&v1).unwrap();
            wrong["form_presence"] = serde_json::json!([]);
            assert!(read_native_data_path_annotation(
                &manifest(&serde_json::to_vec(&wrong).unwrap(), PREFIX, profile),
                profile
            )
            .is_err());
            let v2 = EnvelopeV2 {
                schema: SCHEMA_V2.into(),
                version: 2,
                configuration_uuid: Uuid([1; 16]),
                data_paths: vec![path_resource(profile)],
                form_presence: vec![root_resource(profile)],
            };
            let raw = serde_json::to_vec(&v2).unwrap();
            let wire = manifest(&raw, PREFIX_V2, profile);
            let parsed = read_native_data_path_annotation(&wire, profile)
                .unwrap()
                .unwrap();
            assert_eq!(parsed.envelope.bytes().unwrap(), raw);
            assert!(parsed.form_resource(Uuid([7; 16])).unwrap().is_some());
            assert!(parsed
                .form_presence_resource(Uuid([7; 16]))
                .unwrap()
                .is_some());
            for kind in 0..6 {
                let mut wrong = serde_json::to_value(&v2).unwrap();
                match kind {
                    0 => wrong["version"] = 1.into(),
                    1 => wrong["schema"] = SCHEMA.into(),
                    2 => wrong["form_presence"] = serde_json::json!([]),
                    3 => wrong["opaque"] = true.into(),
                    4 => {
                        wrong["form_presence"] = serde_json::json!([
                            v2.form_presence[0].clone(),
                            v2.form_presence[0].clone()
                        ])
                    }
                    _ => {
                        wrong.as_object_mut().unwrap().remove("data_paths");
                    }
                }
                assert!(read_native_data_path_annotation(
                    &manifest(&serde_json::to_vec(&wrong).unwrap(), PREFIX_V2, profile),
                    profile
                )
                .is_err());
            }
            assert!(
                read_native_data_path_annotation(&manifest(&raw, PREFIX, profile), profile)
                    .is_err()
            );
            assert!(read_native_data_path_annotation(
                &manifest(&raw, "ibcmd-configuration-semantics:3:", profile),
                profile
            )
            .is_err());
        }
    }
}
