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

fn options(o: &ConversionOptions) -> Result<FormatVersion, EdtError> {
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
fn runtime_format(value: &str) -> Result<FormatVersion, EdtError> {
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
    let text = std::str::from_utf8(entry.bytes()).map_err(EdtError::source)?;
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
        match e.path().as_str() {
            ".settings/org.eclipse.core.resources.prefs" => {
                let text = std::str::from_utf8(e.bytes()).map_err(EdtError::source)?;
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
                let doc = ibcmd_xml::XmlReader::from_slice(e.bytes()).map_err(EdtError::source)?;
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
    }
    Ok(())
}

/// Stage only a validated, immutable snapshot. Borrowed readers never see the
/// caller's directory or concurrent mutations. The private codec cannot publish.
fn read_codec(format: Format, root: &Path, v: FormatVersion) -> Result<Configuration, EdtError> {
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
            SourceEntry::from_bytes(
                SourcePath::new(format!("src/{}", e.path())).map_err(EdtError::source)?,
                e.bytes().to_vec(),
            )
            .map_err(EdtError::source)?,
        );
    }
    drop(converted);
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
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<projectDescription>\r\n  <name>{project_name}</name>\r\n  <comment></comment>\r\n  <projects></projects>\r\n  <buildSpec>\r\n    <buildCommand><name>org.eclipse.xtext.ui.shared.xtextBuilder</name><arguments></arguments></buildCommand>\r\n    <buildCommand><name>com.e1c.langtool.builder.translationBuilder</name><arguments></arguments></buildCommand>\r\n  </buildSpec>\r\n  <natures>\r\n    <nature>org.eclipse.xtext.ui.shared.xtextNature</nature>\r\n    <nature>com._1c.g5.v8.dt.core.V8ConfigurationNature</nature>\r\n    <nature>com.e1c.langtool.TranslatingNature</nature>\r\n  </natures>\r\n</projectDescription>\r\n"
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
                SourceEntry::from_bytes(
                    SourcePath::new(p).map_err(EdtError::source)?,
                    e.bytes().to_vec(),
                )
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
    })
}

/// Public bridge always goes through the established ibcmd XML metadata reader,
/// retaining its ordered properties, ownership, references and opaque facets.
fn canonical(tree: &SourceTree, o: &ConversionOptions) -> Result<CanonicalConfiguration, EdtError> {
    let profile = ProfileId::parse(&format!("xml-{}", o.xml_dialect)).map_err(EdtError::source)?;
    let mut objects = Vec::new();
    let mut model_budget = ibcmd_core::model::CanonicalConfigurationBudget::default();
    let mut owners = BTreeMap::new();
    let mut uuids = BTreeSet::new();
    let mut metadata_paths = BTreeSet::new();
    let mut named_owners = Vec::new();
    for e in tree.entries() {
        if !metadata_file(e)? {
            continue;
        }
        metadata_paths.insert(e.path().as_str());
        let doc = ibcmd_xml::XmlReader::from_slice(e.bytes()).map_err(EdtError::source)?;
        let path = ObjectPath::new(vec![
            PathSegment::name(e.path().as_str()).map_err(EdtError::source)?,
        ])
        .map_err(EdtError::source)?;
        let envelope = ibcmd_xml::decode_source_metadata_envelope(&doc, profile.clone(), path)
            .map_err(|error| {
                EdtError::new(format!("{}: canonical metadata bridge: {error}", e.path()))
            })?;
        let root = envelope.root().identity().uuid();
        let stem = e
            .path()
            .as_str()
            .strip_suffix(".xml")
            .unwrap_or(e.path().as_str());
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
                    e.path()
                )));
            }
        }
    }
    let mut asset_map = BTreeMap::new();
    for e in tree.entries().iter().filter(|e| {
        !metadata_paths.contains(e.path().as_str()) && e.path().as_str() != "ConfigDumpInfo.xml"
    }) {
        // Find the nearest metadata owner in O(path depth), rather than
        // scanning every metadata descriptor for every source asset.
        let mut parent = e.path().as_str();
        let mut owner = None;
        while let Some((prefix, _)) = parent.rsplit_once('/') {
            if let Some(uuid) = owners.get(prefix) {
                owner = Some(uuid);
                break;
            }
            parent = prefix;
        }
        if let Some(uuid) = owner.or_else(|| owners.get("Configuration")) {
            let media = MediaKind::new(match e.kind() {
                SourceKind::Module => "text/x-1c-bsl",
                SourceKind::Form => "application/x-1c-form",
                SourceKind::Template => "application/x-1c-template",
                _ => "application/octet-stream",
            })
            .map_err(EdtError::source)?;
            let asset = AssetReference::new(e.digest(), e.bytes().len() as u64, media)
                .map_err(EdtError::source)?;
            model_budget
                .add_asset_reference(&asset)
                .map_err(EdtError::source)?;
            let assets = asset_map.entry(*uuid).or_insert_with(Vec::new);
            if assets.len() >= ibcmd_core::model::MAX_OBJECT_ASSETS {
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
    let mut final_budget = ibcmd_core::model::CanonicalConfigurationBudget::default();
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
            let object = CanonicalObject::new(parts).map_err(EdtError::source)?;
            final_budget.add_object(&object).map_err(EdtError::source)?;
            Ok(object)
        })
        .collect::<Result<Vec<_>, _>>()?;
    CanonicalConfiguration::new(objects).map_err(EdtError::source)
}

