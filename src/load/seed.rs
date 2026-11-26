//! A new external data processor or report (.epf/.erf) built from its XML
//! tree alone (issue #438): the compiled load needs a package to start from,
//! so a platform-built one is taken as the seed and given the tree's own
//! identities -- the root uuid, its contained ObjectId, its generated type and
//! value ids and its name. The load then carries the whole tree onto it: the
//! root's properties and child objects replace the seed's, the tree's forms,
//! templates and modules are added, and the seed's own are removed.
//!
//! The seeds (`src/load/seeds`) are packages 8.3.27.2214 built: copies of the
//! fixtures `tests/fixtures/external/variants/no_module/input.epf` and
//! `tests/fixtures/external/test_report/input.erf`, owned here so a fixture
//! regenerated for a test cannot change the product. Their identities are
//! those their own XML dumps; `the_seed_identities_are_the_seeds_own` holds
//! the constants to the bytes.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};

use super::compiled;
use crate::module_blob::{deflate_raw, inflate_raw};

/// One seed package and the identities its XML dump names.
struct Seed {
    bytes: &'static [u8],
    tag: &'static str,
    uuid: &'static str,
    object_id: &'static str,
    type_id: &'static str,
    value_id: &'static str,
    name: &'static str,
    /// The seed's own child objects: renamed out of the way, so a tree child
    /// of the same name is a new object, never an edit of the seed's.
    children: &'static [&'static str],
}

const PROCESSOR: Seed = Seed {
    bytes: include_bytes!("seeds/processor.epf"),
    tag: "ExternalDataProcessor",
    uuid: "dc0e1582-2d76-4880-ba01-3711da6f4448",
    object_id: "80ba9317-3805-45ad-950d-e4cab1dd642b",
    type_id: "87f78a5f-837e-4f96-995e-37ab22e02b56",
    value_id: "0e90fd7d-d3cd-4e67-a2c1-8410e4ea602e",
    name: "ТестОбработка",
    children: &["Форма", "Макет", "Строка", "Строки", "Число"],
};

const REPORT: Seed = Seed {
    bytes: include_bytes!("seeds/report.erf"),
    tag: "ExternalReport",
    uuid: "b0509899-141e-4cda-ba24-de8ed9711646",
    object_id: "3ae658fc-eb5f-4e2d-a3b9-8f021f275ff7",
    type_id: "1809b0db-0610-4885-86d0-4fcab351518e",
    value_id: "c5c17663-e8dc-4c06-9f35-d58aa343b6d2",
    name: "ТестОтчет",
    children: &["ФормаОтчета", "ОсновнаяСхемаКомпоновкиДанных"],
};

/// `copyinfo` of an object that references no configuration object.
const EMPTY_COPYINFO: &str = "\u{feff}{4,\r\n{0},\r\n{0},\r\n{0},\r\n{0,0},\r\n{0}\r\n}";

/// Kinds of configuration object a tree may name: a metadata path
/// `<Kind>.<Name>…` as a whole value, `cfg:<Kind>Ref.<Name>` in a type.
const CONFIGURATION_KINDS: [&str; 46] = [
    "Catalog",
    "Document",
    "DocumentJournal",
    "DocumentNumerator",
    "Sequence",
    "Enum",
    "ChartOfCharacteristicTypes",
    "ChartOfAccounts",
    "ChartOfCalculationTypes",
    "ExchangePlan",
    "BusinessProcess",
    "Task",
    "InformationRegister",
    "AccumulationRegister",
    "AccountingRegister",
    "CalculationRegister",
    "Constant",
    "DataProcessor",
    "Report",
    "FilterCriterion",
    "SettingsStorage",
    "DefinedType",
    "CommonForm",
    "CommonPicture",
    "CommonTemplate",
    "CommonCommand",
    "CommandGroup",
    "CommonModule",
    "CommonAttribute",
    "SessionParameter",
    "Role",
    "FunctionalOption",
    "FunctionalOptionsParameter",
    "Subsystem",
    "Language",
    "StyleItem",
    "Style",
    "XDTOPackage",
    "WebService",
    "HTTPService",
    "WSReference",
    "EventSubscription",
    "ScheduledJob",
    "ExternalDataSource",
    "IntegrationService",
    "Bot",
];

