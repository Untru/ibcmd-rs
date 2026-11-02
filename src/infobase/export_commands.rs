//! Native export queries on the common SQL/filedb export pipeline.
use crate::{
    cli::InfobaseConfigExportArgs,
    mssql_dump::{DumpHooks, FileSink, dump_config_with},
};
use anyhow::{Context, Result};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// Reads only the inventory, retaining the exact shared writer's bytes.
pub fn info(args: &InfobaseConfigExportArgs) -> Result<Vec<u8>> {
    let mut request = args.connection();
    request.need = super::PlatformNeed::Export {
        sqlcmd: args.sqlcmd.as_deref(),
        output_dir: &args.output_dir,
        overwrite: false,
        incremental: true,
    };
    let config =
        super::resolve_export_query_connection(request, "infobase config export info/status")?;
    if args.extension.is_some() {
        anyhow::bail!("export info/status of extensions is not supported yet");
    }
    let dump = super::dump_args(
        &config,
        None,
        args.sqlcmd.as_deref(),
        &args.db_pwd_env,
        PathBuf::from("ibcmd-rs-inventory"),
        Vec::new(),
        config.legacy_source_version()?,
    );
    crate::mssql_dump::read_config_dump_info(&dump)
}

/// Version comparison intentionally differs from editor byte-diff status.
/// A caller can serialize this without knowing the storage backend.
pub fn status(current: &[u8], base: &[u8], short: bool) -> Result<String> {
    use crate::native_export_status::ConfigDumpInfoSnapshot;
    Ok(ConfigDumpInfoSnapshot::parse(current)?
        .compare(&ConfigDumpInfoSnapshot::parse(base)?)
        .text(short))
}

/// Refuse linked path components and publish through a new sibling file.
/// The final rename replaces a destination entry without following it.
pub(crate) fn guarded_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    guarded_write_with(path, |file| file.write_all(bytes))
}

