//! Exact body roles and owned-object bindings for the measured dynamic cohort.
//! Descriptor/property changes are still judged separately against effective Config.

use std::collections::HashMap;

use anyhow::{Result, anyhow, ensure};

use crate::compiler::bodies::template::{TemplateKind, decode_compatible_template};
use crate::compiler::families::assets::{SourceAssetRegistry, SourceAssetRole};
use crate::compiler::families::native::{NativeValue, parse_optional_bom};
use crate::metadata_model::brace::Brace;
use crate::metadata_model::export::names::own_header;

/// Independent decoding bounds: a small compressed row must not inflate without limit.
pub(super) const MAX_PLAIN_ROW: usize = 8 * 1024 * 1024;
pub(super) const MAX_GRAPH_ROWS: usize = 2048;
pub(super) const MAX_GRAPH_BYTES: usize = 32 * 1024 * 1024;

/// Object/Manager roles actually present in the native matrix. Registry membership by
/// itself is not admission: RecordSet, ValueManager and structural assets remain out.
const MODULE_KINDS: &[&str] = &[
    "CommonModule",
    "Catalog",
    "Document",
    "Report",
    "DataProcessor",
    "ExchangePlan",
    "Task",
    "BusinessProcess",
    "ChartOfAccounts",
    "ChartOfCalculationTypes",
    "ChartOfCharacteristicTypes",
    "Enum",
    "Constant",
    "SettingsStorage",
    "DocumentJournal",
    "InformationRegister",
    "AccumulationRegister",
];

const FORM_PARENTS: &[&str] = &[
    "Catalog",
    "Document",
    "Report",
    "DataProcessor",
    "ExchangePlan",
    "Task",
    "BusinessProcess",
    "ChartOfAccounts",
    "ChartOfCalculationTypes",
    "ChartOfCharacteristicTypes",
    "SettingsStorage",
    "DocumentJournal",
    "InformationRegister",
    "AccumulationRegister",
    "AccountingRegister",
    "CalculationRegister",
];

// Functional old/new native-client controls are required independently of
// offline descriptor/codec fixtures before a nested Form family is published.
const MEASURED_FORM_PARENTS: &[&str] = &[];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BodyRole {
    CommonModule,
    Module,
    Form,
    Template(TemplateKind),
    UnchangedHelp,
}

#[derive(Clone, Debug)]
struct Owned {
    parent: String,
    kind: &'static str,
    template: Option<TemplateKind>,
    validated: bool,
}

#[derive(Default)]
pub(super) struct Owners {
    pub(super) kinds: HashMap<String, &'static str>,
    owned: HashMap<String, Owned>,
}

impl From<HashMap<String, &'static str>> for Owners {
    fn from(kinds: HashMap<String, &'static str>) -> Self {
        Self {
            kinds,
            owned: HashMap::new(),
        }
    }
}

impl Owners {
    pub(super) fn may_own_bodies(kind: &str) -> bool {
        FORM_PARENTS.contains(&kind)
    }

    /// Bind each relevant nested UUID exactly once, from its parent's immutable descriptor.
    pub(super) fn bind_children(&mut self, parent: &str, kind: &str, row: &Brace) -> Result<()> {
        ensure!(Self::may_own_bodies(kind), "unmeasured parent kind {kind}");
        ensure!(
            own_header(row).is_some_and(|(uuid, _)| uuid == parent),
            "owner descriptor identity mismatch"
        );
        let classes = crate::metadata_model::objects::owned_body_classes(kind)
            .ok_or_else(|| anyhow!("parent has no typed body collections"))?;
        let all_classes: HashMap<_, _> = FORM_PARENTS
            .iter()
            .filter_map(|kind| crate::metadata_model::objects::owned_body_classes(kind))
            .flatten()
            .collect();
        let expected: HashMap<_, _> = classes.into_iter().collect();
        let mut groups = std::collections::HashSet::new();
        let mut children = Vec::new();
        collect_children(row, &expected, &all_classes, &mut groups, &mut children)?;
        ensure!(
            groups.len() == expected.len(),
            "missing owned-body collection"
        );
        let mut seen = std::collections::HashSet::new();
        for (_, uuid) in &children {
            ensure!(
                uuid::Uuid::parse_str(uuid)?.hyphenated().to_string() == *uuid,
                "noncanonical owned UUID"
            );
            ensure!(
                !self.kinds.contains_key(uuid)
                    && !self.owned.contains_key(uuid)
                    && seen.insert(uuid.clone()),
                "duplicate or ambiguous owned UUID {uuid}"
            );
        }
        ensure!(
            self.owned
                .len()
                .checked_add(children.len())
                .is_some_and(|n| n <= MAX_GRAPH_ROWS),
            "owned-body graph exceeds row budget"
        );
        // Publish bindings only after the complete descriptor passes validation.
        for (child_kind, uuid) in children {
            self.owned.insert(
                uuid.clone(),
                Owned {
                    parent: parent.to_owned(),
                    kind: child_kind,
                    template: None,
                    validated: false,
                },
            );
            self.kinds.insert(uuid, child_kind);
        }
        Ok(())
    }

