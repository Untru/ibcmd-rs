//! Ratchet over the local .epf/.erf corpus (not in git): every file listed in
//! `tests/fixtures/external/corpus-baseline.txt` must stay byte-identical to
//! the platform's dump made in the configuration infobase.
//!
//! IBCMD_ONECDEC_CORPUS=<root> cargo test --release --no-default-features --test integration external_corpus:: -- --ignored
//! IBCMD_UPDATE_BASELINE=1 also rewrites the baseline with every file that matches now.

use crate::common;

use std::{collections::BTreeSet, fs, path::Path};

#[test]
#[ignore = "needs IBCMD_ONECDEC_CORPUS"]
fn external_corpus_does_not_regress() {
    let Ok(root) = std::env::var("IBCMD_ONECDEC_CORPUS") else {
        return;
    };
    let root = Path::new(&root);
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("external-corpus");
    let mut identical = BTreeSet::new();
    let (mut failed, mut extra) = (Vec::new(), Vec::new());
    let (mut total, mut same) = (0usize, 0usize);
    let mut inputs: Vec<_> = fs::read_dir(root.join("ext-bin"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    inputs.sort();
    for path in inputs {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
            continue;
        };
        let out = work.join(&stem);
        let run = common::export(&path, &out);
        // 0, or 2 with the tree written and the failed entries reported; any
        // other outcome is a crash or a refusal of the whole file.
        if !matches!(run.status.code(), Some(0 | 2)) || !out.exists() {
            failed.push(format!("{stem}: {:?}", run.status.code()));
        }
        let native = common::files(&root.join("native-8.3.27.2214/ext-cfg").join(&stem));
        let ours = common::files(&out);
        for rel in ours.keys() {
            if !native.contains_key(rel) {
                extra.push(format!("{stem}/{rel}"));
            }
        }
        for (rel, bytes) in &native {
            if rel == ".complete" {
                continue;
            }
            total += 1;
            if ours.get(rel) == Some(bytes) {
                same += 1;
                identical.insert(format!("{stem}/{rel}"));
            }
        }
    }
    eprintln!(
        "external corpus: {same}/{total} files identical, {} extra, {} failed exports",
        extra.len(),
        failed.len()
    );
    assert!(failed.is_empty(), "exports failed: {failed:?}");
    // A file the platform does not write is as wrong as a differing one.
    assert!(
        extra.is_empty(),
        "files the platform does not write: {extra:?}"
    );
    let baseline_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/external/corpus-baseline.txt");
    if std::env::var_os("IBCMD_UPDATE_BASELINE").is_some() {
        let lines: Vec<_> = identical.iter().cloned().collect();
        fs::write(&baseline_path, lines.join("\n") + "\n").unwrap();
    }
    let baseline = fs::read_to_string(&baseline_path).unwrap_or_default();
    let regressed: Vec<_> = baseline
        .lines()
        .filter(|line| !line.is_empty() && !identical.contains(*line))
        .collect();
    assert!(regressed.is_empty(), "regressed vs baseline: {regressed:?}");
}