fn guarded_write_with(
    path: &Path,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<()> {
    use std::fs;
    let parent = path.parent().context("output file has no parent")?;
    validate_unlinked_path(path)?;
    fs::create_dir_all(parent)?;
    let staging = parent.join(format!(".ibcmd-export-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staging)?;
    let written = write(&mut file);
    drop(file); // Windows must close the owned staging handle before cleanup.
    let result = written.map_err(anyhow::Error::from).and_then(|()| {
        fs::rename(&staging, path).with_context(|| format!("failed to publish {}", path.display()))
    });
    if result.is_err()
        && let Err(cleanup) = fs::remove_file(&staging)
    {
        return result.with_context(|| {
            format!(
                "could not remove staging file {}: {cleanup}",
                staging.display()
            )
        });
    }
    result
}

fn validate_unlinked_path(path: &Path) -> Result<()> {
    use std::fs;
    for component in path.ancestors() {
        match fs::symlink_metadata(component) {
            Ok(metadata) => {
                #[cfg(windows)]
                let linked = {
                    use std::os::windows::fs::MetadataExt;
                    metadata.file_attributes() & 0x400 != 0
                };
                #[cfg(not(windows))]
                let linked = false;
                if linked || metadata.file_type().is_symlink() {
                    anyhow::bail!("linked output path is not allowed: {}", component.display());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

// Conversion may withhold an unsupported descriptor or resource, or fail
// after writing another file. Keep the whole selection private until its
// completeness reports have been checked; retain the writer's disk budget.
struct ObjectSink {
    root: PathBuf,
    staging: PathBuf,
    files: Mutex<BTreeSet<PathBuf>>,
}
impl ObjectSink {
    fn new(root: PathBuf) -> Result<Self> {
        let staging =
            std::env::temp_dir().join(format!("ibcmd-selected-export-{}", uuid::Uuid::new_v4()));
        let builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&staging)?;
        let mut sink = Self {
            root,
            staging,
            files: Mutex::new(BTreeSet::new()),
        };
        // Only our private scratch is canonicalized: system temp paths may
        // contain an alias. The caller's destination retains its link checks.
        sink.staging = std::fs::canonicalize(&sink.staging)?;
        Ok(sink)
    }

    fn publish(&self) -> Result<()> {
        let files = self.files.lock().unwrap_or_else(|error| error.into_inner());
        // Check the complete set before replacing even its first file.
        for relative in files.iter() {
            let path = self.root.join(relative);
            validate_unlinked_path(&path)?;
            if path.is_dir() {
                anyhow::bail!("object output file is a directory: {}", path.display());
            }
            for parent in path.parent().into_iter().flat_map(Path::ancestors) {
                if parent.exists() && !parent.is_dir() {
                    anyhow::bail!(
                        "object output parent is not a directory: {}",
                        parent.display()
                    );
                }
            }
        }
        for relative in files.iter() {
            crate::cancel::check()?;
            let mut source = std::fs::File::open(self.staging.join(relative))?;
            guarded_write_with(&self.root.join(relative), |file| {
                std::io::copy(&mut source, file).map(|_| ())
            })?;
        }
        Ok(())
    }
}
impl Drop for ObjectSink {
    fn drop(&mut self) {
        // This path was created by this request, never supplied by the caller.
        let _ = std::fs::remove_dir_all(&self.staging);
    }
}
impl FileSink for ObjectSink {
    fn accept(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        let relative = path
            .strip_prefix(&self.root)
            .context("object output escaped its root")?;
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            anyhow::bail!("object output contains an unsafe path component");
        }
        guarded_write(&self.staging.join(relative), bytes)?;
        self.files
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(relative.to_path_buf());
        Ok(())
    }
}

pub fn objects(args: &InfobaseConfigExportArgs, names: &[String], recursive: bool) -> Result<()> {
    anyhow::ensure!(
        !args.output_dir.as_os_str().is_empty(),
        "object output directory must not be empty"
    );
    // Retain links in the caller's spelling so the strict object-output
    // contract can reject them before reading the database or publishing.
    let root = if args.output_dir.is_absolute() {
        args.output_dir.clone()
    } else {
        std::env::current_dir()?.join(&args.output_dir)
    };
    validate_unlinked_path(&root)?;
    let mut request = args.connection();
    request.need = super::PlatformNeed::Export {
        sqlcmd: args.sqlcmd.as_deref(),
        output_dir: &args.output_dir,
        overwrite: false,
        incremental: true,
    };
    let config = super::resolve_export_query_connection(request, "infobase config export objects")?;
    if args.extension.is_some() {
        anyhow::bail!("export objects of extensions is not supported yet");
    }
    if names.is_empty() {
        anyhow::bail!("no metadata objects selected");
    }
    let mut dump = super::dump_args(
        &config,
        None,
        args.sqlcmd.as_deref(),
        &args.db_pwd_env,
        root.clone(),
        Vec::new(),
        config.legacy_source_version()?,
    );
    dump.objects = names.to_vec();
    let sink = Arc::new(ObjectSink::new(root)?);
    let report = dump_config_with(
        &dump,
        DumpHooks {
            rows_snapshot: None,
            sql: None,
            sink: Some(sink.clone()),
            object_recursion: Some(recursive),
            select_objects: Some(crate::stored_objects::stored_rows_selected),
            native_versions: false,
        },
    )?;
    report.ensure_selected_complete()?;
    sink.publish()
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    use std::{
        fs,
        io::{self, Write},
    };

    fn root() -> crate::test_scratch::TestScratch {
        let root = std::env::temp_dir().join(format!("export-write-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        crate::test_scratch::TestScratch::adopt(fs::canonicalize(root).unwrap())
    }

    #[test]
    fn empty_object_output_is_refused_before_connection_resolution() {
        use std::ffi::OsString;
        let db = std::env::temp_dir().join(format!("missing-db-{}", uuid::Uuid::new_v4()));
        let _scratch = crate::test_scratch::TestScratch::adopt(&db);
        let words = vec![
            "config".into(),
            "export".into(),
            "objects".into(),
            "--out=unused".into(),
            OsString::from(format!("--db-path={}", db.display())),
            "--platform=8.5.1".into(),
            "CommonModule.NoSuchObject".into(),
        ];
        let crate::dropin::Invocation::ExportQuery(request) =
            crate::dropin::parse::parse_infobase(&words).unwrap()
        else {
            panic!("expected query")
        };
        let mut args = crate::dropin::export_args(&request.export);
        args.output_dir.clear();
        let error = objects(&args, &request.objects, false).unwrap_err();
        assert_eq!(
            error.to_string(),
            "object output directory must not be empty"
        );
        assert!(!db.exists());
    }

    #[test]
    fn staged_objects_preflight_every_destination_before_replacing_any_file() {
        let root = root();
        let first = root.join("a.txt");
        fs::write(&first, b"original").unwrap();
        let blocked = root.join("z.txt");
        fs::create_dir(&blocked).unwrap();
        let sink = ObjectSink::new(root.to_path_buf()).unwrap();
        let staging = sink.staging.clone();
        sink.accept(&first, b"replacement").unwrap();
        sink.accept(&blocked, b"replacement").unwrap();
        assert_eq!(fs::read(&first).unwrap(), b"original");
        let error = sink.publish().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("object output file is a directory")
        );
        assert_eq!(fs::read(&first).unwrap(), b"original");
        assert!(blocked.is_dir());
        drop(sink);
        assert!(!staging.exists());
    }

    #[test]
    fn partial_write_failure_removes_owned_staging_and_preserves_destination() {
        let root = root();
        let target = root.join("status.txt");
        fs::write(&target, b"original").unwrap();
        let error = guarded_write_with(&target, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("injected write failure"))
        })
        .unwrap_err();
        assert!(format!("{error:#}").contains("injected write failure"));
        assert_eq!(fs::read(&target).unwrap(), b"original");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }

    #[test]
    fn failed_rename_removes_owned_staging_and_preserves_directory() {
        let root = root();
        let target = root.join("directory");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep"), b"original").unwrap();
        let error = guarded_write(&target, b"replacement").unwrap_err();
        assert!(format!("{error:#}").contains("failed to publish"));
        assert_eq!(fs::read(target.join("keep")).unwrap(), b"original");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }
}
