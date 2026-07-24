//! Deterministic offline form/raw/XML correlation for a saved parity corpus.
use crate::cli::FormProvenanceCorpusArgs;
use crate::module_blob::parse_form_body_plain;
use crate::mssql_dump::offline_context::OfflineFormContextFactory;
use crate::mssql_dump::{
    FormItemSchemaTraceEvent, FormItemTraceEvent, FormItemTraceSink, trace_form_body_with_context,
};
use anyhow::{Context, Result, anyhow};
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use serde::Serialize;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

const DEFAULT_PROPERTIES: &[&str] = &[
    "AutoMaxWidth",
    "AutoAddIncomplete",
    "DataPath",
    "TitleDataPath",
];

#[derive(Debug, Serialize)]
pub struct FormProvenanceSummary {
    pub schema_version: u32,
    pub source_commit: Option<String>,
    pub run_id: String,
    pub input_forms: usize,
    pub read_forms: usize,
    pub parsed_forms: usize,
    pub errors: usize,
    pub reasons: BTreeMap<String, usize>,
    pub raw_events: usize,
    pub raw_unique: usize,
    pub raw_collisions: usize,
    pub matched_both: usize,
    pub name_mismatch: usize,
    pub missing_native: usize,
    pub missing_candidate: usize,
    pub missing_both: usize,
    pub native_only: usize,
    pub candidate_only: usize,
    pub xml_only_both: usize,
    pub xml_collisions: usize,
    pub denominator: usize,
    pub rate_percent: f64,
    pub output: String,
}
#[derive(Clone, Serialize)]
struct Record {
    schema_version: u32,
    source_commit: Option<String>,
    run_id: String,
    path: String,
    key: String,
    occurrence: usize,
    raw: Option<FormItemTraceEvent>,
    schema: Option<FormItemSchemaTraceEvent>,
    native: Option<XmlItem>,
    candidate: Option<XmlItem>,
    state: String,
}
#[derive(Clone, Serialize)]
struct ErrorRecord {
    schema_version: u32,
    source_commit: Option<String>,
    run_id: String,
    path: String,
    stage: String,
    reason: String,
    message: String,
}
#[derive(Clone, Serialize)]
struct XmlItem {
    id: String,
    tag: String,
    name: String,
    owner_chain: Vec<String>,
    properties: BTreeMap<String, XmlProperty>,
}
#[derive(Clone, Serialize)]
struct XmlProperty {
    present: bool,
    value: Option<String>,
    order: usize,
    nil: bool,
    empty: bool,
}
struct Node {
    tag: String,
    item: Option<XmlItem>,
    ordinal: usize,
    text: String,
    nil: bool,
}
struct Sink {
    raw: RefCell<Vec<FormItemTraceEvent>>,
    schema: RefCell<Vec<FormItemSchemaTraceEvent>>,
}
impl Sink {
    fn new() -> Self {
        Self {
            raw: RefCell::new(Vec::new()),
            schema: RefCell::new(Vec::new()),
        }
    }
    fn take(self) -> (Vec<FormItemTraceEvent>, Vec<FormItemSchemaTraceEvent>) {
        (self.raw.into_inner(), self.schema.into_inner())
    }
}
impl FormItemTraceSink for Sink {
    fn record(&self, e: FormItemTraceEvent) {
        self.raw.borrow_mut().push(e)
    }
    fn record_schema(&self, e: FormItemSchemaTraceEvent) {
        self.schema.borrow_mut().push(e)
    }
}

