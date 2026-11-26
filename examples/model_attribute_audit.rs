//! Measures the shared attribute body against every attribute-like child of
//! the stored rows:
//! `model_attribute_audit <tree> <rows> <2.20|2.21> <report.json>`.

use std::path::Path;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        anyhow::bail!("usage: model_attribute_audit <tree> <rows> <version> <report.json>");
    }
    let report = ibcmd_rs::metadata_model::attribute::corpus::audit(
        Path::new(&args[1]),
        Path::new(&args[2]),
        &args[3],
        80,
    )?;
    println!(
        "total {} identical {} different {} failed {} no_body {}",
        report.total, report.identical, report.different, report.failed, report.no_body
    );
    let mut slots: Vec<_> = report.slots.iter().collect();
    slots.sort_by(|a, b| b.1.cmp(a.1));
    for (slot, count) in slots.iter().take(40) {
        println!("  {count:7} {slot}");
    }
    let mut failures: Vec<_> = report.failures.iter().collect();
    failures.sort_by(|a, b| b.1.cmp(a.1));
    for (failure, count) in failures.iter().take(30) {
        println!("  fail {count:7} {failure}");
    }
    std::fs::write(&args[4], serde_json::to_string_pretty(&report)?)?;
    Ok(())
}
