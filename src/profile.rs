#![cfg(feature = "platform-oracle")]
//! Research-only external process profiler.

use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{Result, anyhow};
use serde::Serialize;

use crate::cli::ProfileRunArgs;

#[derive(Debug, Serialize)]
pub struct ProfileRunReport {
    pub command: Vec<String>,
    pub duration_ms: u128,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

pub fn run_profiled(args: ProfileRunArgs) -> Result<ProfileRunReport> {
    if args.command.is_empty() {
        return Err(anyhow!("no command provided"));
    }

    let mut command = Command::new(&args.command[0]);
    command.args(&args.command[1..]);

    if args.capture_output {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
    }

    let started = Instant::now();
    let output = command.output()?;
    let duration_ms = started.elapsed().as_millis();

    Ok(ProfileRunReport {
        command: args.command,
        duration_ms,
        exit_code: output.status.code(),
        success: output.status.success(),
        stdout: args
            .capture_output
            .then(|| String::from_utf8_lossy(&output.stdout).to_string()),
        stderr: args
            .capture_output
            .then(|| String::from_utf8_lossy(&output.stderr).to_string()),
    })
}