pub fn run_form_provenance_corpus(
    args: &FormProvenanceCorpusArgs,
) -> Result<FormProvenanceSummary> {
    let properties = filters(&args.property)?;
    let context =
        OfflineFormContextFactory::from_run_root(&args.run_root, args.source_commit.clone())?;
    let manifest_path = args.run_root.join("candidate_dump/manifest.json");
    let manifest: Value = serde_json::from_slice(
        &fs::read(&manifest_path).with_context(|| format!("read {}", manifest_path.display()))?,
    )?;
    let mut forms = manifest_rows(&manifest)?;
    forms.sort();
    forms.dedup();
    let run_id = args
        .run_root
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or_default()
        .to_string();
    let mut s = FormProvenanceSummary {
        schema_version: 1,
        source_commit: args.source_commit.clone(),
        run_id: run_id.clone(),
        input_forms: forms.len(),
        read_forms: 0,
        parsed_forms: 0,
        errors: 0,
        reasons: BTreeMap::new(),
        raw_events: 0,
        raw_unique: 0,
        raw_collisions: 0,
        matched_both: 0,
        name_mismatch: 0,
        missing_native: 0,
        missing_candidate: 0,
        missing_both: 0,
        native_only: 0,
        candidate_only: 0,
        xml_only_both: 0,
        xml_collisions: 0,
        denominator: 0,
        rate_percent: 0.,
        output: args.output.display().to_string(),
    };
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    for (path, inflated, file_name) in forms {
        let raw_path = args.run_root.join("candidate_dump").join(&inflated);
        let plain = match fs::read_to_string(&raw_path) {
            Ok(x) => {
                s.read_forms += 1;
                x
            }
            Err(e) => {
                err(&mut s, &mut errors, &path, "raw_read", e);
                continue;
            }
        };
        let body = match parse_form_body_plain(&plain) {
            Ok(x) => {
                s.parsed_forms += 1;
                x
            }
            Err(e) => {
                err(&mut s, &mut errors, &path, "body_parse", e);
                continue;
            }
        };
        let sink = Sink::new();
        let form_id = match form_id_from_manifest_file_name(&file_name) {
            Ok(id) => id,
            Err(e) => {
                err(&mut s, &mut errors, &path, "context_owner", e);
                continue;
            }
        };
        let owner =
            match form_owner_for_manifest_path(&path, form_id, &context.form_owner_references) {
                Ok(owner) => owner,
                Err(e) => {
                    err(&mut s, &mut errors, &path, "context_owner", e);
                    continue;
                }
            };
        if trace_form_body_with_context(
            &body,
            &context.type_index,
            &context.dcs_type_index,
            &context.object_refs,
            &context.information_register_field_refs,
            owner,
            &sink,
        )
        .is_none()
        {
            err(
                &mut s,
                &mut errors,
                &path,
                "raw_trace",
                "production context extraction failed",
            );
            continue;
        };
        let (raw, schema) = sink.take();
        s.raw_events += raw.len();
        let native = match parse_xml_items(&args.run_root.join("native").join(&path), &properties) {
            Ok(x) => x,
            Err(e) => {
                err(&mut s, &mut errors, &path, "native_xml", e);
                Vec::new()
            }
        };
        let candidate =
            match parse_xml_items(&args.run_root.join("candidate").join(&path), &properties) {
                Ok(x) => x,
                Err(e) => {
                    err(&mut s, &mut errors, &path, "candidate_xml", e);
                    Vec::new()
                }
            };
        rows.extend(correlate(
            &path,
            raw,
            schema,
            native,
            candidate,
            &mut s,
            &run_id,
            args.source_commit.clone(),
        ));
    }
    let text = serialize_records(rows, errors)?;
    fs::write(&args.output, text).with_context(|| format!("write {}", args.output.display()))?;
    s.denominator = s.raw_events;
    s.rate_percent = if s.denominator == 0 {
        0.
    } else {
        s.matched_both as f64 * 100. / s.denominator as f64
    };
    Ok(s)
}
fn serialize_records(mut rows: Vec<Record>, mut errors: Vec<ErrorRecord>) -> Result<String> {
    rows.sort_by(|a, b| {
        (&a.path, &a.key, a.occurrence, &a.state).cmp(&(&b.path, &b.key, b.occurrence, &b.state))
    });
    errors.sort_by(|a, b| (&a.path, &a.stage, &a.message).cmp(&(&b.path, &b.stage, &b.message)));
    let mut text = String::new();
    for r in rows {
        text.push_str(&serde_json::to_string(&r)?);
        text.push('\n');
    }
    for e in errors {
        text.push_str(&serde_json::to_string(&e)?);
        text.push('\n');
    }
    Ok(text)
}
fn err(
    s: &mut FormProvenanceSummary,
    out: &mut Vec<ErrorRecord>,
    path: &str,
    stage: &str,
    e: impl std::fmt::Display,
) {
    s.errors += 1;
    *s.reasons.entry(stage.into()).or_default() += 1;
    out.push(ErrorRecord {
        schema_version: 1,
        source_commit: s.source_commit.clone(),
        run_id: s.run_id.clone(),
        path: path.into(),
        stage: stage.into(),
        reason: stage.into(),
        message: e.to_string(),
    })
}
fn filters(input: &[String]) -> Result<Vec<String>> {
    let p = if input.is_empty() {
        DEFAULT_PROPERTIES.iter().map(|x| x.to_string()).collect()
    } else {
        input.to_vec()
    };
    for x in &p {
        if !DEFAULT_PROPERTIES.contains(&x.as_str()) {
            return Err(anyhow!("unsupported property filter: {x}"));
        }
    }
    Ok(p)
}
fn safe(p: &str) -> bool {
    let q = Path::new(p);
    !q.is_absolute() && q.components().all(|c| matches!(c, Component::Normal(_)))
}
fn manifest_rows(v: &Value) -> Result<Vec<(String, PathBuf, String)>> {
    let mut r = Vec::new();
    for t in v
        .get("tables")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("manifest.tables missing"))?
    {
        for x in t
            .get("rows")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("manifest rows missing"))?
        {
            let (a, b, file_name) = (
                x.get("source_asset_path").and_then(Value::as_str),
                x.get("inflated_path").and_then(Value::as_str),
                x.get("file_name").and_then(Value::as_str),
            );
            if let Some(a) = a {
                if a.ends_with("/Ext/Form.xml") {
                    let (b, file_name) = (b, file_name);
                    let b = b.ok_or_else(|| anyhow!("manifest inflated_path missing for {a}"))?;
                    let file_name =
                        file_name.ok_or_else(|| anyhow!("manifest file_name missing for {a}"))?;
                    if !safe(a) || !safe(b) || !safe(file_name) {
                        return Err(anyhow!("unsafe manifest path"));
                    }
                    r.push((a.into(), PathBuf::from(b), file_name.into()));
                }
            }
        }
    }
    Ok(r)
}
fn form_id_from_manifest_file_name(file_name: &str) -> Result<&str> {
    if !safe(file_name)
        || Path::new(file_name).file_name().and_then(|x| x.to_str()) != Some(file_name)
    {
        return Err(anyhow!("unsafe manifest file_name: {file_name}"));
    }
    let id = file_name.strip_suffix(".0").ok_or_else(|| {
        anyhow!("unexpected form manifest file_name (expected UUID.0): {file_name}")
    })?;
    uuid::Uuid::parse_str(id)
        .with_context(|| format!("invalid form UUID in manifest file_name: {file_name}"))?;
    Ok(id)
}
fn form_owner_for_manifest_path<'a>(
    source_asset_path: &str,
    form_id: &str,
    owner_references: &'a BTreeMap<String, String>,
) -> Result<Option<&'a str>> {
    if source_asset_path.starts_with("CommonForms/") {
        return Ok(None);
    }
    owner_references
        .get(form_id)
        .map(String::as_str)
        .map(Some)
        .ok_or_else(|| anyhow!("missing owner reference for form {form_id} ({source_asset_path})"))
}
fn attrs(
    e: &BytesStart<'_>,
    r: &Reader<&[u8]>,
) -> Result<(String, Option<String>, Option<String>, bool)> {
    let tag = local(e.name().as_ref());
    let (mut id, mut name, mut nil) = (None, None, false);
    for a in e.attributes() {
        let a = a?;
        let k = local(a.key.as_ref());
        let v = a.decode_and_unescape_value(r.decoder())?.into_owned();
        if k == "id" {
            id = Some(v)
        } else if k == "name" {
            name = Some(v)
        } else if k == "nil" && v == "true" {
            nil = true
        }
    }
    Ok((tag, id, name, nil))
}
fn parse_xml_items(path: &Path, props: &[String]) -> Result<Vec<XmlItem>> {
    let b = fs::read(path).with_context(|| format!("read XML {}", path.display()))?;
    let mut r = Reader::from_reader(b.as_slice());
    r.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut st: Vec<Node> = Vec::new();
    let mut out = Vec::new();
    loop {
        match r.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let (t, id, n, nil) = attrs(&e, &r)?;
                let owner = st
                    .iter()
                    .filter_map(|x| x.item.as_ref().map(|i| format!("{}:{}", i.tag, i.id)))
                    .collect();
                st.push(Node {
                    tag: t.clone(),
                    item: id.zip(n).map(|(id, name)| XmlItem {
                        id,
                        tag: t,
                        name,
                        owner_chain: owner,
                        properties: BTreeMap::new(),
                    }),
                    ordinal: 0,
                    text: String::new(),
                    nil,
                })
            }
            Event::Empty(e) => {
                let (t, id, n, nil) = attrs(&e, &r)?;
                finish(
                    Node {
                        tag: t,
                        item: id.zip(n).map(|(id, name)| XmlItem {
                            id,
                            tag: String::new(),
                            name,
                            owner_chain: Vec::new(),
                            properties: BTreeMap::new(),
                        }),
                        ordinal: 0,
                        text: String::new(),
                        nil,
                    },
                    &mut st,
                    props,
                    &mut out,
                )
            }
            Event::Text(e) => {
                if let Some(x) = st.last_mut() {
                    x.text.push_str(e.decode()?.as_ref())
                }
            }
            Event::CData(e) => {
                if let Some(x) = st.last_mut() {
                    x.text.push_str(std::str::from_utf8(e.as_ref())?)
                }
            }
            Event::End(_) => {
                if let Some(n) = st.pop() {
                    finish(n, &mut st, props, &mut out)
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear()
    }
    Ok(out)
}
fn finish(mut n: Node, st: &mut Vec<Node>, props: &[String], out: &mut Vec<XmlItem>) {
    let direct_ordinal = st.last_mut().and_then(|parent| {
        parent.item.as_ref()?;
        let ordinal = parent.ordinal;
        parent.ordinal += 1;
        Some(ordinal)
    });
    if let Some(mut i) = n.item.take() {
        if i.tag.is_empty() {
            i.tag = n.tag.clone();
            i.owner_chain = st
                .iter()
                .filter_map(|x| x.item.as_ref().map(|z| format!("{}:{}", z.tag, z.id)))
                .collect()
        }
        out.push(i);
        return;
    }
    if let Some(parent) = st.last_mut() {
        if let Some(item) = parent.item.as_mut() {
            if props.iter().any(|p| p == &n.tag) {
                let empty = !n.nil && n.text.is_empty();
                let value = (!n.nil && !empty).then_some(n.text);
                item.properties.insert(
                    n.tag,
                    XmlProperty {
                        present: true,
                        value,
                        order: direct_ordinal.unwrap_or(parent.ordinal),
                        nil: n.nil,
                        empty,
                    },
                );
            }
        }
    }
}
fn local(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .rsplit(':')
        .next()
        .unwrap_or_default()
        .into()
}
fn key(id: &str, tag: &str) -> String {
    format!("{id}\0{tag}")
}
fn correlate(
    path: &str,
    raw: Vec<FormItemTraceEvent>,
    schema: Vec<FormItemSchemaTraceEvent>,
    native: Vec<XmlItem>,
    candidate: Vec<XmlItem>,
    s: &mut FormProvenanceSummary,
    run: &str,
    commit: Option<String>,
) -> Vec<Record> {
    let mut schema_by_identity = BTreeMap::new();
    for event in schema {
        schema_by_identity.insert(
            (
                event.id.clone(),
                event.tag.clone(),
                event.name.clone(),
                event.occurrence,
            ),
            event,
        );
    }
    let (mut r, mut n, mut c) = (
        BTreeMap::<String, Vec<_>>::new(),
        BTreeMap::<String, Vec<_>>::new(),
        BTreeMap::<String, Vec<_>>::new(),
    );
    for x in raw {
        r.entry(key(&x.id, &x.tag)).or_default().push(x)
    }
    for x in native {
        n.entry(key(&x.id, &x.tag)).or_default().push(x)
    }
    for x in candidate {
        c.entry(key(&x.id, &x.tag)).or_default().push(x)
    }
    let ks: BTreeSet<_> = r.keys().chain(n.keys()).chain(c.keys()).cloned().collect();
    let mut out = Vec::new();
    for k in ks {
        let (rv, nv, cv) = (
            r.remove(&k).unwrap_or_default(),
            n.remove(&k).unwrap_or_default(),
            c.remove(&k).unwrap_or_default(),
        );
        let collision = rv.len() > 1 || nv.len() > 1 || cv.len() > 1;
        if rv.len() > 1 {
            s.raw_collisions += rv.len() - 1
        } else if rv.len() == 1 {
            s.raw_unique += 1
        }
        if nv.len() > 1 {
            s.xml_collisions += nv.len() - 1
        }
        if cv.len() > 1 {
            s.xml_collisions += cv.len() - 1
        }
        if collision {
            for (i, x) in rv.into_iter().enumerate() {
                out.push(Record {
                    schema_version: 1,
                    source_commit: commit.clone(),
                    run_id: run.into(),
                    path: path.into(),
                    key: k.clone(),
                    occurrence: i,
                    raw: Some(x),
                    schema: None,
                    native: None,
                    candidate: None,
                    state: "collision_raw".into(),
                })
            }
            for (i, x) in nv.into_iter().enumerate() {
                out.push(Record {
                    schema_version: 1,
                    source_commit: commit.clone(),
                    run_id: run.into(),
                    path: path.into(),
                    key: k.clone(),
                    occurrence: i,
                    raw: None,
                    schema: None,
                    native: Some(x),
                    candidate: None,
                    state: "collision_native".into(),
                })
            }
            for (i, x) in cv.into_iter().enumerate() {
                out.push(Record {
                    schema_version: 1,
                    source_commit: commit.clone(),
                    run_id: run.into(),
                    path: path.into(),
                    key: k.clone(),
                    occurrence: i,
                    raw: None,
                    schema: None,
                    native: None,
                    candidate: Some(x),
                    state: "collision_candidate".into(),
                })
            }
            continue;
        }
        let (a, b, d) = (rv.first(), nv.first(), cv.first());
        let state = match (a, b, d) {
            (Some(x), Some(y), Some(z)) if x.name == y.name && x.name == z.name => {
                s.matched_both += 1;
                "matched_both"
            }
            (Some(_), Some(_), Some(_)) => {
                s.name_mismatch += 1;
                "name_mismatch"
            }
            (Some(_), None, Some(_)) => {
                s.missing_native += 1;
                "missing_native"
            }
            (Some(_), Some(_), None) => {
                s.missing_candidate += 1;
                "missing_candidate"
            }
            (Some(_), None, None) => {
                s.missing_both += 1;
                "raw_only"
            }
            (None, Some(_), Some(_)) => {
                s.xml_only_both += 1;
                "xml_only_both"
            }
            (None, Some(_), None) => {
                s.native_only += 1;
                "native_only"
            }
            (None, None, Some(_)) => {
                s.candidate_only += 1;
                "candidate_only"
            }
            _ => "collision",
        };
        let schema = a.and_then(|event| {
            schema_by_identity.remove(&(
                event.id.clone(),
                event.tag.clone(),
                event.name.clone(),
                event.occurrence,
            ))
        });
        out.push(Record {
            schema_version: 1,
            source_commit: commit.clone(),
            run_id: run.into(),
            path: path.into(),
            key: k,
            occurrence: 0,
            raw: a.cloned(),
            schema,
            native: b.cloned(),
            candidate: d.cloned(),
            state: state.into(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp(xml: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ibcmd-provenance-{}.xml", uuid::Uuid::new_v4()));
        fs::write(&p, xml).unwrap();
        p
    }
    fn summary() -> FormProvenanceSummary {
        FormProvenanceSummary {
            schema_version: 1,
            source_commit: None,
            run_id: "t".into(),
            input_forms: 0,
            read_forms: 0,
            parsed_forms: 0,
            errors: 0,
            reasons: BTreeMap::new(),
            raw_events: 0,
            raw_unique: 0,
            raw_collisions: 0,
            matched_both: 0,
            name_mismatch: 0,
            missing_native: 0,
            missing_candidate: 0,
            missing_both: 0,
            native_only: 0,
            candidate_only: 0,
            xml_only_both: 0,
            xml_collisions: 0,
            denominator: 0,
            rate_percent: 0.,
            output: String::new(),
        }
    }
    fn raw(id: &str, name: &str) -> FormItemTraceEvent {
        FormItemTraceEvent {
            id: id.into(),
            tag: "Table".into(),
            name: name.into(),
            occurrence: 0,
            wrapper: "55".into(),
            raw_field_count: 1,
            normalized_field_count: 1,
            owner_chain: Vec::new(),
            top_level_scalars: Vec::new(),
        }
    }
    fn xml(id: &str, name: &str) -> XmlItem {
        XmlItem {
            id: id.into(),
            tag: "Table".into(),
            name: name.into(),
            owner_chain: Vec::new(),
            properties: BTreeMap::new(),
        }
    }
    #[test]
    fn xml_direct_properties_capture_text_empty_nil_cdata_and_order() {
        let p = temp(
            r#"<Form><Table id="1" name="T"><X/><DataPath>a<![CDATA[b]]></DataPath><AutoMaxWidth xsi:nil="true" xmlns:xsi="x"/><Holder><DataPath>bad</DataPath></Holder><TitleDataPath/><Button id="2" name="B"><DataPath>nested</DataPath></Button><Button id="3" name="E"/></Table></Form>"#,
        );
        let x = parse_xml_items(
            &p,
            &DEFAULT_PROPERTIES
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let outer = x.iter().find(|item| item.id == "1").unwrap();
        let q = &outer.properties;
        assert_eq!(q["DataPath"].value.as_deref(), Some("ab"));
        assert_eq!(q["DataPath"].order, 1);
        assert!(q["AutoMaxWidth"].nil);
        assert_eq!(q["AutoMaxWidth"].order, 2);
        assert!(q["TitleDataPath"].empty);
        assert_eq!(q["TitleDataPath"].order, 4);
        assert_ne!(q["DataPath"].value.as_deref(), Some("bad"));
        let nested = x.iter().find(|item| item.id == "2").unwrap();
        assert_eq!(nested.tag, "Button");
        assert_eq!(nested.owner_chain, vec!["Table:1"]);
        assert_eq!(nested.properties["DataPath"].order, 0);
        let empty = x.iter().find(|item| item.id == "3").unwrap();
        assert_eq!(empty.tag, "Button");
        assert_eq!(empty.owner_chain, vec!["Table:1"]);
        let _ = fs::remove_file(p);
    }
    #[test]
    fn correlation_is_fail_closed_and_states_are_explicit() {
        let mut s = summary();
        let a = correlate(
            "p",
            vec![raw("1", "A"), raw("1", "A")],
            vec![],
            vec![xml("1", "A"), xml("1", "A")],
            vec![xml("1", "A"), xml("1", "A")],
            &mut s,
            "r",
            None,
        );
        assert!(a.iter().all(|x| x.state.starts_with("collision_")));
        assert!(a.iter().all(|x| {
            usize::from(x.raw.is_some())
                + usize::from(x.native.is_some())
                + usize::from(x.candidate.is_some())
                == 1
        }));
        assert_eq!(a.iter().filter(|x| x.state == "collision_raw").count(), 2);
        assert_eq!(
            a.iter().filter(|x| x.state == "collision_native").count(),
            2
        );
        assert_eq!(
            a.iter()
                .filter(|x| x.state == "collision_candidate")
                .count(),
            2
        );
        assert_eq!(s.matched_both, 0);
        assert_eq!(s.raw_collisions, 1);
        assert_eq!(s.xml_collisions, 2);
        let mut s = summary();
        let r = correlate(
            "p",
            vec![
                raw("1", "A"),
                raw("2", "B"),
                raw("3", "C"),
                raw("4", "D"),
                raw("6", "F"),
            ],
            vec![],
            vec![
                xml("1", "A"),
                xml("2", "B"),
                xml("4", "D"),
                xml("5", "E"),
                xml("7", "G"),
            ],
            vec![
                xml("1", "A"),
                xml("3", "C"),
                xml("5", "E"),
                xml("4", "X"),
                xml("8", "H"),
            ],
            &mut s,
            "r",
            None,
        );
        let states = r.iter().map(|x| x.state.as_str()).collect::<BTreeSet<_>>();
        for x in [
            "matched_both",
            "missing_candidate",
            "missing_native",
            "name_mismatch",
            "xml_only_both",
            "raw_only",
            "native_only",
            "candidate_only",
        ] {
            assert!(states.contains(x), "{x}")
        }
        assert_eq!(s.matched_both, 1);
        assert_eq!(s.missing_candidate, 1);
        assert_eq!(s.missing_native, 1);
        assert_eq!(s.name_mismatch, 1);
        assert_eq!(s.xml_only_both, 1);
        assert_eq!(s.missing_both, 1);
        assert_eq!(s.native_only, 1);
        assert_eq!(s.candidate_only, 1);
    }
    #[test]
    fn manifest_filters_are_safe_unicode_and_deterministic() {
        assert!(filters(&["AutoMaxWidth".into()]).is_ok());
        assert!(filters(&["Nope".into()]).is_err());
        assert!(safe("Catalogs/Тест/Forms/F/Ext/Form.xml"));
        assert!(!safe("../x"));
        assert!(!safe("C:/x"));
        assert!(manifest_rows(&serde_json::json!({})).is_err());
        assert!(manifest_rows(&serde_json::json!({"tables":[{}]})).is_err());
        assert!(manifest_rows(&serde_json::json!({"tables":[{"rows":[{"source_asset_path":"Catalogs/X/Forms/F/Ext/Form.xml","inflated_path":"Config_inflated/a.txt"}]}]})).is_err());
        let v: Value = serde_json::json!({"tables":[{"rows":[{"source_asset_path":"Catalogs/Тест/Forms/F/Ext/Form.xml","inflated_path":"Config_inflated/a.txt","file_name":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.0"}]}]});
        assert_eq!(
            manifest_rows(&v).unwrap()[0].0,
            "Catalogs/Тест/Forms/F/Ext/Form.xml"
        );
        assert_eq!(
            manifest_rows(&v).unwrap()[0].2,
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.0"
        );
        let mut s = summary();
        let r = correlate(
            "z",
            vec![raw("2", "B"), raw("1", "A")],
            vec![],
            vec![],
            vec![],
            &mut s,
            "r",
            None,
        );
        let one = serialize_records(r.clone(), Vec::new()).unwrap();
        let mut permuted = r;
        permuted.reverse();
        let two = serialize_records(permuted, Vec::new()).unwrap();
        assert_eq!(one, two);
        let mut e = Vec::new();
        err(&mut s, &mut e, "z", "raw_trace", "bad");
        assert_eq!(e[0].schema_version, 1);
        assert_eq!(e[0].source_commit, None);
        assert_eq!(e[0].run_id, "t");
        assert_eq!(e[0].path, "z");
        assert_eq!(e[0].reason, "raw_trace");
        assert_eq!(e[0].message, "bad");
        let malformed = temp("<Form><Table id=\"1\" name=\"T\"></Form>");
        assert!(parse_xml_items(&malformed, &[]).is_err());
        let _ = fs::remove_file(malformed);
        assert_eq!(e[0].stage, "raw_trace");
    }
    #[test]
    fn manifest_form_id_is_taken_exactly_from_file_name() {
        assert_eq!(
            form_id_from_manifest_file_name("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.0").unwrap(),
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
        );
        for file_name in [
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.1",
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.0__part0.txt",
            "../aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.0",
            "not-a-uuid.0",
        ] {
            assert!(
                form_id_from_manifest_file_name(file_name).is_err(),
                "{file_name}"
            );
        }
    }
    #[test]
    fn manifest_owner_is_required_except_for_common_forms() {
        let id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let mut owners = BTreeMap::new();
        owners.insert(id.into(), "Catalog.Test".into());
        assert_eq!(
            form_owner_for_manifest_path("Catalogs/Test/Forms/F/Ext/Form.xml", id, &owners)
                .unwrap(),
            Some("Catalog.Test")
        );
        assert_eq!(
            form_owner_for_manifest_path("CommonForms/F/Ext/Form.xml", id, &BTreeMap::new())
                .unwrap(),
            None
        );
        let missing = form_owner_for_manifest_path(
            "Catalogs/Test/Forms/F/Ext/Form.xml",
            id,
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert!(missing.to_string().contains("missing owner reference"));
    }
}
