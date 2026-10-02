use crate::{
    Conversion, ConversionOptions, Disposition, EdtError, FileAccounting, Project, bounded,
    provenance,
};
use ibcmd_core::artifact::ProfileId;
use ibcmd_core::asset::{AssetReference, MediaKind};
use ibcmd_core::diagnostic::{ObjectPath, PathSegment};
use ibcmd_core::model::{CanonicalConfiguration, CanonicalObject, CanonicalObjectParts};
use ibcmd_xml::source_tree::{
    ReaderLimits, SourceEntry, SourceKind, SourcePath, SourceTree, publish_new_with_limits,
};
use morph1c_core::ir::Configuration;
use morph1c_core::version::FormatVersion;
use morph1c_pipeline::{ConvertOptions, Format};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) fn large_limits() -> ReaderLimits {
    ReaderLimits {
        files: ibcmd_xml::source_tree::MAX_SOURCE_FILES,
        directories: ibcmd_xml::source_tree::MAX_SOURCE_DIRECTORIES,
        depth: ibcmd_xml::source_tree::MAX_SOURCE_DEPTH,
        asset_bytes: ibcmd_xml::source_tree::MAX_SOURCE_FILE_BYTES,
        total_bytes: ibcmd_xml::source_tree::MAX_SOURCE_RETAINED_BYTES,
    }
}

pub(crate) fn options(o: &ConversionOptions) -> Result<FormatVersion, EdtError> {
    if o.edt_version != "2025.2.3" && o.edt_version != "2025.2.3+30" {
        return Err(EdtError::new(format!(
            "unsupported explicit EDT version {}",
            o.edt_version
        )));
    }
    match o.xml_dialect.as_str() {
        "2.20" => Ok(FormatVersion::new(2, 20)),
        "2.21" => Ok(FormatVersion::new(2, 21)),
        _ => Err(EdtError::new(format!(
            "unsupported XML dialect {}",
            o.xml_dialect
        ))),
    }
}
pub(crate) fn runtime_format(value: &str) -> Result<FormatVersion, EdtError> {
    let parts = value.split('.').collect::<Vec<_>>();
    if !(parts.len() == 3 || parts.len() == 4)
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(EdtError::new(
            "runtime version must contain three or four numeric components",
        ));
    }
    morph1c_core::version::parse_v8version(&parts[..3].join(".")).map_err(EdtError::source)
}
fn project_version(
    tree: &SourceTree,
    o: &ConversionOptions,
    v: FormatVersion,
) -> Result<(), EdtError> {
    let entry = tree
        .entries()
        .iter()
        .find(|e| e.path().as_str() == "DT-INF/PROJECT.PMF")
        .ok_or_else(|| EdtError::new("PROJECT.PMF missing"))?;
    project_version_bytes(entry.bytes(), o, v)
}
pub(crate) fn project_version_bytes(
    bytes: &[u8],
    o: &ConversionOptions,
    v: FormatVersion,
) -> Result<(), EdtError> {
    let text = std::str::from_utf8(bytes).map_err(EdtError::source)?;
    let mut manifest_seen = false;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        if line == "Manifest-Version: 1.0" && !manifest_seen {
            manifest_seen = true;
        } else if !line.starts_with("Runtime-Version:") {
            return Err(EdtError::new(
                "unknown PROJECT.PMF field; cannot map project controls to XML",
            ));
        }
    }
    if !manifest_seen {
        return Err(EdtError::new("PROJECT.PMF requires Manifest-Version: 1.0"));
    }
    let values = text
        .lines()
        .filter_map(|l| l.strip_prefix("Runtime-Version:"))
        .map(str::trim)
        .collect::<Vec<_>>();
    if values.len() != 1 {
        return Err(EdtError::new(
            "PROJECT.PMF requires exactly one Runtime-Version",
        ));
    }
    if runtime_format(values[0])? != v {
        return Err(EdtError::new(
            "PROJECT.PMF Runtime-Version disagrees with explicit XML dialect",
        ));
    }
    if let Some(expected) = &o.runtime_version
        && values[0] != expected
    {
        return Err(EdtError::new(
            "PROJECT.PMF Runtime-Version disagrees with explicit runtime profile",
        ));
    }
    Ok(())
}

fn validate_controls(tree: &SourceTree) -> Result<(), EdtError> {
    for e in tree.entries() {
        validate_control(e.path().as_str(), e.bytes())?;
    }
    Ok(())
}
pub(crate) fn validate_control(path: &str, bytes: &[u8]) -> Result<(), EdtError> {
    match path {
        ".settings/org.eclipse.core.resources.prefs" => {
            let text = std::str::from_utf8(bytes).map_err(EdtError::source)?;
            let lines = text.lines().filter(|l| !l.is_empty()).collect::<Vec<_>>();
            if lines.len() != 2
                || !lines.contains(&"eclipse.preferences.version=1")
                || !lines.contains(&"encoding/<project>=UTF-8")
            {
                return Err(EdtError::new(
                    "unsupported Eclipse project encoding/settings",
                ));
            }
        }
        ".project" => {
            let doc = ibcmd_xml::XmlReader::from_slice(bytes).map_err(EdtError::source)?;
            // Eclipse controls use unqualified element names and no attributes,
            // including namespace declarations. Do not consume a qualified
            // lookalike by its local name and silently discard its namespace.
            let mut pending = vec![doc.root()];
            while let Some(element) = pending.pop() {
                if element.name().prefix().is_some() || !element.attributes().is_empty() {
                    return Err(EdtError::new(
                        "unexpected qualified Eclipse project control or attributes",
                    ));
                }
                pending.extend(element.children().iter().filter_map(|node| match node {
                    ibcmd_xml::XmlNode::Element(child) => Some(child),
                    _ => None,
                }));
            }
            for node in doc.before_root().iter().chain(doc.after_root()) {
                if !matches!(node,ibcmd_xml::XmlNode::Text(t) if t.value().trim().is_empty()) {
                    return Err(EdtError::new(
                        "unmapped Eclipse project prolog/epilog content",
                    ));
                }
            }
            fn elements(
                e: &ibcmd_xml::XmlElement,
            ) -> Result<Vec<&ibcmd_xml::XmlElement>, EdtError> {
                if !e.attributes().is_empty() {
                    return Err(EdtError::new("unexpected Eclipse project attributes"));
                }
                let mut out = Vec::new();
                for node in e.children() {
                    match node {
                        ibcmd_xml::XmlNode::Element(child) => out.push(child),
                        ibcmd_xml::XmlNode::Text(t) if t.value().trim().is_empty() => {}
                        _ => return Err(EdtError::new("unexpected Eclipse project content")),
                    }
                }
                Ok(out)
            }
            fn text(e: &ibcmd_xml::XmlElement) -> Result<String, EdtError> {
                if !e.attributes().is_empty() {
                    return Err(EdtError::new("unexpected Eclipse project value attributes"));
                }
                let mut out = String::new();
                for n in e.children() {
                    if let ibcmd_xml::XmlNode::Text(t) = n {
                        out.push_str(t.value());
                    } else {
                        return Err(EdtError::new("unexpected Eclipse project value"));
                    }
                }
                Ok(out)
            }
            if doc.root().name().local() != "projectDescription" {
                return Err(EdtError::new("unexpected Eclipse project root"));
            }
            let mut fields = BTreeSet::new();
            for field in elements(doc.root())? {
                let name = field.name().local();
                if !fields.insert(name) {
                    return Err(EdtError::new("duplicate Eclipse project control field"));
                }
                match name {
                    "name" => {
                        let value = text(field)?;
                        if value.is_empty() {
                            return Err(EdtError::new("empty Eclipse project name"));
                        }
                        bounded::component(&value)?;
                    }
                    "comment" => {
                        if !text(field)?.trim().is_empty() {
                            return Err(EdtError::new(
                                "project comment cannot be represented in configuration XML",
                            ));
                        }
                    }
                    "projects" => {
                        if !elements(field)?.is_empty() {
                            return Err(EdtError::new(
                                "referenced Eclipse projects require explicit support",
                            ));
                        }
                    }
                    "buildSpec" => {
                        let mut commands = BTreeSet::new();
                        for command in elements(field)? {
                            if command.name().local() != "buildCommand" {
                                return Err(EdtError::new("unexpected Eclipse builder"));
                            }
                            let mut command_name = None;
                            let mut arguments = false;
                            for value in elements(command)? {
                                match value.name().local() {
                                    "name" if command_name.is_none() => {
                                        command_name = Some(text(value)?)
                                    }
                                    "arguments" if !arguments => {
                                        arguments = true;
                                        if !elements(value)?.is_empty() {
                                            return Err(EdtError::new(
                                                "custom Eclipse builder arguments are unsupported",
                                            ));
                                        }
                                    }
                                    _ => {
                                        return Err(EdtError::new(
                                            "unexpected Eclipse builder field",
                                        ));
                                    }
                                }
                            }
                            if !commands.insert(
                                command_name
                                    .ok_or_else(|| EdtError::new("missing builder name"))?,
                            ) {
                                return Err(EdtError::new("duplicate Eclipse builder"));
                            }
                        }
                        if commands
                            != ["org.eclipse.xtext.ui.shared.xtextBuilder".to_string()]
                                .into_iter()
                                .collect()
                            && commands
                                != [
                                    "org.eclipse.xtext.ui.shared.xtextBuilder".to_string(),
                                    "com.e1c.langtool.builder.translationBuilder".to_string(),
                                ]
                                .into_iter()
                                .collect()
                        {
                            return Err(EdtError::new("unsupported Eclipse builder set"));
                        }
                    }
                    "natures" => {
                        let mut natures = BTreeSet::new();
                        for nature in elements(field)? {
                            if nature.name().local() != "nature" {
                                return Err(EdtError::new("unexpected Eclipse nature field"));
                            }
                            if !natures.insert(text(nature)?) {
                                return Err(EdtError::new("duplicate Eclipse nature"));
                            }
                        }
                        if natures
                            != [
                                "org.eclipse.xtext.ui.shared.xtextNature".to_string(),
                                "com._1c.g5.v8.dt.core.V8ConfigurationNature".to_string(),
                            ]
                            .into_iter()
                            .collect()
                            && natures
                                != [
                                    "org.eclipse.xtext.ui.shared.xtextNature".to_string(),
                                    "com._1c.g5.v8.dt.core.V8ConfigurationNature".to_string(),
                                    "com.e1c.langtool.TranslatingNature".to_string(),
                                ]
                                .into_iter()
                                .collect()
                        {
                            return Err(EdtError::new("unsupported Eclipse project natures"));
                        }
                    }
                    _ => return Err(EdtError::new("unknown Eclipse project control field")),
                }
            }
            if !["name", "buildSpec", "natures"]
                .iter()
                .all(|name| fields.contains(name))
            {
                return Err(EdtError::new("incomplete Eclipse project description"));
            }
        }
        _ => {}
    }
    Ok(())
}

