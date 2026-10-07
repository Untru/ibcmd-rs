//! Compact main-activation recovery files; this module executes no SQL.
//! The synchronized content-addressed pack is published before its manifest.
//! Historical LIVE manifests retain their separate format and token encoding.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::mssql_main_activation::{
    MAX_PLAN_BYTES, MAX_ROW_BYTES, MAX_ROWS, MainActivationMode, MainActivationRecoverySnapshot,
    MainStorageRow,
};

const MAX_MANIFEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_PACK_BYTES: usize = 2 * MAX_PLAN_BYTES + 2 * MAX_ROW_BYTES;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: u32,
    integrity_sha256: String,
    payload: Payload,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    database: String,
    mode: MainActivationMode,
    recovery_token: String,
    old_generation: String,
    new_generation: String,
    pack_file: String,
    pack_bytes: usize,
    pack_sha256: String,
    overwritten_config_rows: Vec<PackedRow>,
    retained_config_rows: Vec<PackedRow>,
    prior_config_dynamically_updated: Option<PackedRow>,
    prior_params_dynamically_updated: Option<PackedRow>,
    staged_rows: Vec<PackedRow>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackedRow {
    file_name: String,
    part_no: i32,
    creation: String,
    modified: String,
    attributes: i32,
    data_size: u64,
    offset: usize,
    length: usize,
    sha256: String,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn snapshot_token(snapshot: &MainActivationRecoverySnapshot) -> Result<String> {
    Ok(digest(&serde_json::to_vec(snapshot)?))
}

fn validate_database(database: &str) -> Result<()> {
    ensure!(
        !database.is_empty()
            && database.encode_utf16().count() <= 128
            && !database.chars().any(char::is_control),
        "invalid recovery database name"
    );
    Ok(())
}

fn validate_snapshot(snapshot: &MainActivationRecoverySnapshot) -> Result<()> {
    for generation in [&snapshot.old_generation, &snapshot.new_generation] {
        uuid::Uuid::parse_str(generation).context("invalid recovery generation")?;
    }
    let mut names = HashSet::new();
    let mut before_bytes = 0usize;
    let before = snapshot
        .overwritten_config_rows
        .iter()
        .chain(&snapshot.retained_config_rows);
    ensure!(
        before.clone().count() <= MAX_ROWS,
        "too many recovery Config preimages"
    );
    for row in before {
        validate_row(row)?;
        ensure!(
            names.insert((row.file_name.to_lowercase(), row.part_no)),
            "duplicate recovery Config preimage"
        );
        before_bytes += row.binary_data.len();
    }
    ensure!(
        before_bytes <= MAX_PLAN_BYTES,
        "recovery Config preimages exceed byte budget"
    );
    names.clear();
    let mut staged_bytes = 0usize;
    ensure!(
        snapshot.staged_rows.len() <= MAX_ROWS,
        "too many recovery staged rows"
    );
    for row in &snapshot.staged_rows {
        validate_row(row)?;
        ensure!(
            names.insert((row.file_name.to_lowercase(), row.part_no)),
            "duplicate recovery staged row"
        );
        staged_bytes += row.binary_data.len();
    }
    ensure!(
        staged_bytes <= MAX_PLAN_BYTES,
        "recovery stage exceeds byte budget"
    );
    for row in [
        &snapshot.prior_config_dynamically_updated,
        &snapshot.prior_params_dynamically_updated,
    ]
    .into_iter()
    .flatten()
    {
        validate_row(row)?;
        ensure!(
            row.file_name == "DynamicallyUpdated",
            "invalid recovery marker name"
        );
    }
    Ok(())
}

fn validate_row(row: &MainStorageRow) -> Result<()> {
    ensure!(
        !row.file_name.is_empty()
            && row.file_name.encode_utf16().count() <= 128
            && !row.file_name.chars().any(char::is_control),
        "invalid recovery row name"
    );
    ensure!(
        row.part_no == 0
            && row.data_size == row.binary_data.len() as u64
            && row.binary_data.len() <= MAX_ROW_BYTES,
        "invalid recovery row size/part"
    );
    ensure!(
        row.creation.len() <= 27 && row.modified.len() <= 27,
        "invalid recovery row dates"
    );
    Ok(())
}

fn pack_row(row: &MainStorageRow, pack: &mut Vec<u8>) -> PackedRow {
    let offset = pack.len();
    pack.extend_from_slice(&row.binary_data);
    PackedRow {
        file_name: row.file_name.clone(),
        part_no: row.part_no,
        creation: row.creation.clone(),
        modified: row.modified.clone(),
        attributes: row.attributes,
        data_size: row.data_size,
        offset,
        length: row.binary_data.len(),
        sha256: digest(&row.binary_data),
    }
}

fn encode(
    database: &str,
    mode: MainActivationMode,
    snapshot: &MainActivationRecoverySnapshot,
    token: &str,
) -> Result<(Manifest, Vec<u8>)> {
    validate_snapshot(snapshot)?;
    validate_database(database)?;
    ensure!(
        snapshot_token(snapshot)?.eq_ignore_ascii_case(token),
        "recovery token differs from snapshot"
    );
    let mut pack = Vec::new();
    let overwritten_config_rows = snapshot
        .overwritten_config_rows
        .iter()
        .map(|row| pack_row(row, &mut pack))
        .collect();
    let retained_config_rows = snapshot
        .retained_config_rows
        .iter()
        .map(|row| pack_row(row, &mut pack))
        .collect();
    let prior_config_dynamically_updated = snapshot
        .prior_config_dynamically_updated
        .as_ref()
        .map(|row| pack_row(row, &mut pack));
    let prior_params_dynamically_updated = snapshot
        .prior_params_dynamically_updated
        .as_ref()
        .map(|row| pack_row(row, &mut pack));
    let staged_rows = snapshot
        .staged_rows
        .iter()
        .map(|row| pack_row(row, &mut pack))
        .collect();
    ensure!(
        pack.len() <= MAX_PACK_BYTES,
        "recovery pack exceeds byte budget"
    );
    let pack_sha256 = digest(&pack);
    let payload = Payload {
        database: database.to_owned(),
        mode,
        recovery_token: token.to_owned(),
        old_generation: snapshot.old_generation.clone(),
        new_generation: snapshot.new_generation.clone(),
        pack_file: format!("ibcmd-recovery-{pack_sha256}.pack"),
        pack_bytes: pack.len(),
        pack_sha256,
        overwritten_config_rows,
        retained_config_rows,
        prior_config_dynamically_updated,
        prior_params_dynamically_updated,
        staged_rows,
    };
    let integrity_sha256 = digest(&serde_json::to_vec(&payload)?);
    Ok((
        Manifest {
            format: 2,
            integrity_sha256,
            payload,
        },
        pack,
    ))
}

/// Writes an immutable pair and verifies its readback before the caller can run SQL.
/// A failed manifest publication may leave an unreferenced complete pack; it is
/// never mistaken for a committed operation and is not automatically removed.
pub(crate) fn write(
    path: &Path,
    database: &str,
    mode: MainActivationMode,
    snapshot: &MainActivationRecoverySnapshot,
    token: &str,
) -> Result<()> {
    let (manifest, pack) = encode(database, mode, snapshot, token)?;
    let bytes = serde_json::to_vec_pretty(&manifest)?;
    ensure!(
        bytes.len() <= MAX_MANIFEST_BYTES,
        "recovery manifest exceeds byte budget"
    );
    let pack_path = sibling(path, &manifest.payload.pack_file)?;
    ensure!(
        path.file_name()
            .and_then(|name| name.to_str())
            .is_none_or(|name| !name.eq_ignore_ascii_case(&manifest.payload.pack_file)),
        "recovery manifest cannot use its pack filename"
    );
    crate::mssql_artifact::write_new_or_identical(&pack_path, &pack)
        .context("publish recovery pack")?;
    crate::mssql_artifact::write_new_or_identical(path, &bytes)
        .context("publish recovery manifest")?;
    let (saved_database, saved_mode, saved_snapshot) = read(path)?;
    ensure!(
        saved_database == database && saved_mode == mode && saved_snapshot == *snapshot,
        "published recovery readback differs"
    );
    Ok(())
}

pub(crate) fn sibling(manifest: &Path, file_name: &str) -> Result<PathBuf> {
    ensure!(
        !file_name.is_empty()
            && !file_name.contains(['/', '\\', ':'])
            && !file_name.chars().any(char::is_control)
            && file_name != "."
            && file_name != "..",
        "recovery pack must be an adjacent basename"
    );
    Ok(manifest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(file_name))
}

pub(crate) fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "recovery file must be a regular file"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x0400 == 0,
            "recovery file cannot be a reparse point"
        );
    }
    ensure!(
        metadata.len() <= limit as u64,
        "recovery file exceeds byte budget"
    );
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= limit,
        "recovery file grew beyond byte budget"
    );
    Ok(bytes)
}

