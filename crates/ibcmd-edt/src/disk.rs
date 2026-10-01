//! Owned disk inventories: payloads are copied and hashed, never retained as a whole tree.
use crate::{EdtError, codec};
use ibcmd_core::{identity::ObjectUuid, storage::Sha256Digest};
use ibcmd_xml::source_tree::SourceKind;
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Entry {
    pub path: String,
    pub kind: SourceKind,
    pub digest: Sha256Digest,
    pub byte_len: u64,
    pub metadata: bool,
    pub uuid: Option<ObjectUuid>,
}
#[derive(Debug)]
pub(crate) struct Inventory {
    pub root: PathBuf,
    pub entries: Vec<Entry>,
    pub bytes: u64,
}
impl Inventory {
    pub fn scan(root: &Path) -> Result<Self, EdtError> {
        Self::walk(root, None)
    }
    pub fn snapshot(root: &Path, target: &Path) -> Result<Self, EdtError> {
        fs::create_dir(target).map_err(EdtError::source)?;
        Self::walk(root, Some(target))
    }
    fn walk(root: &Path, target: Option<&Path>) -> Result<Self, EdtError> {
        ordinary(root, true)?;
        let mut pending = vec![root.to_path_buf()];
        let mut folded = BTreeMap::<String, String>::new();
        let mut entries = Vec::new();
        let mut identities = BTreeMap::new();
        let mut total = 0u64;
        while let Some(dir) = pending.pop() {
            ordinary(&dir, true)?;
            let mut children = fs::read_dir(&dir)
                .map_err(EdtError::source)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(EdtError::source)?;
            children.sort_by_key(|entry| entry.file_name());
            for child in children {
                let raw_name = child.file_name();
                let raw_name = raw_name
                    .to_str()
                    .ok_or_else(|| EdtError::new("non-UTF8 source filename"))?;
                if raw_name.contains(['/', '\\']) {
                    return Err(EdtError::new("source filename contains a path separator"));
                }
                let path = child.path();
                let relative = path
                    .strip_prefix(root)
                    .map_err(EdtError::source)?
                    .to_str()
                    .ok_or_else(|| EdtError::new("non-UTF8 source path"))?
                    .replace('\\', "/");
                validate_path(&relative)?;
                let fold = relative
                    .chars()
                    .flat_map(char::to_lowercase)
                    .collect::<String>();
                if let Some(previous) = folded.insert(fold, relative.clone()) {
                    return Err(EdtError::new(format!(
                        "path conflict: {previous} / {relative}"
                    )));
                }
                let metadata = ordinary_any(&path)?;
                if metadata.is_dir() {
                    if let Some(dest) = target {
                        fs::create_dir(dest.join(&relative)).map_err(EdtError::source)?;
                    }
                    pending.push(path);
                    continue;
                }
                let mut input = File::open(&path).map_err(EdtError::source)?;
                let mut output = match target {
                    Some(dest) => Some(
                        File::options()
                            .write(true)
                            .create_new(true)
                            .open(dest.join(&relative))
                            .map_err(EdtError::source)?,
                    ),
                    None => None,
                };
                let (byte_len, digest) = transfer(
                    &mut input,
                    output.as_mut().map(|file| file as &mut dyn Write),
                )?;
                if let Some(file) = output {
                    file.sync_all().map_err(EdtError::source)?;
                }
                ordinary(&path, false)?;
                total = total
                    .checked_add(byte_len)
                    .ok_or_else(|| EdtError::new("source byte accounting overflow"))?;
                let inspect_path = target.unwrap_or(root).join(&relative);
                let (kind, descriptor, uuid) = inspect(&relative, &inspect_path)?;
                if let Some(uuid) = uuid
                    && let Some(previous) = identities.insert(uuid, relative.clone())
                {
                    return Err(EdtError::new(format!(
                        "duplicate source UUID: {previous} / {relative}"
                    )));
                }
                entries.push(Entry {
                    path: relative,
                    kind,
                    metadata: descriptor,
                    uuid,
                    digest,
                    byte_len,
                });
            }
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Self {
            root: target.unwrap_or(root).to_path_buf(),
            entries,
            bytes: total,
        })
    }
    pub fn verify(&self) -> Result<(), EdtError> {
        let current = Self::scan(&self.root)?;
        if current.entries != self.entries || current.bytes != self.bytes {
            return Err(EdtError::new("staged inventory changed"));
        }
        Ok(())
    }
    pub fn read_entry(&self, index: usize) -> Result<Vec<u8>, EdtError> {
        let entry = &self.entries[index];
        let path = self.root.join(&entry.path);
        ordinary(&path, false)?;
        let mut input = File::open(&path).map_err(EdtError::source)?;
        let mut bytes = Vec::new();
        let (length, digest) = transfer(&mut input, Some(&mut bytes))?;
        if length != entry.byte_len || digest != entry.digest {
            return Err(EdtError::new(format!(
                "{}: staged payload changed",
                entry.path
            )));
        }
        Ok(bytes)
    }
    pub fn hashes(&self) -> BTreeMap<String, String> {
        self.entries
            .iter()
            .map(|entry| (entry.path.clone(), hex(entry.digest)))
            .collect()
    }
    pub fn entry(&self, path: &str) -> Option<usize> {
        self.entries
            .binary_search_by(|entry| entry.path.as_str().cmp(path))
            .ok()
    }
    pub fn copy_to(&self, target: &Path) -> Result<Self, EdtError> {
        self.verify()?;
        let copied = Self::snapshot(&self.root, target)?;
        if copied.entries != self.entries {
            return Err(EdtError::new("source changed during staged copy"));
        }
        Ok(copied)
    }
}
impl codec::CanonicalInventory for Inventory {
    fn len(&self) -> usize {
        self.entries.len()
    }
    fn file(&self, index: usize) -> codec::CanonicalFile<'_> {
        let entry = &self.entries[index];
        codec::CanonicalFile {
            path: &entry.path,
            kind: entry.kind,
            digest: entry.digest,
            byte_len: entry.byte_len,
        }
    }
    fn metadata_file(&self, index: usize) -> Result<bool, EdtError> {
        Ok(self.entries[index].metadata)
    }
    fn read(&self, index: usize) -> Result<Cow<'_, [u8]>, EdtError> {
        Ok(Cow::Owned(self.read_entry(index)?))
    }
}
pub(crate) fn hex(digest: Sha256Digest) -> String {
    digest
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
pub(crate) fn transfer(
    input: &mut dyn Read,
    mut output: Option<&mut dyn Write>,
) -> Result<(u64, Sha256Digest), EdtError> {
    let mut hash = Sha256::new();
    let mut count = 0u64;
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let size = input.read(&mut chunk).map_err(EdtError::source)?;
        if size == 0 {
            break;
        }
        count = count
            .checked_add(size as u64)
            .ok_or_else(|| EdtError::new("file length overflow"))?;
        hash.update(&chunk[..size]);
        if let Some(writer) = output.as_deref_mut() {
            writer.write_all(&chunk[..size]).map_err(EdtError::source)?;
        }
    }
    Ok((
        count,
        Sha256Digest::parse(&format!("{:x}", hash.finalize())).map_err(EdtError::source)?,
    ))
}
pub(crate) fn validate_path(path: &str) -> Result<(), EdtError> {
    if path.contains('\\') {
        return Err(EdtError::new(
            "disk inventory paths must use normalized separators",
        ));
    }
    ibcmd_xml::source_tree::validate_source_path_safety(path).map_err(EdtError::source)
}
pub(crate) fn ordinary(path: &Path, directory: bool) -> Result<(), EdtError> {
    let metadata = ordinary_any(path)?;
    if metadata.is_dir() != directory {
        return Err(EdtError::new(format!(
            "unexpected source entry kind: {}",
            path.display()
        )));
    }
    Ok(())
}
fn ordinary_any(path: &Path) -> Result<fs::Metadata, EdtError> {
    let metadata = fs::symlink_metadata(path).map_err(EdtError::source)?;
    if metadata.file_type().is_symlink()
        || reparse(&metadata)
        || (!metadata.is_file() && !metadata.is_dir())
    {
        return Err(EdtError::new(format!(
            "unsafe source entry: {}",
            path.display()
        )));
    }
    Ok(metadata)
}
#[cfg(windows)]
fn reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn reparse(_: &fs::Metadata) -> bool {
    false
}

