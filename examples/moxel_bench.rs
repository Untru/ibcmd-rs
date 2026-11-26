//! Offline benchmark and regression harness for the MOXCEL spreadsheet
//! converter: converts every listed spreadsheet body row of a rows folder and
//! either writes the XML, compares it with an earlier run, or only times it.
//!
//! `moxel_bench <rows dir> <row list> <refs.tsv> <threads> [--write DIR] [--compare DIR] [--repeat N]`
//!
//! The row list holds one `FileName` per line (`<uuid>.0`); `refs.tsv` maps
//! uuids to `Kind.Name`, the object references the converter names style
//! items and common pictures by.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use rayon::prelude::*;

fn read_row(rows: &Path, name: &str) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    for part in 0.. {
        let path = rows.join(format!("{name}__part{part}.bin"));
        match fs::read(&path) {
            Ok(bytes) => out.extend_from_slice(&bytes),
            Err(_) if part > 0 => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(out)
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        anyhow::bail!(
            "usage: moxel_bench <rows> <list> <refs.tsv> <threads> [--write DIR] [--compare DIR] [--repeat N]"
        );
    }
    let rows = PathBuf::from(&args[1]);
    let names: Vec<String> = fs::read_to_string(&args[2])?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    let mut refs = BTreeMap::new();
    for line in fs::read_to_string(&args[3])?.lines() {
        if let Some((uuid, name)) = line.split_once('\t') {
            refs.insert(uuid.to_string(), name.to_string());
        }
    }
    let threads: usize = args[4].parse()?;
    let mut write_dir = None;
    let mut compare_dir = None;
    let mut repeat = 1usize;
    let mut index = 5;
    while index < args.len() {
        match args[index].as_str() {
            "--write" => write_dir = Some(PathBuf::from(&args[index + 1])),
            "--compare" => compare_dir = Some(PathBuf::from(&args[index + 1])),
            "--repeat" => repeat = args[index + 1].parse()?,
            other => anyhow::bail!("unknown option {other}"),
        }
        index += 2;
    }
    let loaded: Vec<(String, Vec<u8>)> = names
        .iter()
        .map(|name| Ok((name.clone(), read_row(&rows, name)?)))
        .collect::<anyhow::Result<_>>()?;
    let input_bytes: usize = loaded.iter().map(|(_, bytes)| bytes.len()).sum();
    if let Some(dir) = &write_dir {
        fs::create_dir_all(dir)?;
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()?;
    let failed = AtomicUsize::new(0);
    let different = AtomicUsize::new(0);
    let missing = AtomicUsize::new(0);
    let mut best_wall = u128::MAX;
    let mut best_cpu = u128::MAX;
    let mut output_bytes = 0usize;
    for round in 0..repeat {
        let started = Instant::now();
        let results: Vec<(u128, usize)> = pool.install(|| {
            loaded
                .par_iter()
                .map(|(name, bytes)| {
                    let one = Instant::now();
                    let xml = ibcmd_rs::mssql_dump::try_extract_moxel_spreadsheet_xml(bytes, &refs);
                    let micros = one.elapsed().as_micros();
                    let Ok(xml) = xml else {
                        failed.fetch_add(1, Ordering::Relaxed);
                        return (micros, 0);
                    };
                    if round == 0 {
                        if let Some(dir) = &write_dir {
                            fs::write(dir.join(format!("{name}.xml")), &xml).unwrap();
                        }
                        if let Some(dir) = &compare_dir {
                            match fs::read(dir.join(format!("{name}.xml"))) {
                                Ok(expected) if expected == xml.as_bytes() => {}
                                Ok(_) => {
                                    different.fetch_add(1, Ordering::Relaxed);
                                    eprintln!("different: {name}");
                                }
                                Err(_) => {
                                    missing.fetch_add(1, Ordering::Relaxed);
                                    eprintln!("missing baseline: {name}");
                                }
                            }
                        }
                    }
                    (micros, xml.len())
                })
                .collect()
        });
        let wall = started.elapsed().as_millis();
        let cpu = results.iter().map(|(micros, _)| micros).sum::<u128>() / 1000;
        output_bytes = results.iter().map(|(_, bytes)| bytes).sum();
        best_wall = best_wall.min(wall);
        best_cpu = best_cpu.min(cpu);
        println!("round {round}: wall {wall} ms, cpu {cpu} ms");
        if round == 0 {
            let mut slowest: Vec<(u128, &str)> = results
                .iter()
                .zip(&loaded)
                .map(|((micros, _), (name, _))| (*micros, name.as_str()))
                .collect();
            slowest.sort_unstable_by(|a, b| b.cmp(a));
            for (micros, name) in slowest.iter().take(8) {
                println!("  slow {:>8} ms {name}", micros / 1000);
            }
        }
    }
    println!(
        "{} templates ({} input bytes): best wall {} ms, best cpu {} ms; {} output bytes; failed {} different {} missing {}",
        loaded.len(),
        input_bytes,
        best_wall,
        best_cpu,
        output_bytes,
        failed.load(Ordering::Relaxed) / repeat.max(1),
        different.load(Ordering::Relaxed),
        missing.load(Ordering::Relaxed)
    );
    Ok(())
}