fn unpack_row(row: &PackedRow, pack: &[u8], cursor: &mut usize) -> Result<MainStorageRow> {
    ensure!(
        row.offset == *cursor && row.length <= MAX_ROW_BYTES && row.data_size == row.length as u64,
        "invalid recovery pack row range/size"
    );
    let end = row
        .offset
        .checked_add(row.length)
        .context("recovery row range overflow")?;
    let bytes = pack
        .get(row.offset..end)
        .context("recovery row range outside pack")?;
    ensure!(digest(bytes) == row.sha256, "recovery row digest mismatch");
    *cursor = end;
    Ok(MainStorageRow {
        file_name: row.file_name.clone(),
        part_no: row.part_no,
        creation: row.creation.clone(),
        modified: row.modified.clone(),
        attributes: row.attributes,
        data_size: row.data_size,
        binary_data: bytes.to_vec(),
    })
}

/// Verifies representation integrity, not database ownership or a safe undo.
pub(crate) fn read(
    path: &Path,
) -> Result<(String, MainActivationMode, MainActivationRecoverySnapshot)> {
    let manifest: Manifest = serde_json::from_slice(&read_bounded(path, MAX_MANIFEST_BYTES)?)?;
    ensure!(manifest.format == 2, "unsupported compact recovery format");
    let p = manifest.payload;
    validate_database(&p.database)?;
    ensure!(
        digest(&serde_json::to_vec(&p)?) == manifest.integrity_sha256,
        "recovery manifest integrity mismatch"
    );
    ensure!(
        p.pack_bytes <= MAX_PACK_BYTES
            && p.overwritten_config_rows.len() + p.retained_config_rows.len() <= MAX_ROWS
            && p.staged_rows.len() <= MAX_ROWS,
        "recovery inventory exceeds budget"
    );
    ensure!(
        p.pack_file == format!("ibcmd-recovery-{}.pack", p.pack_sha256),
        "recovery pack name differs from digest"
    );
    let pack_path = sibling(path, &p.pack_file)?;
    let pack = read_bounded(&pack_path, p.pack_bytes)
        .with_context(|| format!("read compact recovery pack {}", pack_path.display()))?;
    ensure!(
        pack.len() == p.pack_bytes && digest(&pack) == p.pack_sha256,
        "recovery pack length/digest mismatch"
    );
    let mut cursor = 0;
    let overwritten_config_rows = p
        .overwritten_config_rows
        .iter()
        .map(|row| unpack_row(row, &pack, &mut cursor))
        .collect::<Result<_>>()?;
    let retained_config_rows = p
        .retained_config_rows
        .iter()
        .map(|row| unpack_row(row, &pack, &mut cursor))
        .collect::<Result<_>>()?;
    let prior_config_dynamically_updated = p
        .prior_config_dynamically_updated
        .as_ref()
        .map(|row| unpack_row(row, &pack, &mut cursor))
        .transpose()?;
    let prior_params_dynamically_updated = p
        .prior_params_dynamically_updated
        .as_ref()
        .map(|row| unpack_row(row, &pack, &mut cursor))
        .transpose()?;
    let staged_rows = p
        .staged_rows
        .iter()
        .map(|row| unpack_row(row, &pack, &mut cursor))
        .collect::<Result<_>>()?;
    ensure!(
        cursor == pack.len(),
        "recovery pack contains unreferenced bytes"
    );
    let snapshot = MainActivationRecoverySnapshot {
        old_generation: p.old_generation,
        new_generation: p.new_generation,
        overwritten_config_rows,
        retained_config_rows,
        prior_config_dynamically_updated,
        prior_params_dynamically_updated,
        staged_rows,
    };
    validate_snapshot(&snapshot)?;
    ensure!(
        snapshot_token(&snapshot)?.eq_ignore_ascii_case(&p.recovery_token),
        "compact recovery token differs from snapshot"
    );
    Ok((p.database, p.mode, snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> MainActivationRecoverySnapshot {
        let row = |name: &str, bytes: &[u8]| MainStorageRow {
            file_name: name.into(),
            part_no: 0,
            creation: "4026-10-01 12:00:00.003".into(),
            modified: "4026-10-01 12:00:01.007".into(),
            attributes: 7,
            data_size: bytes.len() as u64,
            binary_data: bytes.to_vec(),
        };
        MainActivationRecoverySnapshot {
            old_generation: uuid::Uuid::nil().to_string(),
            new_generation: uuid::Uuid::from_u128(1).to_string(),
            overwritten_config_rows: vec![row("root", &[0, 255, 0])],
            retained_config_rows: vec![row("versions", b"retained")],
            prior_config_dynamically_updated: Some(row("DynamicallyUpdated", b"config-marker")),
            prior_params_dynamically_updated: Some(row("DynamicallyUpdated", b"params-marker")),
            staged_rows: vec![row("root", b"next")],
        }
    }

    struct Directory(PathBuf);

    impl Directory {
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            let root = fs::canonicalize(std::env::temp_dir()).unwrap();
            let target = fs::canonicalize(&self.0).unwrap();
            assert_eq!(target.parent(), Some(root.as_path()));
            assert!(
                target
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("ibcmd-recovery-test-")
            );
            fs::remove_dir_all(target).unwrap();
        }
    }

    fn directory() -> Directory {
        let path =
            std::env::temp_dir().join(format!("ibcmd-recovery-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Directory(path)
    }

    fn rewrite(path: &Path, manifest: &mut Manifest) {
        manifest.integrity_sha256 = digest(&serde_json::to_vec(&manifest.payload).unwrap());
        fs::write(path, serde_json::to_vec(manifest).unwrap()).unwrap();
    }

    #[test]
    fn compact_pair_roundtrips_headers_bytes_and_identical_repeat() {
        let dir = directory();
        let path = dir.path().join("recovery.json");
        let snapshot = snapshot();
        let token = snapshot_token(&snapshot).unwrap();
        write(&path, "lab", MainActivationMode::Online, &snapshot, &token).unwrap();
        write(&path, "lab", MainActivationMode::Online, &snapshot, &token).unwrap();
        let (db, mode, decoded) = read(&path).unwrap();
        assert_eq!((db.as_str(), mode), ("lab", MainActivationMode::Online));
        assert_eq!(decoded, snapshot);
        let json = fs::read_to_string(&path).unwrap();
        assert!(!json.contains("binary_data"));
        let mut different = snapshot.clone();
        different.retained_config_rows[0].attributes += 1;
        assert!(
            write(
                &path,
                "lab",
                mode,
                &different,
                &snapshot_token(&different).unwrap()
            )
            .is_err()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), json);
    }

    #[test]
    fn missing_truncated_and_changed_pack_never_decode_as_recovery() {
        let dir = directory();
        let path = dir.path().join("recovery.json");
        let snapshot = snapshot();
        let (manifest, pack) = encode(
            "lab",
            MainActivationMode::Online,
            &snapshot,
            &snapshot_token(&snapshot).unwrap(),
        )
        .unwrap();
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(read(&path).is_err());
        let pack_path = sibling(&path, &manifest.payload.pack_file).unwrap();
        fs::write(&pack_path, &pack[..pack.len() - 1]).unwrap();
        assert!(read(&path).is_err());
        let mut changed = pack.clone();
        changed[0] ^= 1;
        fs::write(&pack_path, changed).unwrap();
        assert!(read(&path).is_err());
        fs::write(&pack_path, pack).unwrap();
        assert_eq!(read(&path).unwrap().2, snapshot);
    }

    #[test]
    fn invalid_input_and_pack_filename_refuse_before_publication() {
        let dir = directory();
        let path = dir.path().join("recovery.json");
        let pristine = snapshot();
        for case in 0..4 {
            let mut bad = pristine.clone();
            match case {
                0 => bad.staged_rows[0].data_size += 1,
                1 => bad
                    .retained_config_rows
                    .push(bad.overwritten_config_rows[0].clone()),
                2 => bad.staged_rows = vec![bad.staged_rows[0].clone(); MAX_ROWS + 1],
                3 => bad.old_generation = "invalid".into(),
                _ => unreachable!(),
            }
            assert!(
                write(
                    &path,
                    "lab",
                    MainActivationMode::Online,
                    &bad,
                    &snapshot_token(&bad).unwrap()
                )
                .is_err()
            );
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        }
        let token = snapshot_token(&pristine).unwrap();
        assert!(write(&path, "", MainActivationMode::Online, &pristine, &token).is_err());
        assert!(
            write(
                &path,
                "lab",
                MainActivationMode::Online,
                &pristine,
                &"0".repeat(64)
            )
            .is_err()
        );
        let (manifest, _) = encode("lab", MainActivationMode::Online, &pristine, &token).unwrap();
        assert!(
            write(
                &dir.path().join(&manifest.payload.pack_file),
                "lab",
                MainActivationMode::Online,
                &pristine,
                &token
            )
            .is_err()
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn corrupt_manifest_ranges_tokens_and_foreign_references_refuse() {
        let dir = directory();
        let path = dir.path().join("recovery.json");
        let snapshot = snapshot();
        write(
            &path,
            "lab",
            MainActivationMode::Online,
            &snapshot,
            &snapshot_token(&snapshot).unwrap(),
        )
        .unwrap();
        let pristine = fs::read(&path).unwrap();
        for case in 0..6 {
            let mut manifest: Manifest = serde_json::from_slice(&pristine).unwrap();
            match case {
                0 => manifest.payload.retained_config_rows[0].offset = 0,
                1 => manifest.payload.recovery_token = "0".repeat(64),
                2 => manifest.payload.pack_sha256 = "../foreign".into(),
                3 => manifest.payload.pack_bytes = MAX_PACK_BYTES + 1,
                4 => manifest.payload.staged_rows[0].sha256 = "0".repeat(64),
                5 => manifest.payload.database = String::new(),
                _ => unreachable!(),
            }
            rewrite(&path, &mut manifest);
            assert!(read(&path).is_err(), "case {case}");
        }
        let mut manifest: Manifest = serde_json::from_slice(&pristine).unwrap();
        manifest.payload.mode = MainActivationMode::Worker;
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(read(&path).is_err());
    }
}