    pub(super) fn bind_descriptor(&mut self, uuid: &str, row: &Brace) -> Result<()> {
        let owned = self
            .owned
            .get_mut(uuid)
            .ok_or_else(|| anyhow!("no proven parent for {uuid}"))?;
        ensure!(
            own_header(row).is_some_and(|(id, _)| id == uuid),
            "owned descriptor identity mismatch"
        );
        let outer = exact_list(row, 3)?;
        ensure!(
            outer[0].as_atom() == Some("1") && outer[2].as_atom() == Some("0"),
            "unknown owned descriptor root"
        );
        let parent_kind = self
            .kinds
            .get(&owned.parent)
            .ok_or_else(|| anyhow!("parent kind missing"))?;
        match owned.kind {
            "Form" => {
                let payload = &outer[1];
                let record = match *parent_kind {
                    "Task"
                    | "BusinessProcess"
                    | "ChartOfAccounts"
                    | "ChartOfCharacteristicTypes"
                    | "AccountingRegister" => payload,
                    "Report" | "DataProcessor" => {
                        let wrapper = exact_list(payload, 2)?;
                        ensure!(
                            wrapper[0].as_atom() == Some("1"),
                            "unknown presentation form wrapper"
                        );
                        let inner = exact_list(&wrapper[1], 3)?;
                        ensure!(inner[0].as_atom() == Some("0"), "unknown form wrapper");
                        &inner[1]
                    }
                    _ => {
                        let wrapper = exact_list(payload, 2)?;
                        ensure!(
                            wrapper[0].as_atom() == Some("0"),
                            "unknown versioned form wrapper"
                        );
                        &wrapper[1]
                    }
                };
                let fields = exact_list(record, 5)?;
                ensure!(
                    fields[0].as_atom() == Some("13") && fields[3].as_atom() == Some("1"),
                    "only measured managed form record 13 is dynamic"
                );
            }
            "Template" => {
                let fields = exact_list(&outer[1], 3)?;
                ensure!(
                    fields[0].as_atom() == Some("2"),
                    "unknown template descriptor"
                );
                owned.template = Some(match (*parent_kind, fields[1].as_atom()) {
                    ("Report", Some("0")) => TemplateKind::SpreadsheetDocument,
                    ("DataProcessor", Some("3")) => TemplateKind::HtmlDocument,
                    ("ExchangePlan", Some("4")) => TemplateKind::TextDocument,
                    _ => return Err(anyhow!("unmeasured template type")),
                });
            }
            _ => return Err(anyhow!("unknown owned kind")),
        }
        owned.validated = true;
        Ok(())
    }

    pub(super) fn admits(&self, uuid: &str) -> bool {
        match self.kinds.get(uuid).copied() {
            Some("CommonForm") => true,
            Some("Form") => self.owned.get(uuid).is_some_and(|o| {
                o.validated
                    && self
                        .kinds
                        .get(&o.parent)
                        .is_some_and(|kind| MEASURED_FORM_PARENTS.contains(kind))
            }),
            Some("Template") => self
                .owned
                .get(uuid)
                .is_some_and(|o| o.validated && o.template.is_some()),
            Some(kind) => MODULE_KINDS.contains(&kind),
            None => false,
        }
    }