fn inspect(
    path: &str,
    physical: &Path,
) -> Result<(SourceKind, bool, Option<ObjectUuid>), EdtError> {
    // Both lexical inspection and filename preflight use seekable private files.
    let mut input = File::open(physical).map_err(EdtError::source)?;
    let mut prefix = [0u8; 1024];
    let length = input.read(&mut prefix).map_err(EdtError::source)?;
    let prefix = &prefix[..length];
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let structured = !matches!(ext.as_str(), "bsl" | "html" | "json")
        && (path == ".project"
            || matches!(
                ext.as_str(),
                "xml" | "mdo" | "form" | "rights" | "dcs" | "style" | "xdto" | "geos" | "flowchart"
            )
            || prefix
                .strip_prefix(b"\xef\xbb\xbf")
                .unwrap_or(prefix)
                .trim_ascii_start()
                .starts_with(b"<?xml"));
    let mut kind = ibcmd_xml::source_tree::classify_source_path(path);
    let mut descriptor = false;
    let mut uuid = None;
    if structured {
        crate::bounded::validate_xml_reader(
            path,
            std::io::BufReader::new(File::open(physical).map_err(EdtError::source)?),
        )?;
        if ext == "xml" {
            let root = ibcmd_xml::XmlReader::inspect_reader(std::io::BufReader::new(
                File::open(physical).map_err(EdtError::source)?,
            ))
            .map_err(EdtError::source)?;
            descriptor = root.local() == "MetaDataObject";
            if kind == SourceKind::OtherXml
                && matches!(
                    root.local(),
                    "MetaDataObject" | "Configuration" | "DefinedType"
                )
            {
                kind = SourceKind::MetadataXml;
            }
            if matches!(
                kind,
                SourceKind::ConfigurationRoot | SourceKind::MetadataXml
            ) {
                uuid = ibcmd_xml::source_tree::inspect_source_uuid(
                    path,
                    std::io::BufReader::new(File::open(physical).map_err(EdtError::source)?),
                )
                .map_err(EdtError::source)?;
            }
        }
    }
    Ok((kind, descriptor, uuid))
}