/// Stage only a validated, immutable snapshot. Borrowed readers never see the
/// caller's directory or concurrent mutations. The private codec cannot publish.
pub(crate) fn read_codec(
    format: Format,
    root: &Path,
    v: FormatVersion,
) -> Result<Configuration, EdtError> {
    let run = || {
        let (mut config, skipped) =
            morph1c_pipeline::read_config(format, root, &ConvertOptions::default())
                .map_err(EdtError::source)?;
        if !skipped.is_empty() {
            return Err(EdtError::new("codec unexpectedly skipped metadata"));
        }
        if config.source_version != Some(v) {
            return Err(EdtError::new(
                "source dialect differs from explicit profile",
            ));
        }
        config.source_version = Some(v);
        Ok(config)
    };
    match format {
        Format::Designer => formats_xml::read::with_verbatim_in_text_eol(run),
        _ => run(),
    }
}
fn render(
    format: Format,
    config: &Configuration,
    dest: &Path,
    v: FormatVersion,
) -> Result<SourceTree, EdtError> {
    morph1c_core::version::with_roundtrip_target(v, || {
        morph1c_pipeline::write_config(format, config, dest)
    })
    .map_err(EdtError::source)?;
    bounded::read_tree(dest, large_limits())
}

fn inventory_accounting(
    source: &SourceTree,
    rebuilt: &SourceTree,
    format: Format,
) -> Result<Vec<FileAccounting>, EdtError> {
    let index = rebuilt
        .entries()
        .iter()
        .map(|e| (e.path().as_str(), e))
        .collect::<BTreeMap<_, _>>();
    let mut accounting = Vec::new();
    let mut failure_count = 0usize;
    let mut failures = Vec::new();
    let mut reject = |path: &str, reason: &str| {
        failure_count += 1;
        // Inventory can contain hundreds of thousands of entries. Keep the
        // complete count without allocating an unbounded diagnostic per entry.
        if failures.len() < 128 {
            let reason = reason.chars().take(1024).collect::<String>();
            failures.push(format!("{path}: {reason}"));
        }
    };
    for e in source.entries() {
        let path = e.path().as_str();
        if let Some(new) = index.get(path) {
            // Descriptors are parsed with totality checks and regenerated from typed IR.
            // Lexical differences are represented by provenance. Every other artifact
            // must regenerate byte-exactly, so unclaimed body fragments fail closed.
            let descriptor = path.ends_with(".mdo") || metadata_file(e)?;
            if !descriptor {
                match same_body(e, new) {
                    Ok(true) => {}
                    Ok(false) => {
                        reject(
                            path,
                            "body codec did not regenerate all source bytes/semantics",
                        );
                        continue;
                    }
                    Err(error) => {
                        reject(path, &error.to_string());
                        continue;
                    }
                }
            }
            accounting.push(FileAccounting {
                path: e.path().clone(),
                disposition: Disposition::Converted,
            });
        } else if format == Format::Designer && path == "ConfigDumpInfo.xml" {
            accounting.push(FileAccounting {
                path: e.path().clone(),
                disposition: Disposition::Retained,
            });
        } else {
            reject(
                path,
                "source file has no complete descriptor/body codec; refusing to skip",
            );
        }
    }
    if failure_count != 0 {
        return Err(EdtError::new(format!(
            "{failure_count} source files failed complete body accounting (showing first {}):\n{}",
            failures.len(),
            failures.join("\n")
        )));
    }
    Ok(accounting)
}

fn project_controls(config: &Configuration, runtime: &str) -> Result<Vec<SourceEntry>, EdtError> {
    let mut entries = Vec::new();
    entries.push(
        SourceEntry::from_bytes(
            SourcePath::new("DT-INF/PROJECT.PMF").map_err(EdtError::source)?,
            format!("Manifest-Version: 1.0\r\nRuntime-Version: {runtime}\r\n").into_bytes(),
        )
        .map_err(EdtError::source)?,
    );
    let root = config
        .objects
        .iter()
        .find(|object| object.kind.as_str() == "Configuration")
        .ok_or_else(|| EdtError::new("no Configuration root"))?;
    let project_name = format!(
        "ibcmd_{}",
        root.uuid
            .0
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let eclipse = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<projectDescription>\r\n  <name>{project_name}</name>\r\n  <comment></comment>\r\n  <projects></projects>\r\n  <buildSpec>\r\n    <buildCommand><name>org.eclipse.xtext.ui.shared.xtextBuilder</name><arguments></arguments></buildCommand>\r\n  </buildSpec>\r\n  <natures>\r\n    <nature>com._1c.g5.v8.dt.core.V8ConfigurationNature</nature>\r\n    <nature>org.eclipse.xtext.ui.shared.xtextNature</nature>\r\n  </natures>\r\n</projectDescription>\r\n"
    );
    entries.push(
        SourceEntry::from_bytes(
            SourcePath::new(".project").map_err(EdtError::source)?,
            eclipse.into_bytes(),
        )
        .map_err(EdtError::source)?,
    );
    entries.push(
        SourceEntry::from_bytes(
            SourcePath::new(".settings/org.eclipse.core.resources.prefs")
                .map_err(EdtError::source)?,
            b"eclipse.preferences.version=1\r\nencoding/<project>=UTF-8\r\n".to_vec(),
        )
        .map_err(EdtError::source)?,
    );
    Ok(entries)
}
pub(crate) fn write_project_controls(
    config: &Configuration,
    project: &Path,
    runtime: &str,
) -> Result<(), EdtError> {
    for entry in project_controls(config, runtime)? {
        let target = project.join(entry.path().as_str());
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(EdtError::source)?;
        }
        std::fs::write(target, entry.bytes()).map_err(EdtError::source)?;
    }
    Ok(())
}

pub(crate) fn xml_to_edt(
    source: &SourceTree,
    o: &ConversionOptions,
) -> Result<Conversion, EdtError> {
    let v = options(o)?;
    let runtime = o.runtime_version.as_deref().ok_or_else(|| {
        EdtError::new("XML to EDT requires an explicit runtime_version for PROJECT.PMF")
    })?;
    if runtime_format(runtime)? != v {
        return Err(EdtError::new("runtime version disagrees with XML dialect"));
    }
    source.validate().map_err(EdtError::source)?;
    bounded::validate_tree(source)?;
    if source
        .entries()
        .iter()
        .any(|e| e.path().as_str().starts_with(provenance::PREFIX))
    {
        return Err(EdtError::new(
            "XML input occupies reserved provenance namespace",
        ));
    }
    let stage = tempfile::Builder::new()
        .prefix("ibcmd-edt-")
        .tempdir()
        .map_err(EdtError::source)?;
    let src = stage.path().join("xml");
    publish_new_with_limits(source, &src, large_limits()).map_err(EdtError::source)?;
    let config = read_codec(Format::Designer, &src, v)?;
    let rebuilt = render(Format::Designer, &config, &stage.path().join("rebuilt"), v)?;
    let accounting = inventory_accounting(source, &rebuilt, Format::Designer)?;
    drop(rebuilt);
    let converted = render(Format::Edt, &config, &stage.path().join("generated"), v)?;
    let mut entries = Vec::new();
    for e in converted.entries() {
        entries.push(
            e.with_path(SourcePath::new(format!("src/{}", e.path())).map_err(EdtError::source)?)
                .map_err(EdtError::source)?,
        );
    }
    drop(converted);
    entries.extend(project_controls(&config, runtime)?);
    let project = SourceTree::new(entries).map_err(EdtError::source)?;
    let canonical = canonical(source, o)?;
    let tree = provenance::retain(source, project, o, &config)?;
    // Prove typed return validity before handing an EDT result to the publisher.
    let check_src = stage.path().join("return-project");
    publish_new_with_limits(&tree, &check_src, large_limits()).map_err(EdtError::source)?;
    let returned = read_codec(Format::Edt, &check_src.join("src"), v)?;
    if provenance::semantic_digest(&returned)? != provenance::semantic_digest(&config)? {
        return Err(EdtError::new(
            "generated EDT differs from source typed semantics",
        ));
    }
    Ok(Conversion {
        tree,
        canonical,
        accounting,
        extensions: crate::extensions::source_extensions_from_model(&config)?,
    })
}

