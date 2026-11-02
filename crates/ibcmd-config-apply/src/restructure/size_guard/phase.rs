//! The guard of a structure phase the S1 gate has prepared (S1-J): it reads the config apply's gate types,
//! so it is apart from the root's settings adapter and goes with the gate.

use crate::mssql_config_apply::gate::{GateVerdict, StructurePhase};
use crate::restructure::plan::Method;
use crate::restructure::reader::RowSource;
use anyhow::Result;
use ibcmd_restructure_engine::restructure::size_guard::{LimitSetting, check_tables};

/// The guard of a structure phase the gate has prepared: reads the sizes of the tables it rebuilds, puts
/// the verdict into the phase's report (`size_check`), and above the limit blocks the stage (the reason
/// starts with `S1:`, like every reason of the S1 gate) and drops the phase. Only `Method::Rebuild` copies
/// tables; the research `AlterAdd` adds a column in place and is not guarded. No phase, nothing to guard.
pub fn guard_phase(
    source: &mut dyn RowSource,
    method: Method,
    setting: &LimitSetting,
    verdict: &mut GateVerdict,
    phase: &mut Option<StructurePhase>,
) -> Result<()> {
    if method != Method::Rebuild {
        return Ok(());
    }
    let Some(prepared) = phase.as_mut() else {
        return Ok(());
    };
    let check = check_tables(source, &prepared.tables, setting)?;
    prepared.size_check = Some(check.to_json());
    if let Some(reason) = check.refusal() {
        verdict.block("", format!("S1: {reason}"));
        *phase = None;
    }
    Ok(())
}