fn metadata_file(entry: &SourceEntry) -> Result<bool, EdtError> {
    if !entry.path().as_str().ends_with(".xml") {
        return Ok(false);
    }
    let doc = ibcmd_xml::XmlReader::from_slice(entry.bytes()).map_err(EdtError::source)?;
    Ok(doc.root().name().local() == "MetaDataObject")
}

fn same_body(a: &SourceEntry, b: &SourceEntry) -> Result<bool, EdtError> {
    if a.bytes() == b.bytes() {
        return Ok(true);
    }
    // Native XML formatting is not semantic data. Require every expanded QName,
    // attribute, non-formatting text and ordered child to survive regeneration.
    // This catches body fields that a borrowed reader accepted but did not emit.
    let structured = |e: &SourceEntry| {
        std::str::from_utf8(e.bytes()).is_ok_and(|s| {
            s.trim_start_matches('\u{feff}')
                .trim_start()
                .starts_with('<')
        })
    };
    if structured(a) && structured(b) && !a.path().as_str().ends_with(".html") {
        fn normalized(
            e: &ibcmd_xml::XmlElement,
            inherited: &BTreeMap<String, String>,
            preserve_space: bool,
            ancestors: &[&str],
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
            let mut children = Vec::new();
            for node in e.children() {
                match node {
                    XmlNode::Element(child) => {
                        let value = normalized(child, &namespaces, preserve_space, &path)?;
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
                        children.push(value)
                    }
                    XmlNode::Text(t)
                        if branches && !preserve_space && !mixed && t.value().trim().is_empty() => {
                    }
                    XmlNode::Text(t) => children.push(serde_json::json!(["text", t.value()])),
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
        let da = ibcmd_xml::XmlReader::from_slice(a.bytes()).map_err(EdtError::source)?;
        let db = ibcmd_xml::XmlReader::from_slice(b.bytes()).map_err(EdtError::source)?;
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
        return Ok(normalized(da.root(), &namespaces, false, &[])?
            == normalized(db.root(), &namespaces, false, &[])?);
    }
    if a.path().as_str().ends_with(".bsl") || a.path().as_str().ends_with(".html") {
        let normalize = |bytes: &[u8]| {
            std::str::from_utf8(bytes)
                .ok()
                .map(|s| s.trim_start_matches('\u{feff}').replace("\r\n", "\n"))
        };
        return Ok(normalize(a.bytes()).is_some() && normalize(a.bytes()) == normalize(b.bytes()));
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
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
