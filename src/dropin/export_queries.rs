//! Native query stdout/file protocols, measured on 8.5.1.1592.
use super::{
    EXIT_FAILED, EXIT_UNSUPPORTED, export_args,
    parse::{ExportQueryKind, ExportQueryRequest},
};
use crate::infobase::{UnsupportedExportQuery, export_commands};
use anyhow::Result;
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

pub(super) fn run(mut request: ExportQueryRequest) -> i32 {
    let kind = request.kind;
    let command = match kind {
        ExportQueryKind::Info => "infobase config export info",
        ExportQueryKind::Status => "infobase config export status",
        ExportQueryKind::Objects => "infobase config export objects",
    };
    let title = match kind {
        ExportQueryKind::Info => {
            "Получение информации о состоянии конфигурации информационной базы"
        }
        ExportQueryKind::Status => {
            "Вычисление изменений конфигурации информационной базы относительно ранее сохраненного состояния"
        }
        ExportQueryKind::Objects => "Экспорт объектов конфигурации в XML",
    };
    let logged = request.out.is_some() || kind == ExportQueryKind::Objects;
    if logged {
        println!("[INFO] {title}...");
    }
    let result = (|| -> Result<()> {
        super::read_requested_password(&mut request.export.common).map_err(anyhow::Error::msg)?;
        if let Some(threads) = request.export.threads {
            crate::parallel::request_workers(threads);
        }
        crate::mssql_dump::model_export::set_quiet_summaries(true);
        let args = export_args(&request.export);
        if kind == ExportQueryKind::Objects {
            return export_commands::objects(&args, &request.objects, request.recursive);
        }
        let current = export_commands::info(&args)?;
        let bytes = if kind == ExportQueryKind::Info {
            current
        } else {
            let base = fs::read(
                request
                    .export
                    .base
                    .as_ref()
                    .expect("parser requires --base"),
            )?;
            let text = export_commands::status(&current, &base, request.short)?;
            let mut bytes = if request.out.is_some() {
                vec![0xef, 0xbb, 0xbf]
            } else {
                Vec::new()
            };
            bytes.extend_from_slice(text.as_bytes());
            bytes
        };
        if let Some(out) = &request.out {
            if kind == ExportQueryKind::Info {
                anyhow::ensure!(
                    out.is_dir(),
                    "output directory does not exist: {}",
                    out.display()
                );
                let path = query_output_path(&out.join("ConfigDumpInfo.xml"))?;
                export_commands::guarded_write(&path, &bytes)?;
            } else {
                let path = query_output_path(out)?;
                export_commands::guarded_write(&path, &bytes).map_err(|error| {
                    if error.downcast_ref::<std::io::Error>().is_some() && path.is_dir() {
                        anyhow::anyhow!("Операция невозможна, при вычислении изменений конфигурации информационной базы относительно ранее сохраненного состояния обнаружены ошибки: Ошибка приемника потока")
                    } else {
                        error
                    }
                })?;
            }
        } else {
            let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
            std::io::stdout().lock().write_all(bytes)?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            if let Some(path) = request.export.common.report.as_deref() {
                if let Err(error) =
                    super::write_json(path, &serde_json::json!({"operation":command,"ok":true}))
                {
                    eprintln!("[ERROR] {error:#}");
                    return EXIT_FAILED;
                }
            }
            if logged {
                let ended = if kind != ExportQueryKind::Objects {
                    "завершено."
                } else {
                    "завершен"
                };
                println!("[INFO] {title} успешно {ended}");
            }
            0
        }
        Err(error) => {
            let exit_code = if error.downcast_ref::<UnsupportedExportQuery>().is_some() {
                EXIT_UNSUPPORTED
            } else {
                EXIT_FAILED
            };
            let message = if let Some(unknown) =
                error.downcast_ref::<crate::stored_objects::UnknownObject>()
            {
                format!(
                    "Операция невозможна, при выполнении экспорта объектов конфигурации в XML обнаружены ошибки: Объект метаданных {} не существует в конфигурации.",
                    unknown.0
                )
            } else {
                format!("{error:#}")
            };
            eprintln!("[ERROR] {message}");
            if let Some(path) = request.export.common.report.as_deref() {
                let failure = super::FailureReport {
                    operation: command,
                    ok: false,
                    error: message,
                };
                if let Err(report_error) = super::write_json(path, &failure) {
                    eprintln!("[ERROR] {report_error:#}");
                }
            }
            exit_code
        }
    }
}

/// A query destination is explicitly selected by the caller. Resolve directory
/// aliases (including macOS /tmp) but never follow the final file entry. Objects
/// retain their separate, strict root-containment checks.
fn query_output_path(path: &Path) -> Result<PathBuf> {
    anyhow::ensure!(
        !path
            .components()
            .any(|part| matches!(part, Component::ParentDir)),
        "parent traversal in output path is not allowed"
    );
    let spelling = path.as_os_str().as_encoded_bytes();
    anyhow::ensure!(
        spelling != b"."
            && !spelling.ends_with(b"/.")
            && !(cfg!(windows) && spelling.ends_with(b"\\.")),
        "output file path ends with a directory component"
    );
    let last = spelling.last().copied();
    anyhow::ensure!(
        last != Some(b'/') && !(cfg!(windows) && last == Some(b'\\')),
        "output file path ends with a directory separator"
    );
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let leaf = absolute
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("output file has no name"))?;
    let mut parent = absolute
        .parent()
        .ok_or_else(|| anyhow::anyhow!("output file has no parent"))?;
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(parent) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    parent
                        .file_name()
                        .ok_or_else(|| anyhow::anyhow!("output parent has no name"))?,
                );
                parent = parent
                    .parent()
                    .ok_or_else(|| anyhow::anyhow!("output parent has no ancestor"))?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    let mut resolved = fs::canonicalize(parent)?;
    for part in missing.into_iter().rev() {
        resolved.push(part);
    }
    resolved.push(leaf);
    Ok(resolved)
}