pub(crate) fn compare_bodies(
    source: &Inventory,
    rebuilt: &Inventory,
    index: usize,
    other: usize,
) -> Result<bool, EdtError> {
    let left = &source.entries[index];
    let right = &rebuilt.entries[other];
    if left.path != right.path {
        return Ok(false);
    }
    if left.digest == right.digest && left.byte_len == right.byte_len {
        return Ok(true);
    }
    let a = source.read_entry(index)?;
    let b = rebuilt.read_entry(other)?;
    codec::same_body_bytes(&left.path, &a, &b)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "hash-bound genuine opaque SOAP template; read-only source, F owned snapshots"]
    fn genuine_opaque_template_names_remain_byte_exact() {
        let lab = PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").expect("F lab"));
        let witness: serde_json::Value =
            serde_json::from_slice(&fs::read(lab.join("uha-opaque-name-witness-r1.json")).unwrap())
                .unwrap();
        let original = fs::read(witness["source"].as_str().unwrap()).unwrap();
        let expected = "a6f7a8b023c40c3cdc56f12a3786b117184272badd9f7db3b799bbe2e9a80161";
        assert_eq!(format!("{:x}", Sha256::digest(&original)), expected);
        let owner = tempfile::Builder::new()
            .prefix("opaque-name-")
            .tempdir_in(&lab)
            .unwrap();
        let source = owner.path().join("source");
        let relative = "DataProcessors/Owner/Templates/Template/Ext/Template.bin";
        fs::create_dir_all(source.join("DataProcessors/Owner/Templates/Template/Ext")).unwrap();
        fs::write(source.join(relative), &original).unwrap();
        let inventory = Inventory::snapshot(&source, &owner.path().join("snapshot")).unwrap();
        assert!(!inventory.entries[0].metadata);
        assert_eq!(inventory.read_entry(0).unwrap(), original);
        let copied = inventory.copy_to(&owner.path().join("published")).unwrap();
        assert_eq!(fs::read(copied.root.join(relative)).unwrap(), original);
        assert!(compare_bodies(&inventory, &copied, 0, 0).unwrap());
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(fs::read(witness["source"].as_str().unwrap()).unwrap())
            ),
            expected
        );
        // A known BinaryData template actually consumes and regenerates this
        // entire body. The label exception grants no unclaimed-file waiver.
        use morph1c_core::ir::{MetadataObject, ObjectKind, PropertyValue, Template, Token, Uuid};
        let options = morph1c_pipeline::ConvertOptions::default()
            .with_target_version(morph1c_core::version::FormatVersion::new(2, 21));
        let fixture = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        ));
        let mut config =
            morph1c_pipeline::read_config(morph1c_pipeline::Format::Designer, fixture, &options)
                .unwrap()
                .0;
        let field = morph1c_core::spec::metadata::common_template::F_TEMPLATE_TYPE;
        let properties = vec![(field, PropertyValue::Enum(Token::new("BinaryData")))];
        let mut template = MetadataObject::new(
            ObjectKind::new("CommonTemplate"),
            "OpaqueSoap",
            Uuid([0x71; 16]),
        );
        template.properties = properties.clone();
        template.templates.push(Template {
            name: "OpaqueSoap".into(),
            properties,
            body: Some(original.clone()),
            pages: Vec::new(),
            resources: Vec::new(),
        });
        config.objects.push(template);
        let native = owner.path().join("native-template");
        morph1c_pipeline::write_config(morph1c_pipeline::Format::Designer, &config, &native)
            .unwrap();
        let conversion_options = crate::ConversionOptions {
            edt_version: "2025.2.3".into(),
            xml_dialect: "2.21".into(),
            runtime_version: Some("8.5.1".into()),
        };
        let converted = crate::read_directory_source(&native)
            .unwrap()
            .xml_to_edt(&conversion_options)
            .unwrap();
        assert!(converted.accounting.iter().any(|file| file.path
            == "CommonTemplates/OpaqueSoap/Ext/Template.bin"
            && file.disposition == crate::Disposition::Converted));
        let edt = owner.path().join("edt-template");
        converted.publish_new(&edt).unwrap();
        assert_eq!(
            fs::read(edt.join("src/CommonTemplates/OpaqueSoap/Template.bin")).unwrap(),
            original
        );
        let returned = crate::read_directory_source(&edt)
            .unwrap()
            .edt_to_xml(&conversion_options)
            .unwrap();
        let xml = owner.path().join("returned-template");
        returned.publish_new(&xml).unwrap();
        assert_eq!(
            fs::read(xml.join("CommonTemplates/OpaqueSoap/Ext/Template.bin")).unwrap(),
            original
        );
    }
    #[test]
    #[ignore = "whole native host canonical disk census; F report, shared heavy FIFO"]
    fn whole_native_host_model_disk_acceptance() {
        let root =
            PathBuf::from(std::env::var_os("IBCMD_EDT_NATIVE_MODEL_ROOT").expect("native corpus"));
        let output = PathBuf::from(
            std::env::var_os("IBCMD_EDT_NATIVE_MODEL_REPORT").expect("new F lab report"),
        );
        assert!(
            root.is_absolute()
                && output.starts_with(Path::new("F:/ibcmd/lab/07"))
                && !output.exists()
        );
        let started = std::time::Instant::now();
        let inventory = Inventory::scan(&root).unwrap();
        let inventory_seconds = started.elapsed().as_secs_f64();
        #[derive(serde::Serialize)]
        struct Row<'a> {
            path: &'a str,
            bytes: u64,
            sha256: String,
        }
        let rows = inventory
            .entries
            .iter()
            .map(|entry| Row {
                path: &entry.path,
                bytes: entry.byte_len,
                sha256: entry.digest.to_string(),
            })
            .collect::<Vec<_>>();
        let source_tree_sha256 =
            format!("{:x}", Sha256::digest(serde_json::to_vec(&rows).unwrap()));
        drop(rows);
        let options = crate::ConversionOptions {
            edt_version: "2025.2.3".into(),
            xml_dialect: std::env::var("IBCMD_EDT_NATIVE_MODEL_DIALECT")
                .unwrap_or_else(|_| "2.20".into()),
            runtime_version: None,
        };
        let result = codec::canonical_inventory(&inventory, &options);
        // Re-scan the complete source after the authoritative model operation.
        let unchanged = inventory.verify();
        let mut report = serde_json::json!({ "source": root, "files": inventory.entries.len(), "source_bytes": inventory.bytes, "source_tree_sha256": source_tree_sha256, "inventory_seconds": inventory_seconds, "elapsed_seconds": started.elapsed().as_secs_f64(), "source_unchanged": unchanged.is_ok() });
        match &result {
            Ok(model) => {
                report["status"] = "PASS".into();
                report["objects"] = model.len().into();
                report["assets"] = model
                    .objects()
                    .iter()
                    .map(|o| o.assets().len())
                    .sum::<usize>()
                    .into();
            }
            Err(error) => {
                report["status"] = "FAIL".into();
                report["error"] = error.to_string().into();
            }
        }
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(output)
            .unwrap();
        file.write_all(&serde_json::to_vec_pretty(&report).unwrap())
            .unwrap();
        file.sync_all().unwrap();
        unchanged.unwrap();
        result.unwrap();
    }
    #[test]
    fn snapshot_has_no_aggregate_buffer_and_detects_tampering() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("opaque.bin"), b"opaque unchanged").unwrap();
        let copy = Inventory::snapshot(&source, &root.path().join("snapshot")).unwrap();
        fs::write(source.join("opaque.bin"), b"user input changed later").unwrap();
        assert_eq!(copy.read_entry(0).unwrap(), b"opaque unchanged");
        fs::write(copy.root.join("opaque.bin"), b"opaque tampered!").unwrap();
        assert!(copy.verify().is_err());
        assert!(copy.read_entry(0).is_err());
    }
    #[test]
    fn directory_depth_is_not_a_fixed_inventory_validity_limit() {
        let owner = tempfile::tempdir().unwrap();
        let source = owner.path().join("source");
        let relative = format!("{}body.bin", "a/".repeat(70));
        let path = source.join(&relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"exact body").unwrap();
        assert!(ibcmd_xml::source_tree::SourcePath::new(&relative).is_err());
        let copied = Inventory::snapshot(&source, &owner.path().join("snapshot")).unwrap();
        assert_eq!(copied.entries.len(), 1);
        assert_eq!(copied.read_entry(0).unwrap(), b"exact body");
        copied.verify().unwrap();
    }
    #[test]
    #[cfg(windows)]
    fn ntfs_unicode_component_is_checked_by_filesystem_not_utf8_byte_quota() {
        let owner = tempfile::tempdir().unwrap();
        let source = owner.path().join("source");
        fs::create_dir(&source).unwrap();
        let name = format!("{}.bin", "Имя".repeat(50));
        assert!(ibcmd_xml::source_tree::SourcePath::new(&name).is_err());
        fs::write(source.join(&name), b"exact body").unwrap();
        let copied = Inventory::snapshot(&source, &owner.path().join("snapshot")).unwrap();
        assert_eq!(copied.entries[0].path, name);
        assert_eq!(copied.read_entry(0).unwrap(), b"exact body");
        copied.verify().unwrap();
    }
    #[test]
    fn missing_extra_and_same_length_edits_fail_verification() {
        for action in 0..3 {
            let root = tempfile::tempdir().unwrap();
            fs::write(root.path().join("body.bin"), b"1234").unwrap();
            let original = Inventory::scan(root.path()).unwrap();
            match action {
                0 => fs::remove_file(root.path().join("body.bin")).unwrap(),
                1 => fs::write(root.path().join("extra.bin"), b"1234").unwrap(),
                _ => fs::write(root.path().join("body.bin"), b"4321").unwrap(),
            }
            assert!(original.verify().is_err());
        }
    }
    #[test]
    fn path_aliases_unsafe_entries_and_duplicate_identities_fail() {
        for unsafe_path in [
            "../outside",
            "/absolute",
            "a//b",
            "a/./b",
            "a\\b",
            "a/NUL",
            "a/.git/HEAD",
        ] {
            assert!(validate_path(unsafe_path).is_err(), "{unsafe_path}");
        }
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("broken.xml"), b"<a>").unwrap();
        assert!(Inventory::scan(root.path()).is_err());
        fs::remove_file(root.path().join("broken.xml")).unwrap();
        for path in ["one.xml", "two.xml"] {
            fs::write(root.path().join(path), b"<MetaDataObject><Catalog uuid='11111111-1111-1111-1111-111111111111'/></MetaDataObject>").unwrap();
        }
        assert!(
            Inventory::scan(root.path())
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
    }
    #[test]
    #[ignore = "actual >256MiB streaming disk IO; F lab only, queue shared heavy FIFO"]
    fn streamed_opaque_above_historical_file_limit_is_not_retained() {
        let lab = PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").expect("F lab"));
        assert!(lab.starts_with(Path::new("F:/ibcmd/lab/07")));
        let owner = tempfile::Builder::new()
            .prefix("disk-large-")
            .tempdir_in(lab)
            .unwrap();
        let source = owner.path().join("source");
        fs::create_dir(&source).unwrap();
        let mut file = File::create(source.join("large.bin")).unwrap();
        let chunk = [0x5a; 64 * 1024];
        let length = 256u64 * 1024 * 1024 + 17;
        for _ in 0..length / chunk.len() as u64 {
            file.write_all(&chunk).unwrap();
        }
        file.write_all(&chunk[..17]).unwrap();
        file.sync_all().unwrap();
        drop(file);
        let inventory = Inventory::snapshot(&source, &owner.path().join("snapshot")).unwrap();
        assert_eq!(inventory.entries.len(), 1);
        assert_eq!(inventory.bytes, length);
        let copied = inventory
            .copy_to(&owner.path().join("published-stage"))
            .unwrap();
        assert_eq!(copied.entries, inventory.entries);
        copied.verify().unwrap();
        // The inventory type holds only path/kind/identity/length/digest; this
        // exercise never calls read_entry or creates a payload-sized Vec.
    }
}