/// The end of a dotted identifier starting at `start`.
fn identifier_end(text: &str, start: usize) -> usize {
    text[start..]
        .find(|c: char| !(c.is_alphanumeric() || c == '.' || c == '_'))
        .map_or(text.len(), |len| start + len)
}

/// The object a metadata path names (`Kind.Name`), when `value` as a whole
/// is a path into a configuration object other than the tree's own `own`.
fn configuration_path(value: &str, own: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || identifier_end(value, 0) != value.len() {
        return None;
    }
    let mut parts = value.split('.');
    let (kind, name) = (parts.next()?, parts.next()?);
    if name.is_empty() || !CONFIGURATION_KINDS.contains(&kind) {
        return None;
    }
    let path = format!("{kind}.{name}");
    (path != own).then_some(path)
}

/// The configuration objects `text` names where a reference is written: a
/// `cfg:` type with an object name, a whole element value (`<xr:Ref>`,
/// `<CommandName>`, a design-time reference, an `MDObjectRef` item) and a
/// whole attribute value (a role's `name="Role.…"`, a help link
/// `href="Catalog.…/Help"`). A path inside a text (a cell
/// `ExchangePlan.%1`, a link `e1cib/data/Catalog.…?ref=`) and a type set of
/// every object of a kind (`cfg:CatalogRef`) are not references: the
/// platform builds both without the configuration. `own` is the tree's
/// object in the configuration spelling (`DataProcessor.<name>`).
fn references_in(text: &str, own: &str, found: &mut BTreeSet<String>) {
    let mut at = 0;
    while let Some(offset) = text[at..].find("cfg:") {
        let start = at + offset + 4;
        let end = identifier_end(text, start);
        let name = &text[start..end];
        if name.contains('.')
            && !name.starts_with("ExternalDataProcessor")
            && !name.starts_with("ExternalReport")
        {
            found.insert(format!("cfg:{name}"));
        }
        at = end.max(start);
    }
    for piece in text.split('>').skip(1) {
        let value = piece.split('<').next().unwrap_or_default();
        found.extend(configuration_path(value, own));
    }
    for piece in text.split("=\"").skip(1) {
        let value = piece.split('"').next().unwrap_or_default();
        found.extend(configuration_path(
            value.split('/').next().unwrap_or_default(),
            own,
        ));
    }
}

/// The configuration objects the tree names: without the configuration
/// their identities are unknown -- a platform building the object from its
/// XML in an empty infobase drops them -- so such a build is refused.
fn configuration_references(tree: &Path, own: &str) -> Result<Vec<String>> {
    let mut found = BTreeSet::new();
    let mut stack = vec![tree.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in
            fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))?
        {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|ext| ext == "xml" || ext == "html")
            {
                let text = fs::read_to_string(&path)
                    .with_context(|| format!("failed to read {}", path.display()))?;
                references_in(&text, own, &mut found);
            }
        }
    }
    Ok(found.into_iter().collect())
}

/// The identities of the tree's external root.
#[derive(Debug, PartialEq, Eq)]
struct Identity {
    uuid: String,
    object_id: String,
    type_id: String,
    value_id: String,
    name: String,
}