pub(crate) fn edt_to_xml(project: &Project, o: &ConversionOptions) -> Result<Conversion, EdtError> {
    let v = options(o)?;
    project_version(project.tree(), o, v)?;
    bounded::validate_tree(project.tree())?;
    validate_controls(project.tree())?;
    let stage = tempfile::Builder::new()
        .prefix("ibcmd-edt-")
        .tempdir()
        .map_err(EdtError::source)?;
    let src = stage.path().join("project");
    publish_new_with_limits(project.tree(), &src, large_limits()).map_err(EdtError::source)?;
    let config = read_codec(Format::Edt, &src.join("src"), v)?;
    let rebuilt = render(Format::Edt, &config, &stage.path().join("rebuilt"), v)?;
    let src_tree = SourceTree::new(
        project
            .entries()
            .iter()
            .filter_map(|e| e.path().as_str().strip_prefix("src/").map(|p| (p, e)))
            .map(|(p, e)| {
                e.with_path(SourcePath::new(p).map_err(EdtError::source)?)
                    .map_err(EdtError::source)
            })
            .collect::<Result<Vec<_>, _>>()?,
    )
    .map_err(EdtError::source)?;
    let mut accounting = inventory_accounting(&src_tree, &rebuilt, Format::Edt)?;
    drop(src_tree);
    drop(rebuilt);
    for a in &mut accounting {
        a.path = SourcePath::new(format!("src/{}", a.path)).map_err(EdtError::source)?;
    }
    for e in project
        .entries()
        .iter()
        .filter(|e| !e.path().as_str().starts_with("src/"))
    {
        if !matches!(
            e.path().as_str(),
            "DT-INF/PROJECT.PMF" | ".project" | ".settings/org.eclipse.core.resources.prefs"
        ) && !e.path().as_str().starts_with(provenance::PREFIX)
        {
            return Err(EdtError::new(format!(
                "{}: unknown EDT project control file",
                e.path()
            )));
        }
        accounting.push(FileAccounting {
            path: e.path().clone(),
            disposition: Disposition::Converted,
        });
    }
    let rendered = render(Format::Designer, &config, &stage.path().join("xml"), v)?;
    let tree = match provenance::restore(project.tree(), o, &config)? {
        Some(original) => {
            // Payload and manifest are untrusted. Hashes alone cannot establish
            // that retained XML describes the EDT currently being converted.
            bounded::validate_tree(&original)?;
            let original_path = stage.path().join("validated-original");
            publish_new_with_limits(&original, &original_path, large_limits())
                .map_err(EdtError::source)?;
            let original_config = read_codec(Format::Designer, &original_path, v)?;
            let original_regenerated = render(
                Format::Designer,
                &original_config,
                &stage.path().join("validated-original-rebuilt"),
                v,
            )?;
            inventory_accounting(&original, &original_regenerated, Format::Designer)?;
            if provenance::semantic_digest(&original_config)?
                != provenance::semantic_digest(&config)?
            {
                return Err(EdtError::new(
                    "retained original XML disagrees with current typed EDT semantics",
                ));
            }
            original
        }
        None => rendered,
    };
    let canonical = canonical(&tree, o)?;
    Ok(Conversion {
        tree,
        canonical,
        accounting,
        extensions: crate::extensions::source_extensions_from_model(&config)?,
    })
}

/// Public bridge always goes through the established ibcmd XML metadata reader,
/// retaining its ordered properties, ownership, references and opaque facets.
pub(crate) struct CanonicalFile<'a> {
    pub path: &'a str,
    pub kind: SourceKind,
    pub digest: ibcmd_core::storage::Sha256Digest,
    pub byte_len: u64,
}
pub(crate) trait CanonicalInventory {
    fn len(&self) -> usize;
    fn file(&self, index: usize) -> CanonicalFile<'_>;
    fn metadata_file(&self, index: usize) -> Result<bool, EdtError>;
    fn read(&self, index: usize) -> Result<std::borrow::Cow<'_, [u8]>, EdtError>;
}
impl CanonicalInventory for SourceTree {
    fn len(&self) -> usize {
        self.entries().len()
    }
    fn file(&self, index: usize) -> CanonicalFile<'_> {
        let entry = &self.entries()[index];
        CanonicalFile {
            path: entry.path().as_str(),
            kind: entry.kind(),
            digest: entry.digest(),
            byte_len: entry.bytes().len() as u64,
        }
    }
    fn metadata_file(&self, index: usize) -> Result<bool, EdtError> {
        metadata_file(&self.entries()[index])
    }
    fn read(&self, index: usize) -> Result<std::borrow::Cow<'_, [u8]>, EdtError> {
        Ok(std::borrow::Cow::Borrowed(self.entries()[index].bytes()))
    }
}
fn canonical(tree: &SourceTree, o: &ConversionOptions) -> Result<CanonicalConfiguration, EdtError> {
    canonical_inventory(tree, o)
}
pub(crate) fn canonical_inventory(
    inventory: &impl CanonicalInventory,
    o: &ConversionOptions,
) -> Result<CanonicalConfiguration, EdtError> {
    canonical_inventory_with_policy(
        inventory,
        o,
        ibcmd_core::source_policy::SourceOperationPolicy::Bounded,
    )
}
pub(crate) fn canonical_inventory_with_policy(
    inventory: &impl CanonicalInventory,
    o: &ConversionOptions,
    operation: ibcmd_core::source_policy::SourceOperationPolicy,
) -> Result<CanonicalConfiguration, EdtError> {
    let profile = ProfileId::parse(&format!("xml-{}", o.xml_dialect)).map_err(EdtError::source)?;
    let mut objects = Vec::new();
    let mut model_budget = ibcmd_core::model::CanonicalConfigurationBudget::with_policy(operation);
    let mut owners = BTreeMap::new();
    let mut uuids = BTreeSet::new();
    let mut metadata_paths = BTreeSet::new();
    let mut named_owners = Vec::new();
    for file_index in 0..inventory.len() {
        let e = inventory.file(file_index);
        if !inventory.metadata_file(file_index)? {
            continue;
        }
        metadata_paths.insert(e.path);
        let bytes = inventory.read(file_index)?;
        let doc = ibcmd_xml::XmlReader::from_slice(&bytes).map_err(EdtError::source)?;
        // Filesystem paths and diagnostic segments have different bounds.
        // Use the stable source-tree index; retain the exact physical path in
        // errors below rather than truncating Cyrillic names or relaxing the
        // common model's bounded diagnostic strings.
        let path = ObjectPath::new_with_policy(
            vec![
                PathSegment::name_with_policy("source_files", operation)
                    .map_err(EdtError::source)?,
                PathSegment::index(u32::try_from(file_index).map_err(EdtError::source)?),
            ],
            operation,
        )
        .map_err(EdtError::source)?;
        let envelope = ibcmd_xml::decode_source_metadata_envelope_with_policy(
            &doc,
            profile.clone(),
            path,
            operation,
        )
        .map_err(|error| {
            EdtError::new(format!("{}: canonical metadata bridge: {error}", e.path))
        })?;
        let root = envelope.root().identity().uuid();
        let stem = e.path.strip_suffix(".xml").unwrap_or(e.path);
        owners.insert(stem.to_string(), root);
        // Link separately stored metadata only through validated, explicit
        // ChildObjects references. An incidental neighbouring path is no proof
        // of ownership. The source decoder above rejects unknown bare kinds.
        if let Some(descriptor) = doc.root().children().iter().find_map(|node| match node {
            ibcmd_xml::XmlNode::Element(element) => Some(element),
            _ => None,
        }) && descriptor.name().local() != "Configuration"
        {
            for child in descriptor
                .children()
                .iter()
                .filter_map(|node| match node {
                    ibcmd_xml::XmlNode::Element(element)
                        if element.name().local() == "ChildObjects" =>
                    {
                        Some(element)
                    }
                    _ => None,
                })
                .flat_map(|children| children.children())
                .filter_map(|node| match node {
                    ibcmd_xml::XmlNode::Element(element) if element.attributes().is_empty() => {
                        Some(element)
                    }
                    _ => None,
                })
            {
                let collection = match child.name().local() {
                    "Form" => "Forms",
                    "Template" => "Templates",
                    "Subsystem" => "Subsystems",
                    "Recalculation" => "Recalculations",
                    "Table" => "Tables",
                    _ => continue,
                };
                let name = child
                    .children()
                    .iter()
                    .filter_map(|node| match node {
                        ibcmd_xml::XmlNode::Text(text) => Some(text.value()),
                        _ => None,
                    })
                    .collect::<String>();
                if !name.is_empty() {
                    named_owners.push((format!("{stem}/{collection}/{name}"), root));
                }
            }
        }
        for object in std::iter::once(envelope.root()).chain(envelope.descendants()) {
            if uuids.insert(object.identity().uuid()) {
                model_budget.add_object(object).map_err(EdtError::source)?;
                objects.push(object.clone());
            } else {
                return Err(EdtError::new(format!(
                    "{}: duplicate canonical metadata UUID",
                    e.path
                )));
            }
        }
    }
    let mut asset_map = BTreeMap::new();
    for index in 0..inventory.len() {
        let e = inventory.file(index);
        if metadata_paths.contains(e.path) || e.path == "ConfigDumpInfo.xml" {
            continue;
        }
        // Find the nearest metadata owner in O(path depth), rather than
        // scanning every metadata descriptor for every source asset.
        let mut parent = e.path;
        let mut owner = None;
        while let Some((prefix, _)) = parent.rsplit_once('/') {
            if let Some(uuid) = owners.get(prefix) {
                owner = Some(uuid);
                break;
            }
            parent = prefix;
        }
        if let Some(uuid) = owner.or_else(|| owners.get("Configuration")) {
            let media = MediaKind::new(match e.kind {
                SourceKind::Module => "text/x-1c-bsl",
                SourceKind::Form => "application/x-1c-form",
                SourceKind::Template => "application/x-1c-template",
                _ => "application/octet-stream",
            })
            .map_err(EdtError::source)?;
            let asset =
                AssetReference::new(e.digest, e.byte_len, media).map_err(EdtError::source)?;
            model_budget
                .add_asset_reference(&asset)
                .map_err(EdtError::source)?;
            let assets = asset_map.entry(*uuid).or_insert_with(Vec::new);
            if operation == ibcmd_core::source_policy::SourceOperationPolicy::Bounded
                && assets.len() >= ibcmd_core::model::MAX_OBJECT_ASSETS
            {
                return Err(EdtError::new(
                    "canonical object asset reference budget exceeded",
                ));
            }
            assets.push(asset);
        }
    }
    let mut declared_owners = BTreeMap::new();
    for (stem, owner) in named_owners {
        if let Some(child) = owners.get(&stem)
            && let Some(previous) = declared_owners.insert(*child, owner)
            && previous != owner
        {
            return Err(EdtError::new("conflicting declared metadata ownership"));
        }
    }
    let mut final_budget = ibcmd_core::model::CanonicalConfigurationBudget::with_policy(operation);
    let objects = objects
        .into_iter()
        .map(|obj| {
            let mut parts = CanonicalObjectParts::new(
                obj.identity().clone(),
                obj.kind().clone(),
                obj.provenance().clone(),
            );
            let declared = declared_owners.get(&obj.identity().uuid()).copied();
            if let (Some(existing), Some(declared)) = (obj.owner(), declared)
                && existing != declared
            {
                return Err(EdtError::new(
                    "embedded and declared metadata ownership conflict",
                ));
            }
            parts.owner = obj.owner().or(declared);
            parts.properties = obj.properties().to_vec();
            parts.references = obj.references().to_vec();
            parts.generated_types = obj.generated_types().to_vec();
            parts.opaque_facets = obj.opaque_facets().clone();
            parts.assets = asset_map.remove(&obj.identity().uuid()).unwrap_or_default();
            let object =
                CanonicalObject::new_with_policy(parts, operation).map_err(EdtError::source)?;
            final_budget.add_object(&object).map_err(EdtError::source)?;
            Ok(object)
        })
        .collect::<Result<Vec<_>, _>>()?;
    CanonicalConfiguration::new_with_policy(objects, operation).map_err(EdtError::source)
}

