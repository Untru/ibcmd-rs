//! Measures type patterns against every type description of a tree (metadata
//! XMLs and owned forms' bodies):
//! `model_pattern_audit <tree> <rows> <2.20|2.21> <report.json>`.

use std::path::Path;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        anyhow::bail!("usage: model_pattern_audit <tree> <rows> <version> <report.json>");
    }
    let report = ibcmd_rs::metadata_model::types::corpus::audit(
        Path::new(&args[1]),
        Path::new(&args[2]),
        &args[3],
        200,
    )?;
    println!(
        "total {} found {} missing {} failed {}",
        report.total, report.found, report.missing, report.failed
    );
    for (kind, count) in &report.missing_by {
        println!("  missing {count:7} {kind}");
    }
    let mut failures: Vec<_> = report.failures.iter().collect();
    failures.sort_by(|a, b| b.1.cmp(a.1));
    for (failure, count) in failures.iter().take(40) {
        println!("  fail {count:7} {failure}");
    }
    std::fs::write(&args[4], serde_json::to_string_pretty(&report)?)?;
    Ok(())
}
