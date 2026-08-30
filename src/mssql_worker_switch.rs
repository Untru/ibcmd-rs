//! Fail-closed 1C worker-process handoff for development live activation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use uuid::Uuid;

const ZERO_UUID: &str = "00000000-0000-0000-0000-000000000000";
const MAX_RAC_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct WorkerSwitchOptions {
    pub rac: PathBuf,
    pub ras_endpoint: String,
    pub cluster_id: Uuid,
    pub infobase_id: Uuid,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerSwitchReport {
    pub old_process: String,
    pub new_process: String,
    pub switch_ms: u128,
}

#[derive(Debug, Clone)]
pub struct WorkerSwitchPlan {
    old_process: String,
}

pub fn prepare_dedicated_worker(options: &WorkerSwitchOptions) -> Result<WorkerSwitchPlan> {
    let before = list_connections(options)?;
    let target = options.infobase_id.hyphenated().to_string();
    let target_processes = before
        .iter()
        .filter(|entry| entry.get("infobase").is_some_and(|value| value == &target))
        .filter_map(|entry| entry.get("process").cloned())
        .collect::<BTreeSet<_>>();
    if target_processes.len() != 1 {
        bail!(
            "worker activation requires exactly one process serving infobase {}; observed {}",
            target,
            target_processes.len()
        );
    }
    let old_process = target_processes.into_iter().next().unwrap();
    validate_process_is_dedicated(&before, &old_process, &target)?;
    Ok(WorkerSwitchPlan { old_process })
}

pub fn switch_dedicated_worker(
    options: &WorkerSwitchOptions,
    plan: &WorkerSwitchPlan,
) -> Result<WorkerSwitchReport> {
    let started = Instant::now();
    let target = options.infobase_id.hyphenated().to_string();
    let current = list_connections(options)?;
    let target_processes = current
        .iter()
        .filter(|entry| entry.get("infobase").is_some_and(|value| value == &target))
        .filter_map(|entry| entry.get("process").cloned())
        .collect::<BTreeSet<_>>();
    if target_processes != BTreeSet::from([plan.old_process.clone()]) {
        bail!(
            "worker assignment changed after preflight; SQL was committed but no worker was turned off"
        );
    }
    validate_process_is_dedicated(&current, &plan.old_process, &target).context(
        "worker assignment changed after preflight; SQL was committed but no worker was turned off",
    )?;
    let old_process = plan.old_process.clone();

    let mut turn_off = Command::new(&options.rac)
        .args([
            "process".to_owned(),
            "turn-off".to_owned(),
            format!("--cluster={}", options.cluster_id.hyphenated()),
            format!("--process={old_process}"),
            options.ras_endpoint.clone(),
        ])
        .spawn()
        .with_context(|| format!("failed to start {}", options.rac.display()))?;

    let deadline = Instant::now() + options.timeout;
    while Instant::now() < deadline {
        if let Some(status) = turn_off
            .try_wait()
            .context("failed to poll rac process turn-off")?
            && !status.success()
        {
            bail!(
                "rac process turn-off failed with status {:?}",
                status.code()
            );
        }
        let current = list_connections(options)?;
        if let Some(new_process) = current.iter().find_map(|entry| {
            (entry.get("infobase") == Some(&target))
                .then(|| entry.get("process"))
                .flatten()
                .filter(|process| *process != &old_process)
                .cloned()
        }) {
            return Ok(WorkerSwitchReport {
                old_process,
                new_process,
                switch_ms: started.elapsed().as_millis(),
            });
        }
        thread::sleep(Duration::from_millis(100));
    }
    if turn_off.try_wait()?.is_none() {
        let _ = turn_off.kill();
        let _ = turn_off.wait();
    }
    bail!(
        "worker process {} was turned off, but infobase {} did not attach to a replacement within {} ms",
        old_process,
        target,
        options.timeout.as_millis()
    )
}

fn validate_process_is_dedicated(
    connections: &[BTreeMap<String, String>],
    process: &str,
    target: &str,
) -> Result<()> {
    let foreign_infobases = connections
        .iter()
        .filter(|entry| entry.get("process").is_some_and(|value| value == process))
        .filter_map(|entry| entry.get("infobase"))
        .filter(|infobase| infobase.as_str() != ZERO_UUID && infobase.as_str() != target)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !foreign_infobases.is_empty() {
        bail!(
            "worker activation refuses shared process {}; foreign infobases: {}",
            process,
            foreign_infobases.into_iter().collect::<Vec<_>>().join(",")
        );
    }
    Ok(())
}

fn list_connections(options: &WorkerSwitchOptions) -> Result<Vec<BTreeMap<String, String>>> {
    let text = run_rac(
        &options.rac,
        [
            "connection".to_owned(),
            "list".to_owned(),
            format!("--cluster={}", options.cluster_id.hyphenated()),
            options.ras_endpoint.clone(),
        ],
    )?;
    Ok(parse_rac_blocks(&text))
}

fn run_rac<I>(executable: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = String>,
{
    let output = Command::new(executable)
        .args(args)
        .output()
        .with_context(|| format!("failed to start {}", executable.display()))?;
    if output.stdout.len() > MAX_RAC_OUTPUT_BYTES || output.stderr.len() > MAX_RAC_OUTPUT_BYTES {
        bail!("rac output exceeded {} bytes", MAX_RAC_OUTPUT_BYTES);
    }
    if !output.status.success() {
        bail!(
            "rac failed with status {:?}: stdout={} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_rac_blocks(text: &str) -> Vec<BTreeMap<String, String>> {
    let mut blocks = Vec::new();
    let mut current = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            current.insert(
                key.trim().to_owned(),
                value.trim().trim_matches('"').to_owned(),
            );
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rac_connection_blocks_without_localized_values() {
        let parsed = parse_rac_blocks(
            "connection : a\nprocess : p1\ninfobase : base\n\nconnection : b\nprocess : p2\ninfobase : 00000000-0000-0000-0000-000000000000\n",
        );
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].get("process").map(String::as_str), Some("p1"));
        assert_eq!(
            parsed[1].get("infobase").map(String::as_str),
            Some(ZERO_UUID)
        );
    }

    #[test]
    fn dedicated_process_gate_rejects_foreign_infobase() {
        let parsed = parse_rac_blocks(
            "connection : a\nprocess : p1\ninfobase : target\n\nconnection : b\nprocess : p1\ninfobase : foreign\n",
        );
        assert!(validate_process_is_dedicated(&parsed, "p1", "target").is_err());
        assert!(validate_process_is_dedicated(&parsed[..1], "p1", "target").is_ok());
    }
}
