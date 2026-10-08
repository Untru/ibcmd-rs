//! Existing raw profile-probe output policy over the shared original collector.
//! This is private control-process plumbing; managed creator authority is separate.

use std::process::{Command, ExitStatus};
use std::time::Duration;

use anyhow::{Context, Result, ensure};

use crate::rac_process::{CapturePolicy, completed_output};

pub(crate) const MAX_PROBE_OUTPUT_BYTES: usize = 64 * 1024;

pub(crate) struct BoundedOutput {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

pub(crate) fn bounded_output(command: &mut Command) -> Result<BoundedOutput> {
    bounded_output_with_timeout(command, Duration::from_secs(20))
}

// The production wrapper owns 20s. Actual OS boundary tests can supply their
// own shorter immutable deadline while executing this SAME policy/collector.
pub(crate) fn bounded_output_with_timeout(
    command: &mut Command,
    timeout: Duration,
) -> Result<BoundedOutput> {
    let output = completed_output(
        command,
        timeout,
        CapturePolicy::DrainToEof {
            limit: MAX_PROBE_OUTPUT_BYTES,
        },
    )
    .context(
        "native profile/RAS probe completion failed or is unproved; original retained on unknown",
    )?;
    ensure!(
        !output.stdout.exceeded && !output.stderr.exceeded,
        "probe output exceeded {MAX_PROBE_OUTPUT_BYTES} bytes"
    );
    // Keep the original profile policy: lossy decoding and no generic
    // success/empty-stderr requirement. Existing profile callers judge status.
    Ok(BoundedOutput {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout.bytes).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr.bytes).into_owned(),
    })
}