pub(crate) fn metadata_file(entry: &SourceEntry) -> Result<bool, EdtError> {
    let path = entry
        .path()
        .as_str()
        .strip_prefix(".ibcmd-provenance/xml/")
        .unwrap_or(entry.path().as_str());
    if path.starts_with("Ext/ParentConfigurations/")
        || path.starts_with("Configuration/ParentConfigurations/")
        || path.starts_with("src/Configuration/ParentConfigurations/")
    {
        return Ok(false);
    }
    if !entry.path().as_str().ends_with(".xml") {
        return Ok(false);
    }
    Ok(body_root(entry.bytes())?.0 == "MetaDataObject")
}

// SourceEntry and bounded preflight already validate complete inputs. Peek
// only the root for codec dispatch; do not rebuild a giant source asset DOM.
pub(crate) fn body_root(bytes: &[u8]) -> Result<(String, String), EdtError> {
    let mut reader = quick_xml::Reader::from_reader(bytes);
    loop {
        match reader.read_event().map_err(EdtError::source)? {
            quick_xml::events::Event::Start(e) | quick_xml::events::Event::Empty(e) => {
                let name = e.name();
                let raw = std::str::from_utf8(name.as_ref()).map_err(EdtError::source)?;
                let key = raw
                    .split_once(':')
                    .map_or_else(|| "xmlns".to_owned(), |(p, _)| format!("xmlns:{p}"));
                let mut uri = String::new();
                for a in e.attributes() {
                    let a = a.map_err(EdtError::source)?;
                    if a.key.as_ref() == key.as_bytes() {
                        uri = a
                            .decode_and_unescape_value(reader.decoder())
                            .map_err(EdtError::source)?
                            .into_owned();
                    }
                }
                return Ok((raw.rsplit(':').next().unwrap_or(raw).to_owned(), uri));
            }
            quick_xml::events::Event::Eof => return Err(EdtError::new("missing XML body root")),
            _ => {}
        }
    }
}