    pub(super) fn role(&self, uuid: &str, suffix: &str) -> Option<BodyRole> {
        let kind = *self.kinds.get(uuid)?;
        if matches!(kind, "CommonForm" | "Form") && self.admits(uuid) {
            return match suffix {
                "0" => Some(BodyRole::Form),
                "1" => Some(BodyRole::UnchangedHelp),
                _ => None,
            };
        }
        if kind == "Template" && suffix == "0" {
            return self.owned.get(uuid)?.template.map(BodyRole::Template);
        }
        if !MODULE_KINDS.contains(&kind) {
            return None;
        }
        let route = SourceAssetRegistry.route_by_suffix(kind, suffix)?;
        match route.role() {
            SourceAssetRole::Module if kind == "CommonModule" => Some(BodyRole::CommonModule),
            SourceAssetRole::ObjectModule | SourceAssetRole::ManagerModule => {
                Some(BodyRole::Module)
            }
            _ => None,
        }
    }
}

/// Unlike the export name index, admission must not collapse ambiguous identities
/// or silently skip a malformed recognized collection.
pub(super) fn root_owners(row: &Brace) -> Result<Owners> {
    fn visit(
        row: &Brace,
        classes: &HashMap<&str, &'static str>,
        groups: &mut std::collections::HashSet<String>,
        kinds: &mut HashMap<String, &'static str>,
    ) -> Result<()> {
        let Some(items) = row.as_list() else {
            return Ok(());
        };
        if let Some(class) = items.first().and_then(Brace::as_atom) {
            if let Some(kind) = classes.get(class) {
                ensure!(groups.insert(class.to_owned()), "duplicate root collection");
                let count = items
                    .get(1)
                    .and_then(Brace::as_atom)
                    .and_then(|n| n.parse::<usize>().ok())
                    .ok_or_else(|| anyhow!("invalid root collection count"))?;
                ensure!(
                    count.checked_add(2) == Some(items.len()),
                    "inconsistent root collection count"
                );
                for item in &items[2..] {
                    let id = item
                        .as_atom()
                        .ok_or_else(|| anyhow!("root owner UUID is not an atom"))?;
                    ensure!(
                        uuid::Uuid::parse_str(id)?.hyphenated().to_string() == id,
                        "noncanonical root owner UUID"
                    );
                    ensure!(
                        kinds.insert(id.to_owned(), kind).is_none(),
                        "duplicate or ambiguous root owner UUID"
                    );
                }
                return Ok(());
            }
        }
        for item in items {
            visit(item, classes, groups, kinds)?;
        }
        Ok(())
    }
    let classes = crate::metadata_model::export::names::root_class_kinds()
        .iter()
        .copied()
        .collect();
    let mut owners = Owners::default();
    visit(
        row,
        &classes,
        &mut std::collections::HashSet::new(),
        &mut owners.kinds,
    )?;
    Ok(owners)
}