/// The text between `open` and the next `close` after `from`.
fn between<'a>(text: &'a str, from: usize, open: &str, close: &str) -> Option<(&'a str, usize)> {
    let start = from + text[from..].find(open)? + open.len();
    let end = start + text[start..].find(close)?;
    Some((&text[start..end], end))
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

/// The root's identities as its XML states them; the `Object` generated type
/// is the one the main row carries.
fn identity(root_xml: &str, tag: &str) -> Result<Identity> {
    let opening = format!("<{tag} uuid=\"");
    let (uuid, at) = between(root_xml, 0, &opening, "\"")
        .with_context(|| format!("the root XML has no <{tag} uuid=…>"))?;
    let (object_id, _) = between(root_xml, at, "<xr:ObjectId>", "</xr:ObjectId>")
        .context("the root XML names no contained ObjectId")?;
    let object_at = root_xml[at..]
        .find("category=\"Object\"")
        .map(|found| at + found)
        .context("the root XML names no Object generated type")?;
    let (type_id, after) = between(root_xml, object_at, "<xr:TypeId>", "</xr:TypeId>")
        .context("the Object generated type has no TypeId")?;
    let (value_id, _) = between(root_xml, after, "<xr:ValueId>", "</xr:ValueId>")
        .context("the Object generated type has no ValueId")?;
    let (name, _) =
        between(root_xml, at, "<Name>", "</Name>").context("the root XML has no <Name>")?;
    let identity = Identity {
        uuid: uuid.to_ascii_lowercase(),
        object_id: object_id.trim().to_ascii_lowercase(),
        type_id: type_id.trim().to_ascii_lowercase(),
        value_id: value_id.trim().to_ascii_lowercase(),
        name: name.trim().to_owned(),
    };
    for id in [
        &identity.uuid,
        &identity.object_id,
        &identity.type_id,
        &identity.value_id,
    ] {
        if !is_uuid(id) {
            bail!("the root XML identity `{id}` is not a uuid");
        }
    }
    if !crate::external::header::is_object_name(&identity.name) {
        bail!("the root name `{}` is not an identifier", identity.name);
    }
    Ok(identity)
}

/// The external root XML of `tree` (a directory, or the root file itself)
/// with its seed: `None` when the tree is not an external object.
fn external_root(tree: &Path) -> Result<Option<(PathBuf, String, &'static Seed)>> {
    let candidates = if tree.is_file() {
        vec![tree.to_path_buf()]
    } else {
        let mut found = fs::read_dir(tree)
            .with_context(|| format!("failed to read {}", tree.display()))?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "xml"))
            .collect::<Vec<_>>();
        found.sort();
        found
    };
    let mut roots = Vec::new();
    for path in candidates {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        for seed in [&PROCESSOR, &REPORT] {
            if text.contains(&format!("<{} uuid=\"", seed.tag)) {
                roots.push((path.clone(), text.clone(), seed));
            }
        }
    }
    match roots.len() {
        0 => Ok(None),
        1 => Ok(roots.pop()),
        _ => bail!("the tree holds more than one external root XML"),
    }
}

/// `text` with every seed identity replaced by the tree's. The seed's
/// children are renamed before the root takes the tree's name, which may be
/// one of theirs (a processor named `Число`).
fn rekeyed(text: &str, seed: &Seed, target: &Identity, children: &[(String, String)]) -> String {
    let mut out = text.to_owned();
    for (from, to) in children {
        out = out.replace(from.as_str(), to.as_str());
    }
    for (index, child) in seed.children.iter().enumerate() {
        out = out.replace(
            &format!("\"{child}\""),
            &format!("\"ibcmdSeedChild{index}\""),
        );
    }
    for (from, to) in [
        (seed.uuid, target.uuid.as_str()),
        (seed.object_id, target.object_id.as_str()),
        (seed.type_id, target.type_id.as_str()),
        (seed.value_id, target.value_id.as_str()),
    ] {
        out = out.replace(from, to);
    }
    out.replace(
        &format!("\"{}\"", seed.name),
        &format!("\"{}\"", target.name),
    )
}