fn same_body(a: &SourceEntry, b: &SourceEntry) -> Result<bool, EdtError> {
    if a.path() != b.path() {
        return Ok(false);
    }
    same_body_bytes(a.path().as_str(), a.bytes(), b.bytes())
}
pub(crate) fn same_body_bytes(path: &str, a: &[u8], b: &[u8]) -> Result<bool, EdtError> {
    if a == b {
        return Ok(true);
    }
    let relative = path.strip_prefix("src/").unwrap_or(path);
    let parts = relative.split('/').collect::<Vec<_>>();
    let dcs_body_path = (parts.len() == 4
        && parts[0] == "CommonTemplates"
        && parts[2..] == ["Ext", "Template.xml"])
        || (parts.len() == 6 && parts[2] == "Templates" && parts[4..] == ["Ext", "Template.xml"])
        || (parts.len() == 3 && parts[0] == "CommonTemplates" && parts[2] == "Template.dcs")
        || (parts.len() == 5 && parts[2] == "Templates" && parts[4] == "Template.dcs");
    if dcs_body_path
        && body_root(a).is_ok_and(|root| {
            root == (
                "DataCompositionSchema".into(),
                "http://v8.1c.ru/8.1/data-composition-system/schema".into(),
            )
        })
        && body_root(b).is_ok_and(|root| {
            root == (
                "DataCompositionSchema".into(),
                "http://v8.1c.ru/8.1/data-composition-system/schema".into(),
            )
        })
    {
        return Ok(
            morph1c_pipeline::dcs_qname_semantic_bytes(a).map_err(EdtError::new)?
                == morph1c_pipeline::dcs_qname_semantic_bytes(b).map_err(EdtError::new)?,
        );
    }
    if relative == format!("Ext/{}", formats_xml::md_picture::RESOURCE) {
        return Ok(formats_xml::md_picture::same_resource(a, b));
    }
    if parts.len() == 3
        && parts.last().copied() == Some(formats_xml::metadata_picture_semantics::RESOURCE)
    {
        return Ok(formats_xml::metadata_picture_semantics::same_resource(a, b));
    }
    if parts.last().copied() == Some(formats_xml::form::PICTURE_SEMANTICS_RESOURCE)
        && (parts.len() == 3 && parts[0] == "CommonForms"
            || parts.len() == 5 && parts[2] == "Forms"
            || parts.len() == 4 && parts[0] == "CommonForms" && parts[2] == "Ext"
            || parts.len() == 6 && parts[2] == "Forms" && parts[4] == "Ext")
    {
        // Only the versioned form resource consumed by the typed pipeline has
        // JSON lexical freedom; unrelated JSON remains byte-exact.
        return Ok(formats_xml::form::same_picture_semantics_resource(a, b));
    }
    if parts.last().copied() == Some(formats_xml::form::EVENT_SEMANTICS_RESOURCE)
        && (parts.len() == 4 && parts[0] == "CommonForms" && parts[2] == "Ext"
            || parts.len() == 6 && parts[2] == "Forms" && parts[4] == "Ext")
    {
        return Ok(formats_xml::form::same_event_semantics_resource(a, b));
    }
    if parts.last().copied() == Some(formats_xml::form::CHART_SEMANTICS_RESOURCE)
        && (parts.len() == 4 && parts[0] == "CommonForms" && parts[2] == "Ext"
            || parts.len() == 6 && parts[2] == "Forms" && parts[4] == "Ext")
    {
        return Ok(formats_xml::form::same_chart_semantics_resource(a, b));
    }
    let chart_sidecar_path = (parts.len() == 6
        && parts[0] == "CommonForms"
        && parts[2] == "Attributes"
        && parts[4] == "ExtInfo")
        || (parts.len() == 8
            && parts[2] == "Forms"
            && parts[4] == "Attributes"
            && parts[6] == "ExtInfo");
    if chart_sidecar_path
        && matches!(
            parts.last().copied(),
            Some("Chart.chart" | "GanttChart.chart")
        )
    {
        // The strict chart codec accounts for every source node and attribute.
        // Compare its complete current typed values: Java BigDecimal accepts
        // BMP decimal digits which EDT subsequently writes as ASCII digits.
        // This never substitutes an earlier value or ignores unknown XML.
        return Ok(
            formats_xml::form::read_chart_sidecar(a).map_err(EdtError::source)?
                == formats_xml::form::read_chart_sidecar(b).map_err(EdtError::source)?,
        );
    }
    let mobile_format = match path {
        "Ext/MobileClientSignature.bin"
        | "src/Configuration/MobileClientSignature.bin"
        | "Configuration/MobileClientSignature.bin" => Some(Format::Designer),
        "src/Configuration/MobileClientSign.bin" | "Configuration/MobileClientSign.bin" => {
            Some(Format::Edt)
        }
        _ => None,
    };
    if let Some(format) = mobile_format {
        let a =
            morph1c_pipeline::canonical_empty_mobile_signature(a, format).map_err(EdtError::new)?;
        let b =
            morph1c_pipeline::canonical_empty_mobile_signature(b, format).map_err(EdtError::new)?;
        return Ok(a.is_some() && a == b);
    }
    // Native XML formatting is not semantic data. Require every expanded QName,
    // attribute, non-formatting text and ordered child to survive regeneration.
    // This catches body fields that a borrowed reader accepted but did not emit.
    let structured = |bytes: &[u8]| {
        std::str::from_utf8(bytes).is_ok_and(|s| {
            s.trim_start_matches('\u{feff}')
                .trim_start()
                .starts_with('<')
        })
    };
    if structured(a) && structured(b) && !path.ends_with(".html") {
        let root_a = body_root(a)?;
        let root_b = body_root(b)?;
        if root_a != root_b {
            return Ok(false);
        }
        if root_a
            == (
                "document".into(),
                "http://v8.1c.ru/8.2/data/spreadsheet".into(),
            )
        {
            fn strip_bom(bytes: &[u8]) -> &[u8] {
                bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes)
            }
            let a = morph1c_pipeline::project_mxl_content_newlines(strip_bom(a), false)
                .map_err(EdtError::source)?;
            let b = morph1c_pipeline::project_mxl_content_newlines(strip_bom(b), false)
                .map_err(EdtError::source)?;
            return Ok(a == b);
        }
        if !matches!(
            root_a.0.as_str(),
            "CommandInterface"
                | "MainSectionCommandInterface"
                | "ClientApplicationInterface"
                | "HomePageWorkArea"
                | "Rights"
                | "Form"
                | "DataCompositionSchema"
                | "dataCompositionSchema"
                | "DataCompositionSettings"
                | "Style"
                | "PredefinedData"
                | "Schedule"
                | "ExchangePlanContent"
                | "AccumulationRegisterAggregates"
        ) {
            return Ok(false);
        }
        fn normalized(
            e: &ibcmd_xml::XmlElement,
            inherited: &BTreeMap<String, String>,
            preserve_space: bool,
            ancestors: &[&str],
            in_chart_settings: bool,
        ) -> Result<serde_json::Value, EdtError> {
            use ibcmd_xml::{AttributeKind, XmlNode};
            let mut namespaces = inherited.clone();
            for a in e.attributes() {
                if let AttributeKind::Namespace(prefix) = a.kind() {
                    namespaces.insert(prefix.clone().unwrap_or_default(), a.value().to_string());
                }
            }
            let expanded = |q: &ibcmd_xml::QName, is_attribute: bool| -> Result<String, EdtError> {
                let uri = if is_attribute && q.prefix().is_none() {
                    ""
                } else {
                    namespaces
                        .get(q.prefix().unwrap_or(""))
                        .map(String::as_str)
                        .unwrap_or("")
                };
                if q.prefix().is_some() && uri.is_empty() {
                    return Err(EdtError::new("unbound namespace in body"));
                }
                Ok(format!("{{{uri}}}{}", q.local()))
            };
            let mut attrs = BTreeMap::new();
            for a in e.attributes() {
                if let AttributeKind::Ordinary(q) = a.kind() {
                    attrs.insert(expanded(q, true)?, a.value().to_string());
                }
            }
            let branches = e
                .children()
                .iter()
                .any(|n| matches!(n, XmlNode::Element(_)));
            let preserve_space = e
                .attributes()
                .iter()
                .find_map(|a| {
                    if matches!(a.kind(),AttributeKind::Ordinary(q) if q.raw()=="xml:space") {
                        Some(a.value())
                    } else {
                        None
                    }
                })
                .map_or(preserve_space, |value| value == "preserve");
            let mixed = e
                .children()
                .iter()
                .any(|n| matches!(n,XmlNode::Text(t) if !t.value().trim().is_empty()));
            let name = expanded(e.name(), false)?;
            let mut path = ancestors.to_vec();
            path.push(name.as_str());
            let in_chart_settings = in_chart_settings
                || (name == "{http://v8.1c.ru/8.3/xcf/logform}Settings"
                    && attrs
                        .get("{http://www.w3.org/2001/XMLSchema-instance}type")
                        .is_some_and(|value| {
                            let (prefix, local) = value.split_once(':').unwrap_or(("", value));
                            matches!(local, "Chart" | "GanttChart")
                                && namespaces.get(prefix).map(String::as_str)
                                    == Some("http://v8.1c.ru/8.2/data/chart")
                        }));
            let chart_decimal = path.first().copied()
                == Some("{http://v8.1c.ru/8.3/xcf/logform}Form")
                && in_chart_settings
                && attrs.len() == 1
                && attrs
                    .get("{http://www.w3.org/2001/XMLSchema-instance}type")
                    .is_some_and(|value| {
                        let (prefix, local) = value.split_once(':').unwrap_or(("", value));
                        local == "decimal"
                            && namespaces.get(prefix).map(String::as_str)
                                == Some("http://www.w3.org/2001/XMLSchema")
                    })
                && !branches;
            let sparse_cmi_false = path.first().copied()
                == Some("{http://g5.1c.ru/v8/dt/form}Form")
                && path.len() == 6
                && path[1] == "{}commandInterface"
                && matches!(path[2], "{}navigationPanel" | "{}commandBar")
                && path.ends_with(&["{}cmiFragmentRecord", "{}userVisible", "{}for"])
                && attrs.is_empty()
                && !mixed
                && !preserve_space;
            let sparse_column_card_true = path.first().copied()
                == Some("{http://g5.1c.ru/v8/dt/form}Form")
                && path.len() >= 3
                && path[path.len() - 2] == "{}items"
                && name == "{}extInfo"
                && namespaces.get("form").map(String::as_str) == Some("http://g5.1c.ru/v8/dt/form")
                && attrs.len() == 1
                && attrs
                    .get("{http://www.w3.org/2001/XMLSchema-instance}type")
                    .map(String::as_str)
                    == Some("form:ColumnGroupExtInfo")
                && !mixed
                && !preserve_space;
            // Original SDK empty Picture import leaves this exact containment
            // null; its 8.5.1 writer materializes the empty node. No other slot
            // or nonempty picture shares this equivalence.
            let native_form_uri = "http://v8.1c.ru/8.3/xcf/logform";
            let type_is_choice = attrs
                .get("{http://www.w3.org/2001/XMLSchema-instance}type")
                .is_some_and(|value| {
                    let (prefix, local) = value.split_once(':').unwrap_or(("", value));
                    local == "FormChoiceListDesTimeValue"
                        && namespaces.get(prefix).map(String::as_str) == Some(native_form_uri)
                });
            let nullable_choice_picture = path.first().copied()
                == Some("{http://v8.1c.ru/8.3/xcf/logform}Form")
                && path.iter().position(|p| *p == "{http://v8.1c.ru/8.3/xcf/logform}ChoiceParameters").is_some_and(|index| {
                    let suffix = &path[index + 1..];
                    suffix.len() >= 2 && suffix[0] == "{http://v8.1c.ru/8.2/managed-application/core}item"
                        && suffix[1] == "{http://v8.1c.ru/8.2/managed-application/core}value"
                        && suffix[2..].chunks(2).all(|pair| pair == ["{http://v8.1c.ru/8.3/xcf/logform}Value", "{http://v8.1c.ru/8.1/data/core}Value"])
                })
                && type_is_choice
                && e.children().iter().filter(|node| matches!(node,
                    ibcmd_xml::XmlNode::Element(child)
                    if expanded(child.name(), false).is_ok_and(|n| n == "{http://v8.1c.ru/8.3/xcf/logform}Picture")
                )).count() <= 1
                && e.children().iter().filter(|node| matches!(node,
                    ibcmd_xml::XmlNode::Element(child)
                    if expanded(child.name(), false).is_ok_and(|n| n == "{http://v8.1c.ru/8.3/xcf/logform}Presentation")
                )).count() <= 1
                && attrs.len() == 1
                && !mixed
                && !preserve_space;
            let mut children = Vec::new();
            for node in e.children() {
                match node {
                    XmlNode::Element(child) => {
                        let value = normalized(
                            child,
                            &namespaces,
                            preserve_space,
                            &path,
                            in_chart_settings,
                        )?;
                        // Witnessed EDT CMI role boolean: absent <value> means
                        // false. Only this exact typed scalar spelling may omit.
                        if sparse_cmi_false
                            && value == serde_json::json!(["{}value", {}, [["text", "false"]]])
                        {
                            continue;
                        }
                        // Authentic 8.3 ColumnGroup omission denotes the same
                        // known typed true default emitted by the EDT codec.
                        if sparse_column_card_true
                            && value == serde_json::json!(["{}showInCard", {}, [["text", "true"]]])
                        {
                            continue;
                        }
                        if nullable_choice_picture
                            && (value
                                == serde_json::json!([
                                    "{http://v8.1c.ru/8.3/xcf/logform}Picture",
                                    {},
                                    []
                                ])
                                || value
                                    == serde_json::json!([
                                        "{http://v8.1c.ru/8.3/xcf/logform}Presentation",
                                        {},
                                        []
                                    ]))
                        {
                            continue;
                        }
                        children.push(value)
                    }
                    XmlNode::Text(t)
                        if branches && !preserve_space && !mixed && t.value().trim().is_empty() => {
                    }
                    XmlNode::Text(t) => {
                        if chart_decimal {
                            let digits = formats_xml::form::normalize_big_decimal(t.value())
                                .ok_or_else(|| {
                                    EdtError::new("invalid chart BigDecimal body value")
                                })?;
                            children.push(serde_json::json!([
                                "text",
                                digits.strip_suffix(".0").unwrap_or(&digits)
                            ]));
                        } else {
                            children.push(serde_json::json!(["text", t.value()]));
                        }
                    }
                    XmlNode::Comment(t) => children.push(serde_json::json!(["comment", t.value()])),
                    _ => {
                        return Err(EdtError::new(
                            "body has an unaccounted XML lexical fragment",
                        ));
                    }
                }
            }
            Ok(serde_json::json!([name, attrs, children]))
        }
        let da = ibcmd_xml::XmlReader::from_slice(a).map_err(EdtError::source)?;
        let db = ibcmd_xml::XmlReader::from_slice(b).map_err(EdtError::source)?;
        // These are the source-family codec bodies whose structured emitters
        // explicitly define indentation/BOM/EOL conventions. Unknown XML bodies
        // must remain byte-exact; this is not a general XML comparison mode.
        if da.root().name().local() != db.root().name().local()
            || !matches!(
                da.root().name().local(),
                "CommandInterface"
                    | "MainSectionCommandInterface"
                    | "ClientApplicationInterface"
                    | "HomePageWorkArea"
                    | "Rights"
                    | "Form"
                    | "DataCompositionSchema"
                    | "dataCompositionSchema"
                    | "DataCompositionSettings"
                    | "Style"
                    | "PredefinedData"
                    | "Schedule"
                    | "ExchangePlanContent"
                    | "AccumulationRegisterAggregates"
            )
        {
            return Ok(false);
        }
        for doc in [&da, &db] {
            for node in doc.before_root().iter().chain(doc.after_root()) {
                if !matches!(node,ibcmd_xml::XmlNode::Text(t) if t.value().trim().is_empty()) {
                    return Ok(false);
                }
            }
        }
        let namespaces = BTreeMap::from([(
            "xml".to_string(),
            "http://www.w3.org/XML/1998/namespace".to_string(),
        )]);
        return Ok(normalized(da.root(), &namespaces, false, &[], false)?
            == normalized(db.root(), &namespaces, false, &[], false)?);
    }
    if path.ends_with(".bsl") || path.ends_with(".html") {
        let normalize = |bytes: &[u8]| {
            std::str::from_utf8(bytes)
                .ok()
                .map(|s| s.trim_start_matches('\u{feff}').replace("\r\n", "\n"))
        };
        return Ok(normalize(a).is_some() && normalize(a) == normalize(b));
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nullable_choice_picture_guard_requires_exact_typed_role_and_empty_singleton() {
        let frame = |picture: &str| {
            format!(
                r#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:app="http://v8.1c.ru/8.2/managed-application/core" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><ChoiceParameters><app:item name="Filter"><app:value xsi:type="FormChoiceListDesTimeValue"><Presentation/><Value xsi:nil="true"/>{picture}</app:value></app:item></ChoiceParameters></Form>"#
            )
        };
        let entry = |s: String| {
            SourceEntry::from_bytes(
                SourcePath::new("CommonForms/Test/Ext/Form.xml").unwrap(),
                s.into_bytes(),
            )
            .unwrap()
        };
        assert!(same_body(&entry(frame("")), &entry(frame("<Picture/>"))).unwrap());
        assert!(
            same_body(
                &entry(frame("").replace("<Presentation/>", "")),
                &entry(frame("<Picture/>"))
            )
            .unwrap()
        );
        assert!(
            !same_body(
                &entry(frame("")),
                &entry(
                    frame("<Picture/>")
                        .replace("<Presentation/>", "<Presentation/><Presentation/>")
                )
            )
            .unwrap()
        );

        for picture in [
            "<Picture unknown='true'/>",
            "<Picture/><Picture/>",
            "<Picture>value</Picture>",
            "<Picture><Ref/></Picture>",
            "<Picture xml:space='preserve'/>",
            "<Picture xmlns='urn:unknown'/>",
        ] {
            assert!(!same_body(&entry(frame("")), &entry(frame(picture))).unwrap());
        }
        for (a, b) in [
            (
                frame("").replace("FormChoiceListDesTimeValue", "Other"),
                frame("<Picture/>").replace("FormChoiceListDesTimeValue", "Other"),
            ),
            (
                frame("").replace("ChoiceParameters", "Other"),
                frame("<Picture/>").replace("ChoiceParameters", "Other"),
            ),
            (
                frame("").replace("http://v8.1c.ru/8.3/xcf/logform", "urn:other"),
                frame("<Picture/>").replace("http://v8.1c.ru/8.3/xcf/logform", "urn:other"),
            ),
        ] {
            assert!(!same_body(&entry(a), &entry(b)).unwrap());
        }
    }

    #[test]
    fn explicit_source_inventory_policy_retains_all_asset_references_without_changing_defaults() {
        use ibcmd_core::source_policy::SourceOperationPolicy;
        let mut entries = vec![SourceEntry::from_bytes(SourcePath::new("Configuration.xml").unwrap(), b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses'><Configuration uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>Owner</Name></Properties></Configuration></MetaDataObject>".to_vec()).unwrap()];
        for index in 0..=ibcmd_core::model::MAX_OBJECT_ASSETS {
            entries.push(
                SourceEntry::from_bytes(
                    SourcePath::new(format!("Ext/asset-{index:05}.bin")).unwrap(),
                    vec![index as u8],
                )
                .unwrap(),
            );
        }
        let tree = SourceTree::new(entries).unwrap();
        let options = ConversionOptions {
            edt_version: "2025.2.3".into(),
            xml_dialect: "2.21".into(),
            runtime_version: Some("8.5.1".into()),
        };
        assert!(canonical_inventory(&tree, &options).is_err());
        let model = canonical_inventory_with_policy(&tree, &options, SourceOperationPolicy::Source)
            .unwrap();
        assert_eq!(model.len(), 1);
        let assets = model.objects()[0].assets();
        assert_eq!(assets.len(), ibcmd_core::model::MAX_OBJECT_ASSETS + 1);
        for (asset, entry) in assets.iter().zip(
            tree.entries()
                .iter()
                .filter(|entry| entry.path().as_str() != "Configuration.xml"),
        ) {
            assert_eq!(asset.sha256(), entry.digest());
            assert_eq!(asset.byte_len(), entry.bytes().len() as u64);
        }
    }
    #[test]
    #[ignore = "read-only whole native corpus host model acceptance; requires shared heavy FIFO and F lab"]
    fn whole_native_host_model_acceptance() {
        let root = std::path::PathBuf::from(
            std::env::var_os("IBCMD_EDT_NATIVE_MODEL_ROOT").expect("native corpus"),
        );
        let output = std::path::PathBuf::from(
            std::env::var_os("IBCMD_EDT_NATIVE_MODEL_REPORT").expect("new F lab report"),
        );
        assert!(root.is_absolute() && output.is_absolute());
        assert!(output.starts_with(std::path::Path::new("F:/ibcmd/lab/07")));
        assert!(!output.exists(), "immutable report already exists");
        let started = std::time::Instant::now();
        let tree = bounded::read_tree(&root, large_limits()).unwrap();
        let inventory_elapsed = started.elapsed().as_secs_f64();
        #[derive(serde::Serialize)]
        struct SourceHashRow<'a> {
            path: &'a str,
            bytes: usize,
            sha256: String,
        }
        use sha2::Digest;
        let source_rows = tree
            .entries()
            .iter()
            .map(|entry| SourceHashRow {
                path: entry.path().as_str(),
                bytes: entry.bytes().len(),
                sha256: entry.digest().to_string(),
            })
            .collect::<Vec<_>>();
        let source_hash = format!(
            "{:x}",
            sha2::Sha256::digest(serde_json::to_vec(&source_rows).unwrap())
        );
        let source_bytes = source_rows.iter().map(|row| row.bytes as u64).sum::<u64>();
        drop(source_rows);
        let options = ConversionOptions {
            edt_version: "2025.2.3".into(),
            xml_dialect: std::env::var("IBCMD_EDT_NATIVE_MODEL_DIALECT")
                .unwrap_or_else(|_| "2.20".into()),
            runtime_version: None,
        };
        let result = canonical(&tree, &options);
        let report = match &result {
            Ok(model) => serde_json::json!({
                "status": "PASS", "source": root, "files": tree.entries().len(),
                "source_tree_sha256": source_hash, "source_bytes": source_bytes,
                "objects": model.len(),
                "assets": model.objects().iter().map(|o| o.assets().len()).sum::<usize>(),
                "inventory_seconds": inventory_elapsed,
                "elapsed_seconds": started.elapsed().as_secs_f64(),
            }),
            Err(error) => serde_json::json!({
                "status": "FAIL", "source": root, "files": tree.entries().len(),
                "source_tree_sha256": source_hash, "source_bytes": source_bytes,
                "inventory_seconds": inventory_elapsed,
                "elapsed_seconds": started.elapsed().as_secs_f64(), "error": error.to_string(),
            }),
        };
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(output)
            .unwrap();
        file.write_all(&serde_json::to_vec_pretty(&report).unwrap())
            .unwrap();
        result.unwrap();
    }
    #[test]
    fn picture_resource_equivalence_is_typed_and_scoped_to_forms() {
        let value = serde_json::json!({ "schema": "urn:ibcmd:source-extension:picture-semantics:1", "version": 1, "form": { "form_uuid": morph1c_core::ir::Uuid([42; 16]), "records": [] } });
        let a = serde_json::to_vec(&value).unwrap();
        let b = serde_json::to_vec_pretty(&value).unwrap();
        for path in [
            "CommonForms/F/ibcmd-picture-semantics.v1.json",
            "src/Catalogs/C/Forms/F/ibcmd-picture-semantics.v1.json",
        ] {
            assert!(same_body_bytes(path, &a, &b).unwrap());
            let mut unknown: serde_json::Value = serde_json::from_slice(&a).unwrap();
            unknown["unknown"] = true.into();
            assert!(!same_body_bytes(path, &a, &serde_json::to_vec(&unknown).unwrap()).unwrap());
            unknown.as_object_mut().unwrap().remove("unknown");
            unknown["version"] = 2.into();
            assert!(!same_body_bytes(path, &a, &serde_json::to_vec(&unknown).unwrap()).unwrap());
            unknown["version"] = 1.into();
            unknown["form"]["form_uuid"] =
                serde_json::to_value(morph1c_core::ir::Uuid([43; 16])).unwrap();
            assert!(!same_body_bytes(path, &a, &serde_json::to_vec(&unknown).unwrap()).unwrap());
        }
        for path in [
            "ibcmd-picture-semantics.v1.json",
            "CommonModules/F/ibcmd-picture-semantics.v1.json",
            "CommonForms/F/unrelated.json",
        ] {
            assert!(!same_body_bytes(path, &a, &b).unwrap());
        }
    }
    #[test]
    fn mobile_empty_signature_equivalence_is_scoped_to_the_root_blob() {
        let native = b"{2,\"\",\"\",\n{\n{0},\n{0},\n{0},\n{0}\n},0}";
        let lexical = [
            b"\xef\xbb\xbf".as_slice(),
            String::from_utf8_lossy(native)
                .replace('\n', "\r\n")
                .as_bytes(),
        ]
        .concat();
        let body = |path: &str, bytes: &[u8]| {
            SourceEntry::from_bytes(SourcePath::new(path).unwrap(), bytes.to_vec()).unwrap()
        };
        let root = "Ext/MobileClientSignature.bin";
        assert!(same_body(&body(root, native), &body(root, &lexical)).unwrap());
        let unrelated = "DataProcessors/X/Ext/MobileClientSignature.bin";
        assert!(!same_body(&body(unrelated, native), &body(unrelated, &lexical)).unwrap());
        assert!(!same_body(&body(root, native), &body("Other.bin", &lexical)).unwrap());
        let changed = String::from_utf8_lossy(native).replace("},0}", "},1}");
        assert!(!same_body(&body(root, native), &body(root, changed.as_bytes())).unwrap());
        let carrier = String::from_utf8_lossy(native).replace("{0}", "{-1}");
        assert!(same_body(&body(root, native), &body(root, carrier.as_bytes())).is_err());
        let edt = "src/Configuration/MobileClientSign.bin";
        assert!(same_body(&body(edt, native), &body(edt, carrier.as_bytes())).unwrap());
        for preferred in [
            "src/Configuration/MobileClientSignature.bin",
            "Configuration/MobileClientSignature.bin",
        ] {
            assert!(same_body(&body(preferred, native), &body(preferred, &lexical)).unwrap());
            assert!(
                same_body(
                    &body(preferred, native),
                    &body(preferred, carrier.as_bytes())
                )
                .is_err()
            );
            assert!(
                !same_body(
                    &body(preferred, native),
                    &body(preferred, changed.as_bytes())
                )
                .unwrap()
            );
        }
        let unrelated = "src/CommonModules/X/MobileClientSignature.bin";
        assert!(!same_body(&body(unrelated, native), &body(unrelated, &lexical)).unwrap());
    }
    #[test]
    fn large_mxl_inventory_and_body_guard_avoid_per_node_dom() {
        let path = SourcePath::new("CommonTemplates/Large/Ext/Template.xml").unwrap();
        let mut xml = String::from(
            "<document xmlns='http://v8.1c.ru/8.2/data/spreadsheet' xmlns:v8='http://v8.1c.ru/8.1/data/core'>",
        );
        for _ in 0..1_050_000 {
            xml.push_str("<row/>");
        }
        xml.push_str("<v8:content>line1\r\nline2</v8:content></document>");
        let original = SourceEntry::from_bytes(path.clone(), xml.as_bytes().to_vec()).unwrap();
        assert!(!metadata_file(&original).unwrap());
        let regenerated = SourceEntry::from_bytes(
            path.clone(),
            xml.replace("line1\r\nline2", "line1\nline2").into_bytes(),
        )
        .unwrap();
        assert!(same_body(&original, &regenerated).unwrap());
        let changed = SourceEntry::from_bytes(
            path.clone(),
            xml.replace("line1\r\nline2", "line1\nchanged").into_bytes(),
        )
        .unwrap();
        assert!(!same_body(&original, &changed).unwrap());
        let malformed = xml.replace("</document>", "</different>");
        assert!(SourceEntry::from_bytes(path, malformed.into_bytes()).is_err());
    }
    #[test]
    fn canonical_bridge_accepts_long_physical_paths_and_retains_exact_error_path() {
        let path = format!("CommonModules/{}.xml", "Отчет".repeat(24));
        assert!(path.len() > ibcmd_core::diagnostic::MAX_PATH_NAME_BYTES);
        let body = br#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20"><CommonModule uuid="11111111-1111-4111-8111-111111111111"><Properties><Name>Module</Name></Properties></CommonModule></MetaDataObject>"#;
        let tree = |bytes: &[u8]| {
            SourceTree::new(vec![
                SourceEntry::from_bytes(SourcePath::new(&path).unwrap(), bytes.to_vec()).unwrap(),
            ])
            .unwrap()
        };
        let options = ConversionOptions {
            edt_version: "2025.2.3".into(),
            xml_dialect: "2.20".into(),
            runtime_version: Some("8.3.27".into()),
        };
        let model = canonical(&tree(body), &options).unwrap();
        assert_eq!(model.objects().len(), 1);
        let diagnostic_path = model.objects()[0].identity().path();
        assert_eq!(
            diagnostic_path,
            &ObjectPath::new(vec![
                PathSegment::name("source_files").unwrap(),
                PathSegment::index(0),
            ])
            .unwrap()
        );
        let invalid = String::from_utf8(body.to_vec()).unwrap().replace(
            "<Name>Module</Name>",
            "<Name>Module</Name><Name>Duplicate</Name>",
        );
        let error = canonical(&tree(invalid.as_bytes()), &options)
            .unwrap_err()
            .to_string();
        assert!(error.contains(&path), "{error}");
        assert!(error.contains("canonical metadata bridge"), "{error}");
    }
    #[test]
    fn inventory_reports_missing_and_changed_bodies_together() {
        let entry = |path: &str, bytes: &[u8]| {
            SourceEntry::from_bytes(SourcePath::new(path).unwrap(), bytes.to_vec()).unwrap()
        };
        let source = SourceTree::new(vec![
            entry("Configuration/Help/ru.html", b"<html>help</html>"),
            entry(
                "CommonModules/Logic/Module.bsl",
                b"Procedure Original() EndProcedure",
            ),
        ])
        .unwrap();
        let rebuilt = SourceTree::new(vec![entry(
            "CommonModules/Logic/Module.bsl",
            b"Procedure Changed() EndProcedure",
        )])
        .unwrap();
        let error = inventory_accounting(&source, &rebuilt, Format::Edt)
            .unwrap_err()
            .to_string();
        assert!(error.contains("2 source files failed"), "{error}");
        assert!(error.contains("Configuration/Help/ru.html"), "{error}");
        assert!(error.contains("CommonModules/Logic/Module.bsl"), "{error}");
        assert!(error.contains("refusing to skip"), "{error}");
        assert!(error.contains("did not regenerate"), "{error}");
    }

    #[test]
    fn inventory_diagnostic_is_bounded_but_counts_every_rejected_file() {
        let source = SourceTree::new(
            (0..130)
                .map(|index| {
                    SourceEntry::from_bytes(
                        SourcePath::new(format!("Bodies/{index:03}.bsl")).unwrap(),
                        b"body".to_vec(),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let rebuilt = SourceTree::new(Vec::new()).unwrap();
        let error = inventory_accounting(&source, &rebuilt, Format::Edt)
            .unwrap_err()
            .to_string();
        assert!(error.contains("130 source files failed"), "{error}");
        assert!(error.contains("showing first 128"), "{error}");
        assert_eq!(error.matches("refusing to skip").count(), 128);
        assert!(error.contains("Bodies/127.bsl"));
        assert!(!error.contains("Bodies/128.bsl"));
        assert!(!error.contains("Bodies/129.bsl"));
    }

    #[test]
    #[ignore = "requires genuine F laboratory form corpus"]
    fn strict_form_regeneration_census() {
        use formats_xml::form::{FormDialect, read_form, write_form};
        let root = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_CENSUS_ROOT").unwrap());
        let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
        let dialect = if std::env::var("IBCMD_EDT_CENSUS_DIALECT").unwrap_or_default() == "xml" {
            FormDialect::Designer
        } else {
            FormDialect::Edt
        };
        let label = if dialect == FormDialect::Edt {
            "edt"
        } else {
            "xml"
        };
        let start = std::time::Instant::now();
        fn files(root: &Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(root).unwrap() {
                let entry = entry.unwrap();
                let kind = entry.file_type().unwrap();
                assert!(!kind.is_symlink());
                if kind.is_dir() {
                    files(&entry.path(), out);
                } else if entry.path().extension().is_some_and(|e| e == "form")
                    || entry.file_name() == "Form.xml"
                {
                    out.push(entry.path());
                }
            }
        }
        let mut paths = Vec::new();
        files(&root, &mut paths);
        paths.sort();
        assert!(!paths.is_empty());
        let witnesses = lab.join(format!("form-regeneration-{label}-witnesses"));
        std::fs::create_dir_all(&witnesses).unwrap();
        let mut failures = BTreeMap::<String, Vec<String>>::new();
        let mut passed = 0;
        let mut changed = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            let rel = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let result = (|| -> Result<bool, EdtError> {
                let snapshot = tempfile::tempdir_in(&lab).map_err(EdtError::source)?;
                let name = if dialect == FormDialect::Edt {
                    "Form.form"
                } else {
                    "Form.xml"
                };
                std::fs::copy(path, snapshot.path().join(name)).map_err(EdtError::source)?;
                let limits = ReaderLimits {
                    files: 1,
                    directories: 1,
                    depth: 2,
                    asset_bytes: 256 * 1024 * 1024,
                    total_bytes: 256 * 1024 * 1024,
                };
                let tree = bounded::read_tree(snapshot.path(), limits)?;
                let source = &tree.entries()[0];
                let body = if dialect == FormDialect::Designer {
                    formats_xml::read::with_verbatim_in_text_eol(|| {
                        read_form(dialect, source.bytes())
                    })
                } else {
                    morph1c_core::version::with_source_version(
                        Some(FormatVersion::new(2, 20)),
                        || read_form(dialect, source.bytes()),
                    )
                }
                .map_err(EdtError::source)?;
                let version = if dialect == FormDialect::Designer {
                    let descriptor =
                        formats_xml::parse(source.bytes()).map_err(EdtError::source)?;
                    match descriptor.root.attr("version").map(|a| a.value.as_str()) {
                        Some("2.20") => FormatVersion::new(2, 20),
                        Some("2.21") => FormatVersion::new(2, 21),
                        _ => {
                            return Err(EdtError::new(
                                "laboratory form requires explicit witnessed XML version",
                            ));
                        }
                    }
                } else {
                    FormatVersion::new(2, 20)
                };
                let output = morph1c_core::version::with_roundtrip_target(version, || {
                    write_form(dialect, &body)
                })
                .map_err(EdtError::source)?;
                bounded::validate_xml(name, &output)?;
                let generated = SourceEntry::from_bytes(SourcePath::new(name).unwrap(), output)
                    .map_err(EdtError::source)?;
                if same_body(source, &generated)? {
                    return Ok(true);
                }
                let witness = changed.len() + 1;
                if witness <= 1024 {
                    std::fs::write(
                        witnesses.join(format!("{witness}.source.{label}")),
                        source.bytes(),
                    )
                    .map_err(EdtError::source)?;
                    std::fs::write(
                        witnesses.join(format!("{witness}.generated.{label}")),
                        generated.bytes(),
                    )
                    .map_err(EdtError::source)?;
                }
                changed.push(serde_json::json!({"path":rel,"witness":witness}));
                Ok(false)
            })();
            match result {
                Ok(true) => passed += 1,
                Ok(false) => {}
                Err(e) => failures.entry(e.to_string()).or_default().push(rel),
            }
            if index % 250 == 0 || index + 1 == paths.len() {
                let progress = serde_json::json!({"root":root,"files":paths.len(),"processed":index+1,"passed":passed,"regeneration_differences":changed.len(),"failure_kinds":failures.len(),"elapsed_seconds":start.elapsed().as_secs_f64()});
                std::fs::write(
                    lab.join(format!("form-regeneration-{label}-progress.json")),
                    serde_json::to_vec_pretty(&progress).unwrap(),
                )
                .unwrap();
                eprintln!(
                    "regenerated={}/{} passed={} differences={} failures={} elapsed={:.1}s",
                    index + 1,
                    paths.len(),
                    passed,
                    changed.len(),
                    failures.len(),
                    start.elapsed().as_secs_f64()
                );
            }
        }
        let report = serde_json::json!({"root":root,"dialect":format!("{dialect:?}"),"files":paths.len(),"passed":passed,"regeneration_differences":changed,"failures":failures,"elapsed_seconds":start.elapsed().as_secs_f64()});
        std::fs::write(
            lab.join(format!("form-regeneration-{label}.json")),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        assert!(
            changed.is_empty() && failures.is_empty(),
            "see F laboratory regeneration census"
        );
    }
    fn body(path: &str, xml: &str) -> SourceEntry {
        SourceEntry::from_bytes(SourcePath::new(path).unwrap(), xml.as_bytes().to_vec()).unwrap()
    }
    #[test]
    fn canonical_retains_name_only_form_and_template_references() {
        let tree = SourceTree::new(vec![body("Catalogs/Test.xml", r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21"><Catalog uuid="11111111-1111-1111-1111-111111111111"><Properties><Name>Test</Name></Properties><ChildObjects><Form>MainForm</Form><Template>Print</Template></ChildObjects></Catalog></MetaDataObject>"#), body("Catalogs/Test/Forms/MainForm.xml", r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21"><Form uuid="22222222-2222-2222-2222-222222222222"><Properties><Name>MainForm</Name></Properties></Form></MetaDataObject>"#), body("Catalogs/Test/Templates/Print.xml", r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21"><Template uuid="33333333-3333-3333-3333-333333333333"><Properties><Name>Print</Name></Properties></Template></MetaDataObject>"#)]).unwrap();
        let canonical = canonical(
            &tree,
            &ConversionOptions {
                edt_version: "2025.2.3".into(),
                xml_dialect: "2.21".into(),
                runtime_version: Some("8.5.1".into()),
            },
        )
        .unwrap();
        assert_eq!(canonical.objects().len(), 3);
        for child in canonical.objects().iter().skip(1) {
            assert_eq!(
                child.owner(),
                Some(canonical.objects()[0].identity().uuid())
            );
        }
        let references = canonical.objects()[0]
            .opaque_facets()
            .as_slice()
            .iter()
            .filter(|f| f.placement().kind().as_str() == "xml:child-object-reference")
            .collect::<Vec<_>>();
        assert_eq!(references.len(), 2);
        for (facet, expected) in references.into_iter().zip(["MainForm", "Print"]) {
            let permit = facet
                .emit_permit(&ProfileId::parse("xml-2.21").unwrap())
                .unwrap();
            assert!(
                std::str::from_utf8(permit.bytes())
                    .unwrap()
                    .contains(expected)
            );
        }
    }

    #[test]
    fn external_table_owner_requires_exact_declared_reference() {
        let tree=SourceTree::new(vec![body("ExternalDataSources/X.xml",r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21"><ExternalDataSource uuid="11111111-1111-1111-1111-111111111111"><Properties><Name>X</Name></Properties><ChildObjects><Table>Y</Table></ChildObjects></ExternalDataSource></MetaDataObject>"#),body("ExternalDataSources/X/Tables/Y.xml",r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21"><Table uuid="22222222-2222-2222-2222-222222222222"><Properties><Name>Y</Name></Properties></Table></MetaDataObject>"#)]).unwrap();
        let configuration = canonical(
            &tree,
            &ConversionOptions {
                edt_version: "2025.2.3".into(),
                xml_dialect: "2.21".into(),
                runtime_version: Some("8.5.1".into()),
            },
        )
        .unwrap();
        assert_eq!(
            configuration.objects()[1].owner(),
            Some(configuration.objects()[0].identity().uuid())
        );
    }

    #[test]
    fn formatting_equivalence_preserves_space_mixed_content_and_prolog() {
        let path = "Ext/CommandInterface.xml";
        let a = body(path, "<CommandInterface><x/></CommandInterface>");
        let b = body(path, "<CommandInterface>\n  <x/>\n</CommandInterface>");
        assert!(same_body(&a, &b).unwrap());
        let a = body(
            path,
            "<CommandInterface xml:space='preserve'><x/> </CommandInterface>",
        );
        let b = body(
            path,
            "<CommandInterface xml:space='preserve'><x/>  </CommandInterface>",
        );
        assert!(!same_body(&a, &b).unwrap());
        let a = body(path, "<CommandInterface>A<x/> </CommandInterface>");
        let b = body(path, "<CommandInterface>A<x/>  </CommandInterface>");
        assert!(!same_body(&a, &b).unwrap());
        let a = body(
            path,
            "<!-- keep --><CommandInterface><x/></CommandInterface>",
        );
        let b = body(path, "<CommandInterface>\n<x/>\n</CommandInterface>");
        assert!(!same_body(&a, &b).unwrap());
        let a = body("Ext/Unknown.xml", "<Unknown><x/></Unknown>");
        let b = body("Ext/Unknown.xml", "<Unknown>\n<x/>\n</Unknown>");
        assert!(!same_body(&a, &b).unwrap());
    }
    #[test]
    fn only_edt_cmi_role_false_has_witnessed_sparse_equivalence() {
        let source = r#"<form:Form xmlns:form="http://g5.1c.ru/v8/dt/form"><commandInterface><navigationPanel><cmiFragmentRecord><userVisible><for><role>Role.Editor</role></for></userVisible></cmiFragmentRecord></navigationPanel></commandInterface></form:Form>"#;
        let sparse = body("Form.form", source);
        let explicit = source.replace("<for>", "<for><value>false</value>");
        assert!(same_body(&sparse, &body("Form.form", &explicit)).unwrap());
        for bad in [
            explicit.replace("false", "true"),
            explicit.replace("<value>", "<value retained=\"yes\">"),
            explicit.replace("<for>", "<for>keep"),
            explicit
                .replace("<value>", "<other:value xmlns:other=\"urn:unknown\">")
                .replace("</value>", "</other:value>"),
            explicit.replace("<for>", "<for xml:space=\"preserve\">"),
        ] {
            assert!(!same_body(&sparse, &body("Form.form", &bad)).unwrap());
        }
        let other = source.replace("cmiFragmentRecord", "otherRecord");
        assert!(
            !same_body(
                &body("Form.form", &other),
                &body(
                    "Form.form",
                    &other.replace("<for>", "<for><value>false</value>")
                )
            )
            .unwrap()
        );
    }
    #[test]
    fn typed_column_group_rejects_duplicate_default_property() {
        use formats_xml::form::{FormDialect, read_form, write_form};
        use morph1c_core::ir::{FieldId, FormBody, FormControlKind, FormItem, PropertyValue};
        let mut form = FormBody::new();
        let mut group = FormItem::new(FormControlKind::new("ColumnGroup"), "Column", 1);
        group
            .ext_info
            .push((FieldId(181), PropertyValue::Bool(true)));
        form.items.push(group);
        let encoded = write_form(FormDialect::Edt, &form).unwrap();
        let text = std::str::from_utf8(&encoded).unwrap();
        let valid = "<showInCard>true</showInCard>";
        assert!(text.contains(valid));
        assert!(read_form(FormDialect::Edt, &encoded).is_ok());
        let duplicate = text.replace(valid, &format!("{valid}{valid}"));
        assert!(read_form(FormDialect::Edt, duplicate.as_bytes()).is_err());
    }
    #[test]
    fn number_digit_equivalence_is_scoped_to_typed_chart_settings() {
        let source = r#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:c="http://v8.1c.ru/8.2/data/chart"><Settings xsi:type="c:Chart"><c:dataValue xsi:type="xs:decimal">١.٥</c:dataValue></Settings></Form>"#;
        let ascii = source.replace("١.٥", "1.5");
        assert!(
            same_body_bytes(
                "CommonForms/Test/Ext/Form.xml",
                source.as_bytes(),
                ascii.as_bytes()
            )
            .unwrap()
        );
        assert!(
            !same_body_bytes(
                "CommonForms/Test/Ext/Form.xml",
                source.as_bytes(),
                ascii.replace("1.5", "2.75").as_bytes()
            )
            .unwrap()
        );
        for replacement in [
            ("c:Chart", "c:Unknown"),
            ("http://v8.1c.ru/8.2/data/chart", "urn:unknown"),
            ("xs:decimal", "xs:string"),
            ("<Settings xsi:type=\"c:Chart\">", "<Settings>"),
        ] {
            let other = source.replace(replacement.0, replacement.1);
            assert!(
                !same_body_bytes(
                    "CommonForms/Test/Ext/Form.xml",
                    other.as_bytes(),
                    other.replace("١.٥", "1.5").as_bytes()
                )
                .unwrap()
            );
        }
        let path = "src/CommonForms/Test/Attributes/Diagram/ExtInfo/Chart.chart";
        let sidecar = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<chart:Chart xmlns:chart=\"http://g5.1c.ru/v8/dt/chart/model\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><realDataItems><dataValue xsi:type=\"core:NumberValue\"><value>١.٥</value></dataValue></realDataItems></chart:Chart>\r\n";
        assert!(
            same_body_bytes(
                path,
                sidecar.as_bytes(),
                sidecar.replace("١.٥", "1.5").as_bytes()
            )
            .unwrap()
        );
        assert!(
            !same_body_bytes(
                path,
                sidecar.as_bytes(),
                sidecar.replace("١.٥", "2.75").as_bytes()
            )
            .unwrap()
        );
        assert!(
            !same_body_bytes(
                "Other/Chart.chart",
                sidecar.as_bytes(),
                sidecar.replace("١.٥", "1.5").as_bytes()
            )
            .unwrap()
        );
        assert!(
            same_body_bytes(
                path,
                sidecar.as_bytes(),
                sidecar
                    .replace("</realDataItems>", "<unknown/></realDataItems>")
                    .as_bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn only_typed_column_group_show_in_card_has_true_default_equivalence() {
        let source = r#"<form:Form xmlns:form="http://g5.1c.ru/v8/dt/form" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><items xsi:type="form:FormGroup"><extInfo xsi:type="form:ColumnGroupExtInfo"><showTitle>true</showTitle></extInfo></items></form:Form>"#;
        let explicit = source.replace("</extInfo>", "<showInCard>true</showInCard></extInfo>");
        assert!(same_body(&body("Form.form", source), &body("Form.form", &explicit)).unwrap());
        for bad in [
            explicit.replace("<showInCard>true", "<showInCard>false"),
            explicit.replace("<showInCard>", "<showInCard extra='keep'>"),
            explicit.replace("<extInfo ", "<extInfo xml:space='preserve' "),
            explicit.replace("<showTitle>", "keep<showTitle>"),
            explicit.replace("<extInfo ", "<extInfo extra='keep' "),
            explicit
                .replace(
                    "<showInCard>",
                    "<other:showInCard xmlns:other='urn:unknown'>",
                )
                .replace("</showInCard>", "</other:showInCard>"),
            explicit.replace("http://g5.1c.ru/v8/dt/form", "urn:wrong:form"),
            explicit.replace("form:ColumnGroupExtInfo", "form:UsualGroupExtInfo"),
        ] {
            assert!(!same_body(&body("Form.form", source), &body("Form.form", &bad)).unwrap());
        }
    }
}