fn collect_children(
    row: &Brace,
    expected: &HashMap<&str, &'static str>,
    known: &HashMap<&str, &'static str>,
    groups: &mut std::collections::HashSet<String>,
    children: &mut Vec<(&'static str, String)>,
) -> Result<()> {
    let Some(items) = row.as_list() else {
        return Ok(());
    };
    if let Some(class) = items.first().and_then(Brace::as_atom) {
        if known.contains_key(class) {
            let kind = *expected
                .get(class)
                .ok_or_else(|| anyhow!("foreign owned-body collection"))?;
            ensure!(
                groups.insert(class.to_owned()),
                "duplicate owned-body collection"
            );
            let count = items
                .get(1)
                .and_then(Brace::as_atom)
                .and_then(|n| n.parse::<usize>().ok())
                .ok_or_else(|| anyhow!("invalid owned-body collection count"))?;
            ensure!(
                count <= MAX_GRAPH_ROWS && count.checked_add(2) == Some(items.len()),
                "inconsistent owned-body collection count"
            );
            for child in &items[2..] {
                children.push((
                    kind,
                    child
                        .as_atom()
                        .ok_or_else(|| anyhow!("owned UUID is not an atom"))?
                        .to_owned(),
                ));
            }
            return Ok(());
        }
    }
    for item in items {
        collect_children(item, expected, known, groups, children)?;
    }
    Ok(())
}

fn exact_list(row: &Brace, size: usize) -> Result<&[Brace]> {
    row.as_list()
        .filter(|items| items.len() == size)
        .ok_or_else(|| anyhow!("unknown descriptor shape"))
}

pub(super) fn inflate(blob: &[u8]) -> Result<Vec<u8>> {
    inflate_limit(blob, MAX_PLAIN_ROW)
}

/// A graph caller may have less than one row's budget remaining.
pub(super) fn inflate_limit(blob: &[u8], limit: usize) -> Result<Vec<u8>> {
    let limit = limit.min(MAX_PLAIN_ROW);
    let mut plain = Vec::new();
    let mut decoder = flate2::Decompress::new(false);
    let mut chunk = [0u8; 8192];
    loop {
        let before_in = decoder.total_in();
        let before_out = decoder.total_out();
        let capacity = chunk.len().min(limit + 1 - plain.len());
        let status = decoder.decompress(
            &blob[before_in as usize..],
            &mut chunk[..capacity],
            flate2::FlushDecompress::None,
        )?;
        let produced = (decoder.total_out() - before_out) as usize;
        plain.extend_from_slice(&chunk[..produced]);
        ensure!(
            plain.len() <= limit,
            "dynamic decoded row exceeds {limit} bytes"
        );
        if status == flate2::Status::StreamEnd {
            ensure!(
                decoder.total_in() == blob.len() as u64,
                "dynamic row has trailing compressed data"
            );
            break;
        }
        ensure!(
            decoder.total_in() != before_in || produced != 0,
            "dynamic row has incomplete DEFLATE stream"
        );
    }
    Ok(plain)
}

/// The bounded native parser validates depth/UTF-8 before ownership traversal recurses.
pub(super) fn descriptor(blob: &[u8]) -> Result<(Brace, usize)> {
    let plain = inflate(blob)?;
    Ok((descriptor_plain(&plain)?, plain.len()))
}

pub(super) fn descriptor_plain(plain: &[u8]) -> Result<Brace> {
    fn brace(value: NativeValue) -> Brace {
        match value {
            NativeValue::Token(value) => Brace::Atom(value),
            NativeValue::Text(value) => Brace::Str(value),
            NativeValue::List { values, .. } => {
                Brace::List(values.into_iter().map(brace).collect())
            }
        }
    }
    Ok(brace(parse_optional_bom(plain)?))
}

pub(super) fn validate_body(role: BodyRole, blob: &[u8]) -> Result<()> {
    let plain = inflate(blob)?;
    match role {
        BodyRole::CommonModule if plain.starts_with(b"\xef\xbb\xbf") => {
            let text = std::str::from_utf8(&plain[3..])?;
            ensure!(
                !text.trim_start().starts_with(['<', '{', '[']),
                "plain common module is a non-BSL wrapper"
            );
        }
        BodyRole::CommonModule | BodyRole::Module => {
            let elements = crate::v8_container::parse_v8_container(&plain)?;
            ensure!(
                elements.len() == 2
                    && elements.iter().filter(|e| e.name == "text").count() == 1
                    && elements.iter().filter(|e| e.name == "info").count() == 1,
                "unmeasured module container"
            );
            ensure!(
                elements
                    .iter()
                    .find(|e| e.name == "info")
                    .expect("checked info")
                    .data
                    == b"\xef\xbb\xbf{3,1,0,\"\",0}",
                "unmeasured module info record"
            );
            let text = &elements
                .iter()
                .find(|e| e.name == "text")
                .expect("checked text")
                .data;
            let source = text
                .strip_prefix(b"\xef\xbb\xbf")
                .ok_or_else(|| anyhow!("module text has no UTF-8 BOM"))?;
            std::str::from_utf8(source)?;
        }
        BodyRole::Form => {
            ensure!(plain.starts_with(b"\xef\xbb\xbf"), "form body has no BOM");
            let body = crate::compiler::bodies::form::decode_compatible_managed_form(blob)?;
            let parsed = body.parsed();
            ensure!(
                parsed
                    .layout
                    .trim()
                    .strip_prefix('{')
                    .and_then(|s| s.split(',').next())
                    .map(str::trim)
                    == Some("50")
                    && parsed.revision == crate::module_blob::FormBodyRevision::V4
                    && parsed.trailing_fields == 7
                    && parsed.trailing.len() == 7,
                "unmeasured managed form body layout"
            );
            ensure!(
                parsed.trailing[5..] == ["0", "0"],
                "unmeasured managed form tail scalars"
            );
            for (index, section) in parsed.trailing[..5].iter().enumerate() {
                let tree = parse_optional_bom(section.as_bytes())?;
                let fields = tree
                    .as_list()
                    .ok_or_else(|| anyhow!("form section is not a list"))?;
                ensure!(
                    fields.first().and_then(NativeValue::as_token)
                        == Some(if index == 0 { "4" } else { "0" }),
                    "unmeasured form section marker"
                );
                let count = fields
                    .get(1)
                    .and_then(NativeValue::as_token)
                    .and_then(|n| n.parse::<usize>().ok())
                    .ok_or_else(|| anyhow!("invalid form section count"))?;
                ensure!(
                    count.checked_add(if index == 0 { 5 } else { 2 }) == Some(fields.len()),
                    "inconsistent form section count"
                );
                if index == 0 {
                    ensure!(
                        fields.get(fields.len() - 3).and_then(NativeValue::as_token) == Some("0")
                            && fields.get(fields.len() - 2).and_then(NativeValue::as_token)
                                == Some("0")
                            && fields.last().and_then(NativeValue::as_list).is_some(),
                        "unmeasured form attribute section tail"
                    );
                }
            }
        }
        BodyRole::Template(kind) => {
            decode_compatible_template(kind, blob)?;
        }
        BodyRole::UnchangedHelp => {}
    }
    Ok(())
}

/// Keeping a prior alias is admission too: a body-only stage must not carry an
/// unmeasured descriptor/property change or help invalidation into another generation.
pub(super) fn validate_retained_alias(
    ordinary: &str,
    owners: &Owners,
    baseline: &[u8],
    alias: &[u8],
) -> Result<()> {
    match super::model::classify_name(ordinary) {
        super::model::RowName::Descriptor(owner) => {
            ensure!(owners.admits(owner), "unmeasured retained descriptor owner");
            let (base, _) = descriptor(baseline)?;
            let (pending, _) = descriptor(alias)?;
            ensure!(base == pending, "retained descriptor/property change");
        }
        super::model::RowName::Body { owner, suffix } => {
            ensure!(owners.admits(owner), "unmeasured retained body owner");
            let role = owners
                .role(owner, suffix)
                .ok_or_else(|| anyhow!("unmeasured retained body role"))?;
            if role == BodyRole::UnchangedHelp {
                ensure!(
                    inflate(baseline)? == inflate(alias)?,
                    "retained help changed outside the body-only cohort"
                );
            } else {
                validate_body(role, baseline)?;
                validate_body(role, alias)?;
            }
        }
        _ => return Err(anyhow!("unmeasured retained metadata alias")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    fn raw(name: &str) -> Vec<u8> {
        std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/dynamic-metadata-native")
                .join(format!("{name}.bin")),
        )
        .unwrap()
    }

    fn deflate(plain: &[u8]) -> Vec<u8> {
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(plain).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn native_matrix_binds_exact_owners_and_decodes_all_measured_body_families() {
        let manifest: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/dynamic-metadata-native/manifest.json"
        ))
        .unwrap();
        for case in manifest["cases"].as_array().unwrap() {
            let parent = case["parent"].as_str().unwrap();
            let owner = case["owner"].as_str().unwrap();
            let kind = case["kind"].as_str().unwrap();
            let kind = MODULE_KINDS
                .iter()
                .chain(FORM_PARENTS)
                .find(|k| **k == kind)
                .unwrap();
            let mut owners: Owners = HashMap::from([(parent.to_owned(), *kind)]).into();
            if parent != owner {
                owners
                    .bind_children(parent, kind, &descriptor(&raw(parent)).unwrap().0)
                    .unwrap();
                assert!(!owners.admits(owner), "unvalidated {owner}");
                owners
                    .bind_descriptor(owner, &descriptor(&raw(owner)).unwrap().0)
                    .unwrap();
            }
            if case["family"] == "forms" {
                assert!(owners.owned.get(owner).is_some_and(|o| o.validated));
                assert_eq!(
                    owners.admits(owner),
                    MEASURED_FORM_PARENTS.contains(kind),
                    "functional admission separate from fixture codec proof"
                );
            } else {
                assert!(owners.admits(owner), "{case}");
            }
            let row = case["row"].as_str().unwrap();
            let suffix = row.rsplit_once('.').unwrap().1;
            let role = if case["family"] == "forms" {
                if owners.admits(owner) {
                    assert_eq!(owners.role(owner, suffix), Some(BodyRole::Form));
                } else {
                    assert!(owners.role(owner, suffix).is_none());
                }
                BodyRole::Form
            } else {
                owners.role(owner, suffix).unwrap()
            };
            match case["family"].as_str().unwrap() {
                "modules" => assert_eq!(role, BodyRole::Module),
                "forms" => {
                    assert_eq!(role, BodyRole::Form);
                    assert_eq!(
                        owners.role(owner, "1"),
                        owners.admits(owner).then_some(BodyRole::UnchangedHelp)
                    );
                }
                "templates" => assert!(matches!(role, BodyRole::Template(_))),
                _ => unreachable!(),
            }
            validate_body(role, &raw(row)).unwrap_or_else(|error| panic!("{case}: {error:#}"));
            assert!(owners.role(owner, "unknown").is_none());
        }
    }

    fn header(id: &str) -> Brace {
        crate::brace_list![
            Brace::num(3),
            crate::brace_list![Brace::num(1), Brace::num(0), Brace::atom(id)],
            Brace::str("fixture"),
            Brace::num(0),
            Brace::str(""),
            Brace::num(0),
            Brace::num(0),
            Brace::num(0),
            Brace::num(0)
        ]
    }
    fn collection(class: &str, count: &str, ids: &[&str]) -> Brace {
        let mut items = vec![Brace::atom(class), Brace::atom(count)];
        items.extend(ids.iter().map(|id| Brace::atom(id)));
        Brace::List(items)
    }
    const PARENT: &str = "00000001-0002-0003-0004-000000000005";
    const CHILD: &str = "aaaaaa11-0012-0013-0014-000000000015";
    const OTHER: &str = "00000021-0022-0023-0024-000000000025";
    fn parent(groups: Vec<Brace>) -> Brace {
        let mut row = vec![
            Brace::num(1),
            header(PARENT),
            Brace::num(groups.len() as i64),
        ];
        row.extend(groups);
        Brace::List(row)
    }
    fn catalog_groups() -> Vec<Brace> {
        crate::metadata_model::objects::owned_body_classes("Catalog")
            .unwrap()
            .iter()
            .map(|(class, kind)| {
                collection(
                    class,
                    if *kind == "Form" { "1" } else { "0" },
                    if *kind == "Form" { &[CHILD] } else { &[] },
                )
            })
            .collect()
    }
    #[test]
    fn ownership_refuses_foreign_unknown_count_duplicate_and_reparented_collections_atomically() {
        let groups = catalog_groups();
        let mut owners: Owners = HashMap::from([
            (PARENT.to_owned(), "Catalog"),
            (OTHER.to_owned(), "Catalog"),
        ])
        .into();
        owners
            .bind_children(PARENT, "Catalog", &parent(groups.clone()))
            .unwrap();
        assert_eq!(owners.kinds.get(CHILD), Some(&"Form"));
        assert!(!owners.admits(CHILD));
        let mut reparent = parent(groups.clone());
        // Exact own identity is independently checked before collection traversal.
        reparent.as_list_mut().unwrap()[1] = header(OTHER);
        assert!(owners.bind_children(OTHER, "Catalog", &reparent).is_err());
        for variant in 0..8 {
            let mut bad = groups.clone();
            let index = bad
                .iter()
                .position(|g| g.as_list().unwrap().len() > 2)
                .unwrap();
            match variant {
                0 => {
                    bad[index].as_list_mut().unwrap()[0] = Brace::atom(
                        crate::metadata_model::objects::owned_body_classes("Document")
                            .unwrap()
                            .iter()
                            .find(|(_, kind)| *kind == "Form")
                            .unwrap()
                            .0,
                    )
                }
                1 => bad[index].as_list_mut().unwrap()[0] = Brace::atom(OTHER),
                2 => bad[index].as_list_mut().unwrap()[1] = Brace::num(2),
                3 => bad[index].as_list_mut().unwrap()[1] = Brace::atom("-1"),
                4 => bad.push(bad[index].clone()),
                5 => {
                    bad[index].as_list_mut().unwrap().push(Brace::atom(CHILD));
                    bad[index].as_list_mut().unwrap()[1] = Brace::num(2);
                }
                6 => bad[index].as_list_mut().unwrap()[2] = Brace::str(CHILD),
                7 => bad[index].as_list_mut().unwrap()[2] = Brace::atom(CHILD.to_uppercase()),
                _ => unreachable!(),
            }
            let mut fresh: Owners = HashMap::from([(PARENT.to_owned(), "Catalog")]).into();
            assert!(
                fresh
                    .bind_children(PARENT, "Catalog", &parent(bad))
                    .is_err(),
                "variant {variant}"
            );
            assert_eq!(fresh.kinds.len(), 1, "no partial binding");
        }
        let mut collision: Owners = HashMap::from([
            (PARENT.to_owned(), "Catalog"),
            (CHILD.to_owned(), "CommonModule"),
        ])
        .into();
        assert!(
            collision
                .bind_children(PARENT, "Catalog", &parent(groups))
                .is_err()
        );
    }

    #[test]
    fn root_binding_refuses_duplicate_malformed_noncanonical_and_cross_kind_ids() {
        let classes = crate::metadata_model::export::names::root_class_kinds();
        let row = collection(classes[0].0, "1", &[CHILD]);
        assert_eq!(
            root_owners(&row).unwrap().kinds.get(CHILD),
            Some(&classes[0].1)
        );
        for bad in [
            collection(classes[0].0, "-1", &[CHILD]),
            collection(classes[0].0, "2", &[CHILD]),
            collection(classes[0].0, "18446744073709551615", &[]),
            collection(classes[0].0, "2", &[CHILD, CHILD]),
            crate::brace_list![row.clone(), row.clone()],
            crate::brace_list![row.clone(), collection(classes[1].0, "1", &[CHILD])],
        ] {
            assert!(root_owners(&bad).is_err());
        }
    }

    #[test]
    fn retained_aliases_refuse_property_help_unknown_role_and_unmeasured_codec_changes() {
        let owners: Owners = HashMap::from([
            (PARENT.to_owned(), "CommonModule"),
            (CHILD.to_owned(), "CommonForm"),
            (OTHER.to_owned(), "AccountingRegister"),
        ])
        .into();
        let old_descriptor = deflate(b"\xef\xbb\xbf{1,\"old property\"}");
        let new_descriptor = deflate(b"\xef\xbb\xbf{1,\"new property\"}");
        assert!(validate_retained_alias(PARENT, &owners, &old_descriptor, &old_descriptor).is_ok());
        assert!(
            validate_retained_alias(PARENT, &owners, &old_descriptor, &new_descriptor).is_err()
        );
        let old_body = deflate(b"\xef\xbb\xbfFunction Marker() Export\nReturn \"A\";\nEndFunction");
        let new_body = deflate(b"\xef\xbb\xbfFunction Marker() Export\nReturn \"B\";\nEndFunction");
        assert!(
            validate_retained_alias(&format!("{PARENT}.0"), &owners, &old_body, &new_body).is_ok()
        );
        for unknown in [
            deflate(b"no BOM"),
            deflate(b"\xef\xbb\xbf{unknown wrapper}"),
            deflate(b"\xef\xbb\xbf\xff"),
        ] {
            assert!(
                validate_retained_alias(&format!("{PARENT}.0"), &owners, &old_body, &unknown)
                    .is_err()
            );
        }
        let old_help = deflate(b"unchanged help");
        let new_help = deflate(b"changed help");
        assert!(
            validate_retained_alias(&format!("{CHILD}.1"), &owners, &old_help, &old_help).is_ok()
        );
        assert!(
            validate_retained_alias(&format!("{CHILD}.1"), &owners, &old_help, &new_help).is_err()
        );
        for row in [
            format!("{PARENT}.1"),
            format!("{PARENT}.unknown"),
            format!("{OTHER}.0"),
            "unknown".to_owned(),
        ] {
            assert!(validate_retained_alias(&row, &owners, &old_body, &new_body).is_err());
        }
    }

    #[test]
    fn all_inflated_body_paths_require_complete_bounded_deflate_without_trailing_bytes() {
        let plain = b"\xef\xbb\xbf{1,0}";
        let complete = deflate(plain);
        assert_eq!(inflate(&complete).unwrap(), plain);
        for length in 0..complete.len() {
            assert!(inflate(&complete[..length]).is_err());
        }
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(plain).unwrap();
        encoder.flush().unwrap();
        assert!(inflate(encoder.get_ref()).is_err());
        let mut trailing = complete;
        trailing.push(0);
        assert!(inflate(&trailing).is_err());
        assert!(inflate(&deflate(&vec![0; MAX_PLAIN_ROW + 1])).is_err());
    }
}
