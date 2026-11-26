//! Short utility commands keep the original owner until the final caller policy.
//! This is a private seam shared by NativeRuntime and its finite OS controls.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use super::child::{OriginalChild, require_deadline};

pub(crate) fn deadline(startup: Option<Instant>, timeout: Duration) -> Result<Instant> {
    let deadline = match startup {
        Some(deadline) => deadline,
        None => Instant::now()
            .checked_add(timeout)
            .context("managed command deadline overflow")?,
    };
    require_deadline(deadline)?;
    Ok(deadline)
}

pub(crate) fn run<T>(
    failed: &mut bool,
    collectors: &mut Vec<OriginalChild>,
    executable: &Path,
    argv: &[String],
    deadline: Instant,
    check_tool: impl FnOnce() -> Result<()>,
    policy: impl FnOnce((i32, Vec<u8>, Vec<u8>)) -> Result<T>,
) -> Result<T> {
    if *failed {
        bail!("managed native lifetime already unproved");
    }
    require_deadline(deadline)?;
    check_tool()?;
    require_deadline(deadline)?;
    // Retain the existing control-command custody admission; not a metadata cap.
    if collectors.len() >= 1024 {
        bail!("managed original command custody bound; retain lifetime");
    }
    collectors.push(OriginalChild::spawn_at(executable, argv, deadline)?);
    let original = collectors
        .last_mut()
        .context("original command custody absent")?;
    let completed = original.completed_at(deadline);
    let output = match completed {
        Ok(output) => output,
        Err(error) => {
            *failed = true;
            return Err(error);
        }
    };
    // A timely nonzero/status/encoding refusal is a known terminal command.
    // A policy which finishes late cannot release its original custody as known.
    if let Err(error) = require_deadline(deadline) {
        original.mark_unproved();
        *failed = true;
        return Err(error);
    }
    let result = policy(output);
    if let Err(error) = require_deadline(deadline) {
        original.mark_unproved();
        *failed = true;
        return Err(error);
    }
    result
}

pub(crate) fn strict_text(output: (i32, Vec<u8>, Vec<u8>)) -> Result<String> {
    let (exit, out, err) = output;
    if exit != 0 || !err.is_empty() {
        bail!("managed utility command failed; arguments/output redacted");
    }
    String::from_utf8(out).context("managed utility response must be strict UTF-8")
}