/// Writes the seed package, given the tree's identities, to `path`.
fn write_seed(seed: &Seed, target: &Identity, path: &Path) -> Result<()> {
    let raw = path.with_extension("raw");
    fs::write(&raw, seed.bytes).context("failed to write the seed package")?;
    let (archive, limits) = compiled::decode_base(&raw)?;
    // The seed's own forms and templates get fresh ids too: a tree child
    // with the very id of a seed child (a tree made from the seed's own
    // source) would otherwise be removed with it.
    let children = archive
        .image()
        .entries()
        .iter()
        .filter_map(|entry| {
            let key = entry.logical_key().as_str();
            let id = key.split('.').next()?;
            (is_uuid(id) && ![seed.uuid, seed.object_id, seed.type_id, seed.value_id].contains(&id))
                .then(|| id.to_owned())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|id| {
            let fresh =
                crate::external::derived_uuid(&format!("ibcmd-rs seed child {id} {}", target.uuid));
            (id, fresh)
        })
        .collect::<Vec<_>>();
    let mut rows = BTreeMap::new();
    let mut dropped = BTreeSet::new();
    for entry in archive.image().entries() {
        let key = entry.logical_key().as_str();
        let new_key = rekeyed(key, seed, target, &children);
        let packed = entry.packed_payload();
        let rewritten = match inflate_raw(packed)
            .ok()
            .and_then(|plain| String::from_utf8(plain).ok())
        {
            Some(text) => {
                let new_text = rekeyed(&text, seed, target, &children);
                (new_text != text)
                    .then(|| deflate_raw(new_text.as_bytes()))
                    .transpose()?
            }
            None => None,
        };
        if new_key != key {
            dropped.insert(key.to_owned());
            rows.insert(new_key, rewritten.unwrap_or_else(|| packed.to_vec()));
        } else if let Some(rewritten) = rewritten {
            rows.insert(new_key, rewritten);
        }
    }
    // The seed's copyinfo names the seed's own references; the built object
    // refers to nothing outside itself (`configuration_references` refused
    // the rest), and 8.3.27.2214 building such an object from its XML in an
    // empty infobase writes this one (`/LoadExternalDataProcessorOrReportFromFiles`).
    rows.insert(
        "copyinfo".to_owned(),
        deflate_raw(EMPTY_COPYINFO.as_bytes())?,
    );
    compiled::write_edit(&archive, rows, &dropped, path, limits)?;
    let _ = fs::remove_file(&raw);
    Ok(())
}

/// One build at a time: the form compiler records data-path steps for the
/// whole process (`form_native::record_data_path_steps`).
static BUILD: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Builds `output` from the external tree `tree` with no base package:
/// `Ok(None)` when the tree is not an external object. The package is built
/// and verified in a scratch folder and published last, never over an
/// existing file; a failed build writes no `output`.
pub fn build_external(
    tree: &Path,
    output: &Path,
    source_version: crate::legacy_version::InfobaseConfigSourceVersion,
) -> std::result::Result<Option<super::LoadReport>, super::LoadError> {
    let Some((root_xml, text, seed)) = external_root(tree)? else {
        return Ok(None);
    };
    let target = identity(&text, seed.tag)?;
    let internal = if seed.tag == "ExternalReport" {
        "Report"
    } else {
        "DataProcessor"
    };
    let references = configuration_references(
        if tree.is_file() {
            root_xml.parent().unwrap_or(tree)
        } else {
            tree
        },
        &format!("{internal}.{}", target.name),
    )?;
    if !references.is_empty() {
        return Err(anyhow!(
            "the object refers to configuration objects, whose identities only the \
             configuration holds -- build it with cf load --base <a file of that \
             configuration's object>: {}",
            references.join(", ")
        )
        .into());
    }
    let tree_dir = if tree.is_file() {
        root_xml
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| anyhow!("the root XML has no folder"))?
    } else {
        tree.to_path_buf()
    };
    let _build = BUILD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let scratch = std::env::temp_dir().join(format!(
        "ibcmd-rs-seed-{}-{}-{}",
        std::process::id(),
        target.uuid,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    ));
    fs::create_dir(&scratch).context("failed to create the seed scratch folder")?;
    let (seed_path, built) = if seed.tag == "ExternalReport" {
        (scratch.join("seed.erf"), scratch.join("built.erf"))
    } else {
        (scratch.join("seed.epf"), scratch.join("built.epf"))
    };
    crate::compiler::bodies::form_native::record_data_path_steps();
    let result = write_seed(seed, &target, &seed_path)
        .map_err(super::LoadError::from)
        .and_then(|()| {
            match super::load_onto_base(&tree_dir, &seed_path, &built, source_version) {
                // The tree is the seed itself: the rekeyed seed is the package.
                Err(super::LoadError::NoChanges) => fs::copy(&seed_path, &built)
                    .map(|_| super::LoadReport::default())
                    .context("failed to write the package")
                    .map_err(super::LoadError::from),
                other => other,
            }
        });
    let steps = crate::compiler::bodies::form_native::take_data_path_steps();
    let result = result.and_then(|report| {
        let copyinfo = copyinfo_with_steps(&text, seed, &target, &steps)?;
        if copyinfo != EMPTY_COPYINFO {
            replace_entry(&built, "copyinfo", copyinfo.as_bytes(), &scratch)?;
        }
        publish(&built, output)?;
        Ok(report)
    });
    let _ = fs::remove_dir_all(&scratch);
    result.map(Some)
}

