//! The check on the four reference exports (БСП and ERP УХ, 8.3.27 and 8.5):
//! a tree against itself, and against a copy written differently, must need
//! no restructuring and give no reason. Ignored by default: the exports live
//! under the lab folders. Run with
//! `cargo test -p ibcmd-apply-check apply_check::corpus -- --ignored --nocapture`.
//!
//! `IBCMD_RS_APPLY_CHECK_DB` (a database restored from the БСП 8.3.27 corpus
//! backup) also enables the check of the БСП export against that database.

use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::metadata_model::export::rewrite_file;

const REFERENCE_TREES: &[(&str, &str)] = &[
    (
        "БСП 8.3.27",
        r"F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native",
    ),
    (
        "УХ 8.3.27",
        r"E:\ibcmd_lab\parity\ibcmd_rs_uha_8327_native_20260919_20260919_uha_8327_85head\native",
    ),
    (
        "БСП 8.5",
        r"F:\ibcmd\lab\v85\ibcmd_rs_bsp_85_src_20260922_20260922_bsp85_r2\native",
    ),
    ("УХ 8.5", r"F:\ibcmd\lab\v85\native\uha_20260923\native"),
];

fn reference_trees() -> Vec<(&'static str, PathBuf)> {
    REFERENCE_TREES
        .iter()
        .map(|(name, path)| (*name, PathBuf::from(path)))
        .filter(|(name, path)| {
            let present = path.join("Configuration.xml").is_file();
            if !present {
                println!("{name}: {} is not there, skipped", path.display());
            }
            present
        })
        .collect()
}

fn assert_clean(name: &str, verdict: &Verdict) {
    assert!(
        !verdict.needs_restructuring && verdict.reasons.is_empty(),
        "{name}: {:#?}",
        &verdict.reasons[..verdict.reasons.len().min(10)]
    );
    assert!(verdict.objects.is_empty(), "{name}: {:?}", verdict.objects);
}

#[test]
#[ignore = "reads the four reference exports under the lab folders"]
fn a_reference_tree_against_itself_needs_no_restructuring() {
    let trees = reference_trees();
    assert!(!trees.is_empty(), "none of the reference trees is present");
    for (name, path) in trees {
        let started = std::time::Instant::now();
        let verdict = check_trees(&path, &path).unwrap();
        println!(
            "{name}: {} files, {} reasons, {:.1}s",
            verdict.stats.new_files,
            verdict.reasons.len(),
            started.elapsed().as_secs_f64()
        );
        assert_clean(name, &verdict);
        assert_eq!(verdict.stats.new_files, verdict.stats.old_files);
    }
}

/// Every `stride`-th metadata file of `root`, written to `old/` as it is and
/// to `new/` as the model's writer writes it plus a trailing blank line: the
/// files differ in bytes, so the check has to parse both, and must find the
/// same tree.
fn write_sample_pair(root: &Path, scratch: &Path, stride: usize) -> usize {
    let mut taken = 0usize;
    let mut seen = 0usize;
    for entry in walkdir::WalkDir::new(root).sort_by_file_name() {
        let entry = entry.unwrap();
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|ext| ext.to_str()) != Some("xml")
        {
            continue;
        }
        // Only every stride-th XML file is opened: reading all of them is
        // minutes of first-read time on a corpus.
        seen += 1;
        if !seen.is_multiple_of(stride) {
            continue;
        }
        let bytes = fs::read(entry.path()).unwrap();
        let head = &bytes[..bytes.len().min(2048)];
        if !head
            .windows(b"<MetaDataObject".len())
            .any(|window| window == b"<MetaDataObject")
        {
            continue;
        }
        let rel = entry.path().strip_prefix(root).unwrap();
        let rewritten = rewrite_file(&bytes).unwrap_or_else(|error| {
            panic!("{}: cannot be rewritten: {error:#}", entry.path().display())
        });
        for (side, text) in [
            ("old", String::from_utf8_lossy(&bytes).into_owned()),
            ("new", format!("{rewritten}\r\n")),
        ] {
            let target = scratch.join(side).join(rel);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, text).unwrap();
        }
        taken += 1;
    }
    taken
}

#[test]
#[ignore = "reads the four reference exports under the lab folders"]
fn a_reference_tree_against_a_rewritten_copy_shows_no_change() {
    let trees = reference_trees();
    assert!(!trees.is_empty(), "none of the reference trees is present");
    for (name, path) in trees {
        let scratch = std::env::temp_dir().join(format!(
            "ibcmd-apply-check-corpus-{}-{}",
            std::process::id(),
            name.replace(' ', "_").replace('.', "")
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&scratch);
        let _ = fs::remove_dir_all(&scratch);
        let taken = write_sample_pair(&path, &scratch, 7);
        assert!(taken > 100, "{name}: only {taken} files sampled");
        let verdict = check_trees(&scratch.join("old"), &scratch.join("new")).unwrap();
        fs::remove_dir_all(&scratch).ok();
        println!(
            "{name}: {taken} files parsed on both sides, {} compared, {} reasons",
            verdict.stats.descriptors_compared,
            verdict.reasons.len()
        );
        assert_eq!(verdict.stats.descriptors_compared, taken, "{name}");
        assert_clean(name, &verdict);
    }
}

#[test]
#[ignore = "requires a local SQL Server database restored from the БСП corpus"]
fn the_bsp_export_against_its_database_shows_no_change() {
    let Ok(database) = std::env::var("IBCMD_RS_APPLY_CHECK_DB") else {
        println!("IBCMD_RS_APPLY_CHECK_DB is not set, skipped");
        return;
    };
    let Some((name, path)) = reference_trees()
        .into_iter()
        .find(|(name, _)| *name == "БСП 8.3.27")
    else {
        println!("the БСП 8.3.27 export is not there, skipped");
        return;
    };
    let sql = crate::sql::SqlExec::from_options(crate::sql::SqlOptions {
        sqlcmd: None,
        bcp: None,
        server: "localhost",
        user: None,
        password: None,
        password_env: "IBCMD_DB_PSW",
        trust_server_certificate: true,
    })
    .unwrap();
    let verdict = check_tree_against_db(&sql, &database, &path, None, false).unwrap();
    println!(
        "{name} against {database}: {} descriptors compared, {} bodies not compared",
        verdict.stats.descriptors_compared, verdict.stats.body_files_not_compared
    );
    assert_clean(name, &verdict);
}
