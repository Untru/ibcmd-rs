use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use flate2::read::DeflateDecoder;
use serde::{Deserialize, Serialize};

use crate::cli::MssqlDumpConfigArgs;
use crate::module_blob::unpack_module_blob_text;

#[derive(Debug, Serialize)]
pub struct MssqlDumpConfigReport {
    pub database: String,
    pub output_dir: PathBuf,
    pub tables: Vec<MssqlDumpedTableReport>,
    pub total_rows: usize,
    pub total_binary_bytes: usize,
    pub total_inflated_rows: usize,
    pub total_module_text_rows: usize,
}

#[derive(Debug, Serialize)]
pub struct MssqlDumpedTableReport {
    pub table: String,
    pub rows: usize,
    pub binary_bytes: usize,
    pub inflated_rows: usize,
    pub module_text_rows: usize,
}

#[derive(Debug, Serialize)]
struct MssqlDumpManifest {
    database: String,
    tables: Vec<MssqlDumpTableManifest>,
}

#[derive(Debug, Serialize)]
struct MssqlDumpTableManifest {
    table: String,
    rows: Vec<MssqlDumpRowManifest>,
}

#[derive(Debug, Serialize)]
struct MssqlDumpRowManifest {
    file_name: String,
    part_no: i32,
    data_size: i64,
    binary_bytes: usize,
    binary_path: String,
    inflated_path: Option<String>,
    module_text_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigRow {
    #[serde(rename = "file_name")]
    file_name: String,
    #[serde(rename = "part_no")]
    part_no: i32,
    #[serde(rename = "data_size")]
    data_size: i64,
    #[serde(rename = "binary_hex")]
    binary_hex: String,
}

pub fn dump_config(args: &MssqlDumpConfigArgs) -> Result<MssqlDumpConfigReport> {
    prepare_output_dir(&args.output_dir, args.overwrite)?;

    let mut table_names = vec!["Config"];
    if args.include_config_save {
        table_names.push("ConfigSave");
    }

    let mut reports = Vec::new();
    let mut manifest_tables = Vec::new();
    for table in table_names {
        let rows = fetch_rows(&args.sqlcmd, &args.server, &args.database, table)?;
        let dumped = dump_table_rows(
            &args.output_dir,
            table,
            rows,
            args.inflate,
            args.extract_module_text,
        )?;
        reports.push(MssqlDumpedTableReport {
            table: table.to_string(),
            rows: dumped.rows.len(),
            binary_bytes: dumped.binary_bytes,
            inflated_rows: dumped.inflated_rows,
            module_text_rows: dumped.module_text_rows,
        });
        manifest_tables.push(MssqlDumpTableManifest {
            table: table.to_string(),
            rows: dumped.rows,
        });
    }

    let manifest = MssqlDumpManifest {
        database: args.database.clone(),
        tables: manifest_tables,
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    fs::write(args.output_dir.join("manifest.json"), manifest_json).with_context(|| {
        format!(
            "failed to write {}",
            args.output_dir.join("manifest.json").display()
        )
    })?;

    Ok(MssqlDumpConfigReport {
        database: args.database.clone(),
        output_dir: args.output_dir.clone(),
        total_rows: reports.iter().map(|table| table.rows).sum(),
        total_binary_bytes: reports.iter().map(|table| table.binary_bytes).sum(),
        total_inflated_rows: reports.iter().map(|table| table.inflated_rows).sum(),
        total_module_text_rows: reports.iter().map(|table| table.module_text_rows).sum(),
        tables: reports,
    })
}

struct DumpedTable {
    rows: Vec<MssqlDumpRowManifest>,
    binary_bytes: usize,
    inflated_rows: usize,
    module_text_rows: usize,
}

fn dump_table_rows(
    output_dir: &Path,
    table: &str,
    rows: Vec<ConfigRow>,
    inflate: bool,
    extract_module_text: bool,
) -> Result<DumpedTable> {
    let table_dir = output_dir.join(table);
    fs::create_dir_all(&table_dir)
        .with_context(|| format!("failed to create {}", table_dir.display()))?;
    let inflated_dir = output_dir.join(format!("{table}_inflated"));
    if inflate {
        fs::create_dir_all(&inflated_dir)
            .with_context(|| format!("failed to create {}", inflated_dir.display()))?;
    }
    let module_text_dir = output_dir.join(format!("{table}_module_text"));
    if extract_module_text {
        fs::create_dir_all(&module_text_dir)
            .with_context(|| format!("failed to create {}", module_text_dir.display()))?;
    }

    let mut manifests = Vec::new();
    let mut binary_bytes = 0;
    let mut inflated_rows = 0;
    let mut module_text_rows = 0;
    for row in rows {
        let bytes = decode_hex(&row.binary_hex)
            .with_context(|| format!("failed to decode {} row {}", table, row.file_name))?;
        if bytes.len() != row.data_size as usize {
            bail!(
                "{} row {} DataSize {} does not match BinaryData length {}",
                table,
                row.file_name,
                row.data_size,
                bytes.len()
            );
        }
        binary_bytes += bytes.len();

        let safe_name = safe_storage_file_name(&row.file_name, row.part_no);
        let binary_relative = PathBuf::from(table).join(format!("{safe_name}.bin"));
        let binary_path = output_dir.join(&binary_relative);
        fs::write(&binary_path, &bytes)
            .with_context(|| format!("failed to write {}", binary_path.display()))?;

        let inflated_relative = if inflate {
            match inflate_raw_deflate(&bytes) {
                Ok(inflated) => {
                    let relative =
                        PathBuf::from(format!("{table}_inflated")).join(format!("{safe_name}.txt"));
                    let path = output_dir.join(&relative);
                    fs::write(&path, inflated)
                        .with_context(|| format!("failed to write {}", path.display()))?;
                    inflated_rows += 1;
                    Some(relative.to_string_lossy().replace('\\', "/"))
                }
                Err(_) => None,
            }
        } else {
            None
        };

        let module_text_relative = if extract_module_text {
            match unpack_module_blob_text(&bytes) {
                Ok(text) => {
                    let relative = PathBuf::from(format!("{table}_module_text"))
                        .join(format!("{safe_name}.bsl"));
                    let path = output_dir.join(&relative);
                    fs::write(&path, text)
                        .with_context(|| format!("failed to write {}", path.display()))?;
                    module_text_rows += 1;
                    Some(relative.to_string_lossy().replace('\\', "/"))
                }
                Err(_) => None,
            }
        } else {
            None
        };

        manifests.push(MssqlDumpRowManifest {
            file_name: row.file_name,
            part_no: row.part_no,
            data_size: row.data_size,
            binary_bytes: bytes.len(),
            binary_path: binary_relative.to_string_lossy().replace('\\', "/"),
            inflated_path: inflated_relative,
            module_text_path: module_text_relative,
        });
    }

    Ok(DumpedTable {
        rows: manifests,
        binary_bytes,
        inflated_rows,
        module_text_rows,
    })
}

fn fetch_rows(sqlcmd: &Path, server: &str, database: &str, table: &str) -> Result<Vec<ConfigRow>> {
    let sql = format!(
        "SET NOCOUNT ON; USE {db};\n\
         SELECT FileName AS file_name,\n\
                PartNo AS part_no,\n\
                DataSize AS data_size,\n\
                CONVERT(varchar(max), BinaryData, 2) AS binary_hex\n\
         FROM {table}\n\
         ORDER BY FileName, PartNo\n\
         FOR JSON PATH;",
        db = quote_ident(database),
        table = quote_ident(table),
    );
    let stdout = run_sql_capture(sqlcmd, server, &sql)?;
    let json = extract_json_array(&stdout, &format!("dump {table} rows from {database}"))?;
    serde_json::from_str(&json)
        .with_context(|| format!("failed to parse {table} rows JSON for {database}"))
}

fn run_sql_capture(sqlcmd: &Path, server: &str, sql: &str) -> Result<String> {
    let output = Command::new(sqlcmd)
        .arg("-C")
        .arg("-S")
        .arg(server)
        .arg("-W")
        .arg("-Q")
        .arg(sql)
        .output()
        .with_context(|| format!("failed to run {}", sqlcmd.display()))?;
    if !output.status.success() {
        bail!(
            "sqlcmd failed with exit code {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn prepare_output_dir(path: &Path, overwrite: bool) -> Result<()> {
    if path.exists() {
        if !path.is_dir() {
            bail!(
                "output path exists and is not a directory: {}",
                path.display()
            );
        }
        if fs::read_dir(path)?.next().is_some() && !overwrite {
            bail!(
                "output directory is not empty: {}. Pass --overwrite to replace it",
                path.display()
            );
        }
        if overwrite {
            clear_directory(path)?;
        }
    } else {
        fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    }
    Ok(())
}

fn clear_directory(path: &Path) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn inflate_raw_deflate(input: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = DeflateDecoder::new(input);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .context("failed to inflate raw deflate blob")?;
    Ok(output)
}

fn decode_hex(hex: &str) -> Result<Vec<u8>> {
    let hex = hex.trim().strip_prefix("0x").unwrap_or(hex.trim());
    if !hex.len().is_multiple_of(2) {
        return Err(anyhow!("hex string has odd length"));
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&hex[index..index + 2], 16)
                .with_context(|| format!("invalid hex byte at offset {index}"))
        })
        .collect()
}

fn extract_json_array(stdout: &str, context: &str) -> Result<String> {
    let start = stdout
        .find('[')
        .ok_or_else(|| anyhow!("{context}: sqlcmd output does not contain JSON array"))?;
    let end = stdout
        .rfind(']')
        .ok_or_else(|| anyhow!("{context}: sqlcmd output does not contain JSON array end"))?;
    if end < start {
        return Err(anyhow!("{context}: invalid JSON array boundaries"));
    }
    Ok(stdout[start..=end].to_string())
}

fn quote_ident(value: &str) -> String {
    format!("[{}]", value.replace(']', "]]"))
}

fn safe_storage_file_name(file_name: &str, part_no: i32) -> String {
    let mut safe = String::with_capacity(file_name.len() + 16);
    for ch in file_name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            safe.push(ch);
        } else {
            safe.push('_');
        }
    }
    if safe.is_empty() {
        safe.push_str("row");
    }
    format!("{safe}__part{part_no}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module_blob::pack_module_blob_bytes;

    #[test]
    fn decodes_plain_hex_and_sql_hex() {
        assert_eq!(decode_hex("efbbbf").unwrap(), vec![0xef, 0xbb, 0xbf]);
        assert_eq!(decode_hex("0x0102ff").unwrap(), vec![1, 2, 255]);
    }

    #[test]
    fn rejects_odd_hex_length() {
        assert!(decode_hex("abc").is_err());
    }

    #[test]
    fn extracts_json_array_from_sqlcmd_noise() {
        let stdout = "Changed database context.\r\n[{\"a\":1}]\r\n(1 row affected)";
        assert_eq!(extract_json_array(stdout, "test").unwrap(), r#"[{"a":1}]"#);
    }

    #[test]
    fn sanitizes_storage_file_names() {
        assert_eq!(
            safe_storage_file_name("abc/def:ghi", 0),
            "abc_def_ghi__part0"
        );
        assert_eq!(safe_storage_file_name("", 2), "row__part2");
    }

    #[test]
    fn extracts_module_text_from_dumped_rows() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-dump-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        let text = b"Procedure Run()\r\nEndProcedure\r\n";
        let packed = pack_module_blob_bytes(text, None, None).unwrap();
        let row = ConfigRow {
            file_name: "module-id.0".to_string(),
            part_no: 0,
            data_size: packed.blob.len() as i64,
            binary_hex: encode_hex_for_test(&packed.blob),
        };

        let dumped = dump_table_rows(&root, "Config", vec![row], false, true).unwrap();

        assert_eq!(dumped.module_text_rows, 1);
        let module_text_path = dumped.rows[0].module_text_path.as_ref().unwrap();
        let written = fs::read(root.join(module_text_path)).unwrap();
        assert_eq!(written, text);

        let _ = fs::remove_dir_all(root);
    }

    fn encode_hex_for_test(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