/// The marker of a value-tree additional column segment
/// (`form_native::DATA_PATH_ADDITIONAL_COLUMN_UUID`).
const ADDITIONAL_COLUMN_UUID: &str = "5bdad865-f2c5-434b-8041-ba4aad3b6687";

/// Builtin types whose members a form path steps through by column id.
const BUILTIN_TYPES: [(&str, &str); 2] = [
    ("v8:ValueTable", "acf6192e-81ca-46ef-93a6-5a6968b78663"),
    ("v8:ValueTree", "e603c0f2-92fb-4d47-8f38-a44a381cf235"),
];

/// The generated `TabularSection` type id of each tabular section the root
/// XML declares.
fn tabular_section_types(root_xml: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut at = 0;
    while let Some(offset) = root_xml[at..].find("<TabularSection uuid=\"") {
        let start = at + offset;
        let end = root_xml[start..]
            .find("</TabularSection>")
            .map_or(root_xml.len(), |len| start + len);
        let body = &root_xml[start..end];
        let type_id = body.find("category=\"TabularSection\"").and_then(|found| {
            between(body, found, "<xr:TypeId>", "</xr:TypeId>").map(|(id, _)| id.trim())
        });
        let name = between(body, 0, "<Properties>", "</Properties>")
            .and_then(|(properties, _)| between(properties, 0, "<Name>", "</Name>"))
            .map(|(name, _)| name.trim());
        if let (Some(type_id), Some(name)) = (type_id, name) {
            out.insert(name.to_owned(), type_id.to_ascii_lowercase());
        }
        at = end;
    }
    out
}

/// `copyinfo` naming the members the forms' data paths step through.
///
/// 8.3.27.2214 keeps a form `<DataPath>` only if each step it takes into a
/// type is listed in the third block as `{<segment>,<type id>,{<n>|-1}}`:
/// with the platform's own copyinfo of a built processor its `Объект.Адрес`
/// paths survive, with that block removed both are dropped, and the other
/// blocks change nothing (corpus processor `узОбновлениеПрограммы`). The
/// steps are what the form compiler walked
/// (`form_native::take_data_path_steps`): through the object's own type and
/// its tabular sections (107 of the 111 corpus processors' blocks predicted
/// so), and through value-table and value-tree columns.
fn copyinfo_with_steps(
    root_xml: &str,
    seed: &Seed,
    target: &Identity,
    steps: &BTreeSet<(String, String)>,
) -> Result<String> {
    let internal = if seed.tag == "ExternalReport" {
        "Report"
    } else {
        "DataProcessor"
    };
    let own = format!("meta|{internal}.{}|", target.name);
    let sections = tabular_section_types(root_xml);
    let mut entries = Vec::new();
    let mut unmeasured = BTreeSet::new();
    for (segment, parent) in steps {
        let inner = segment.trim_start_matches('{').trim_end_matches('}');
        // The shapes measured over the Vanessa Automation corpus (631 own
        // members, 64 value table and tree columns, 22 value-tree additional
        // columns); any other step is refused, not left for the platform to
        // drop.
        let measured = if let Some(section) = parent.strip_prefix(&own) {
            let type_id = if section.is_empty() {
                Some(target.type_id.clone())
            } else {
                sections.get(section).cloned()
            };
            type_id
                .filter(|_| inner.starts_with("0,"))
                .map(|type_id| (type_id, "-1".to_owned()))
        } else if let Some((name, uuid)) = BUILTIN_TYPES.iter().find(|(name, _)| name == parent) {
            if inner.parse::<i64>().is_ok() {
                Some(((*uuid).to_owned(), inner.to_owned()))
            } else if *name == "v8:ValueTree"
                && inner.ends_with(&format!(",{ADDITIONAL_COLUMN_UUID}"))
            {
                // A value-tree additional column repeats its segment.
                Some(((*uuid).to_owned(), inner.to_owned()))
            } else {
                None
            }
        } else {
            None
        };
        match measured {
            Some((type_id, index)) => {
                entries.push(format!("{{\r\n{segment},{type_id},\r\n{{{index}}}\r\n}}"));
            }
            // 8.3.27.2214 lists a platform object's members under the
            // platform type's id (СериализаторMXL of Vanessa Automation: a
            // settings composer's `Settings.Filter`); those ids are not
            // measured.
            None => {
                unmeasured.insert(match parent.strip_prefix("platform|") {
                    Some(member) => member.to_owned(),
                    None => format!("{segment} of {parent}"),
                });
            }
        }
    }
    if !unmeasured.is_empty() {
        bail!(
            "a form data path takes a step whose copyinfo entry is not measured (a member \
             of a platform object, a totals or row-count marker) -- build it with cf load \
             --base: {}",
            unmeasured.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
    if entries.is_empty() {
        return Ok(EMPTY_COPYINFO.to_owned());
    }
    Ok(format!(
        "\u{feff}{{4,\r\n{{0}},\r\n{{0}},\r\n{{{},\r\n{}\r\n}},\r\n{{0,0}},\r\n{{0}}\r\n}}",
        entries.len(),
        entries.join(",\r\n")
    ))
}

/// `file` with the entry `key` holding `plain`; `scratch` is `file`'s folder.
fn replace_entry(file: &Path, key: &str, plain: &[u8], scratch: &Path) -> Result<()> {
    let rewritten = scratch.join("rewritten");
    {
        let (archive, limits) = compiled::decode_base(file)?;
        let rows = BTreeMap::from([(key.to_owned(), deflate_raw(plain)?)]);
        compiled::write_edit(&archive, rows, &BTreeSet::new(), &rewritten, limits)?;
    }
    fs::rename(&rewritten, file).context("failed to rewrite the package")?;
    Ok(())
}

/// Copies `built` to `output`, which must not exist; a partial copy is
/// removed.
fn publish(built: &Path, output: &Path) -> Result<()> {
    let mut target = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .with_context(|| format!("failed to create {}", output.display()))?;
    let copied = fs::File::open(built)
        .and_then(|mut source| std::io::copy(&mut source, &mut target))
        .and_then(|_| target.sync_all());
    if let Err(err) = copied {
        drop(target);
        let _ = fs::remove_file(output);
        return Err(err).with_context(|| format!("failed to write {}", output.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_root_identity() {
        let xml = "<ExternalDataProcessor uuid=\"DC0E1582-2D76-4880-BA01-3711DA6F4448\">\
            <InternalInfo><xr:ContainedObject><xr:ClassId>c</xr:ClassId>\
            <xr:ObjectId>80ba9317-3805-45ad-950d-e4cab1dd642b</xr:ObjectId></xr:ContainedObject>\
            <xr:GeneratedType name=\"ExternalDataProcessorObject.Б\" category=\"Object\">\
            <xr:TypeId>87f78a5f-837e-4f96-995e-37ab22e02b56</xr:TypeId>\
            <xr:ValueId>0e90fd7d-d3cd-4e67-a2c1-8410e4ea602e</xr:ValueId></xr:GeneratedType>\
            </InternalInfo><Properties><Name>Б</Name></Properties>";
        let found = identity(xml, "ExternalDataProcessor").unwrap();
        assert_eq!(found.uuid, "dc0e1582-2d76-4880-ba01-3711da6f4448");
        assert_eq!(found.name, "Б");
        assert!(
            identity(
                &xml.replace("<Name>Б", "<Name>1 Б"),
                "ExternalDataProcessor"
            )
            .is_err()
        );
    }

    #[test]
    fn rekeys_ids_and_the_quoted_name_only() {
        let target = Identity {
            uuid: "11111111-1111-1111-1111-111111111111".into(),
            object_id: "22222222-2222-2222-2222-222222222222".into(),
            type_id: "33333333-3333-3333-3333-333333333333".into(),
            value_id: "44444444-4444-4444-4444-444444444444".into(),
            name: "Новая".into(),
        };
        let text = format!(
            "{{{},{},\"ТестОбработка\",ТестОбработкаX}}",
            PROCESSOR.uuid, PROCESSOR.object_id
        );
        assert_eq!(
            rekeyed(&text, &PROCESSOR, &target, &[]),
            "{11111111-1111-1111-1111-111111111111,22222222-2222-2222-2222-222222222222,\
             \"Новая\",ТестОбработкаX}"
        );
    }

    fn target() -> Identity {
        Identity {
            uuid: "11111111-1111-1111-1111-111111111111".into(),
            object_id: "22222222-2222-2222-2222-222222222222".into(),
            type_id: "33333333-3333-3333-3333-333333333333".into(),
            value_id: "44444444-4444-4444-4444-444444444444".into(),
            name: "Б".into(),
        }
    }

    #[test]
    fn copyinfo_lists_measured_steps_and_refuses_the_rest() {
        let root = "<TabularSection uuid=\"t\"><InternalInfo>\
            <xr:GeneratedType name=\"x\" category=\"TabularSection\">\
            <xr:TypeId>55555555-5555-5555-5555-555555555555</xr:TypeId></xr:GeneratedType>\
            </InternalInfo><Properties><Name>ТЧ</Name></Properties></TabularSection>";
        let steps = BTreeSet::from([
            ("{0,a}".to_owned(), "meta|DataProcessor.Б|".to_owned()),
            ("{0,c}".to_owned(), "meta|DataProcessor.Б|ТЧ".to_owned()),
            ("{2}".to_owned(), "v8:ValueTable".to_owned()),
            (
                "{1,5bdad865-f2c5-434b-8041-ba4aad3b6687}".to_owned(),
                "v8:ValueTree".to_owned(),
            ),
        ]);
        let text = copyinfo_with_steps(root, &PROCESSOR, &target(), &steps).unwrap();
        assert!(text.contains("{0},\r\n{4,\r\n"));
        assert!(text.contains(
            "{1,5bdad865-f2c5-434b-8041-ba4aad3b6687},e603c0f2-92fb-4d47-8f38-a44a381cf235,\
             \r\n{1,5bdad865-f2c5-434b-8041-ba4aad3b6687}"
        ));
        assert!(text.contains("{0,a},33333333-3333-3333-3333-333333333333,\r\n{-1}"));
        assert!(text.contains("{0,c},55555555-5555-5555-5555-555555555555,\r\n{-1}"));
        assert!(text.contains("{2},acf6192e-81ca-46ef-93a6-5a6968b78663,\r\n{2}"));
        assert_eq!(
            copyinfo_with_steps(root, &PROCESSOR, &target(), &BTreeSet::new()).unwrap(),
            EMPTY_COPYINFO
        );
        for (segment, parent, named) in [
            ("{0}", "platform|Settings", "Settings"),
            ("{0,f}", "meta|Catalog.Чужой|", "Catalog.Чужой"),
            ("{100000000}", "meta|DataProcessor.Б|ТЧ", "100000000"),
            ("{0,c}", "meta|DataProcessor.Б|НетТакой", "НетТакой"),
            (
                "{1,5bdad865-f2c5-434b-8041-ba4aad3b6687}",
                "v8:ValueTable",
                "v8:ValueTable",
            ),
        ] {
            let step = BTreeSet::from([(segment.to_owned(), parent.to_owned())]);
            let refused = copyinfo_with_steps(root, &PROCESSOR, &target(), &step).unwrap_err();
            assert!(refused.to_string().contains(named), "{refused}");
        }
    }

    #[test]
    fn finds_configuration_references() {
        let mut found = BTreeSet::new();
        references_in(
            "<v8:Type>cfg:CatalogRef.Валюты</v8:Type><v8:Type>cfg:ExternalDataProcessorObject.Б\
             </v8:Type><xr:Ref>CommonPicture.Лого</xr:Ref><a href=\"Catalog.Банки/Help\">\
             <xr:Ref>StdPicture.Print</xr:Ref><xr:Value name=\"Role.Админ\">false</xr:Value>\
             <CommandName>CommonCommand.Печать</CommandName>\
             <xr:Value xsi:type=\"xr:DesignTimeRef\">Enum.Виды.EnumValue.Первый</xr:Value>\
             <CommandName>DataProcessor.Б.Command.Свой</CommandName>",
            "DataProcessor.Б",
            &mut found,
        );
        assert_eq!(
            found.into_iter().collect::<Vec<_>>(),
            [
                "Catalog.Банки",
                "CommonCommand.Печать",
                "CommonPicture.Лого",
                "Enum.Виды",
                "Role.Админ",
                "cfg:CatalogRef.Валюты"
            ]
        );
    }

    /// The three refusals the review found on the Vanessa Automation
    /// plugins: each builds, and exports back the same, without the check.
    #[test]
    fn a_path_inside_a_text_or_a_kind_type_set_is_no_reference() {
        let mut found = BTreeSet::new();
        references_in(
            "<v8:content>ExchangePlan.%1 не найден</v8:content>\
             <v8:content>e1cib/data/Catalog.Catalog1?ref=00000000</v8:content>\
             <v8:TypeSet>cfg:CatalogRef</v8:TypeSet>\
             <v8:Type>cfg:ExternalDataSourceTableRef.Источник.Таблица</v8:Type>",
            "DataProcessor.Б",
            &mut found,
        );
        assert_eq!(
            found.into_iter().collect::<Vec<_>>(),
            ["cfg:ExternalDataSourceTableRef.Источник.Таблица"]
        );
    }

    #[test]
    fn a_root_named_like_a_seed_child_keeps_its_name() {
        let target = Identity {
            name: "Число".into(),
            ..target()
        };
        let text = "{\"ТестОбработка\",\"Число\",\"Форма\"}";
        assert_eq!(
            rekeyed(text, &PROCESSOR, &target, &[]),
            "{\"Число\",\"ibcmdSeedChild4\",\"ibcmdSeedChild0\"}"
        );
    }

    /// The constants name the seeds' own identities: a replaced seed package
    /// fails here, not in a build.
    #[test]
    fn the_seed_identities_are_the_seeds_own() {
        let scratch =
            std::env::temp_dir().join(format!("ibcmd-rs-seed-identities-{}", std::process::id()));
        let _scratch = crate::test_scratch::TestScratch::adopt(&scratch);
        fs::create_dir_all(&scratch).unwrap();
        for (seed, file) in [(&PROCESSOR, "seed.epf"), (&REPORT, "seed.erf")] {
            let path = scratch.join(file);
            fs::write(&path, seed.bytes).unwrap();
            let (archive, _) = compiled::decode_base(&path).unwrap();
            let main = archive
                .image()
                .entries()
                .iter()
                .find(|entry| entry.logical_key().as_str() == seed.uuid)
                .expect("the seed's main row is keyed by its uuid");
            let text = String::from_utf8(inflate_raw(main.packed_payload()).unwrap()).unwrap();
            for id in [seed.object_id, seed.type_id, seed.value_id] {
                assert!(text.contains(id), "{} names no {id}", seed.tag);
            }
            let every = archive
                .image()
                .entries()
                .iter()
                .filter_map(|entry| inflate_raw(entry.packed_payload()).ok())
                .filter_map(|plain| String::from_utf8(plain).ok())
                .collect::<String>();
            assert!(text.contains(&format!("\"{}\"", seed.name)));
            for name in seed.children {
                assert!(
                    every.contains(&format!("\"{name}\"")),
                    "{} names no {name}",
                    seed.tag
                );
            }
        }
    }
}
