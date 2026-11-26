//! Internal authentication protocol. Read-only admin lists are observations,
//! never proof that credentials are required for administrative writes.
use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(super) enum Family {
    Agent,
    Cluster,
}
impl Family {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Cluster => "cluster",
        }
    }
    pub(super) fn is_agent(self) -> bool {
        self == Self::Agent
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(super) enum Credentials {
    Correct,
    WrongPassword,
    ImplicitOs,
}
impl Credentials {
    fn suffix(self) -> &'static str {
        match self {
            Self::Correct => "control",
            Self::WrongPassword => "wrong",
            Self::ImplicitOs => "os",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Register,
    Remove,
}

pub(super) struct Plan {
    administrator: String,
    cluster: Uuid,
}
impl Plan {
    pub(super) fn new(administrator: &str, cluster: Uuid) -> Result<Self> {
        let nonce = administrator
            .strip_prefix("ibcmd_")
            .context("generated administrator required")?;
        let parsed = Uuid::parse_str(nonce).context("generated administrator UUID required")?;
        if parsed.is_nil() || parsed.to_string() != nonce || cluster.is_nil() {
            bail!("canonical own administrator and nonnil cluster required");
        }
        Ok(Self {
            administrator: administrator.into(),
            cluster,
        })
    }
    pub(super) fn name(&self, family: Family, challenge: Credentials) -> String {
        format!(
            "{}_{}_{}",
            self.administrator,
            family.name(),
            challenge.suffix()
        )
    }
    pub(super) fn arguments(
        &self,
        family: Family,
        challenge: Credentials,
        action: Action,
    ) -> Vec<String> {
        let mut argv = vec![
            family.name().into(),
            "admin".into(),
            match action {
                Action::Register => "register",
                Action::Remove => "remove",
            }
            .into(),
        ];
        if family == Family::Cluster {
            argv.push(format!("--cluster={}", self.cluster));
        }
        argv.push(format!("--name={}", self.name(family, challenge)));
        argv
    }
    pub(super) fn mutation_arguments(
        &self,
        family: Family,
        challenge: Credentials,
        action: Action,
        credentials: Credentials,
        passwords: [&str; 3],
    ) -> Vec<String> {
        let mut argv = self.arguments(family, challenge, action);
        if action == Action::Register {
            argv.extend([format!("--pwd={}", passwords[2]), "--auth=pwd".into()]);
        }
        let password = match credentials {
            Credentials::Correct => Some(passwords[0]),
            Credentials::WrongPassword => Some(passwords[1]),
            Credentials::ImplicitOs => None,
        };
        if let Some(password) = password {
            argv.extend([
                format!("--{}-user={}", family.name(), self.administrator),
                format!("--{}-pwd={password}", family.name()),
            ]);
        }
        argv
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Inventory(BTreeMap<String, BTreeMap<String, String>>);
impl Inventory {
    pub(super) fn from_rows(rows: Vec<BTreeMap<String, String>>, allowed: &[&str]) -> Result<Self> {
        if rows.is_empty() || rows.len() > 2 || allowed.is_empty() || allowed.len() > 2 {
            bail!("bounded own admin inventory required");
        }
        let mut inventory = BTreeMap::new();
        for row in rows {
            if row.len() > 32
                || row
                    .iter()
                    .any(|(key, value)| key.is_empty() || key.len() > 128 || value.len() > 4096)
            {
                bail!("bounded complete admin fields required");
            }
            let name = row
                .get("name")
                .context("administrator name absent")?
                .clone();
            if !allowed.contains(&name.as_str())
                || row.get("auth").map(String::as_str) != Some("pwd")
                || row.get("os-user").is_some_and(|value| !value.is_empty())
                || inventory.insert(name, row).is_some()
            {
                bail!("foreign, duplicate or non-password administrator refused");
            }
        }
        Ok(Self(inventory))
    }
    fn digest(&self) -> Result<String> {
        Ok(format!(
            "{:X}",
            Sha256::digest(serde_json::to_vec(&self.0)?)
        ))
    }
    fn exact_baseline(&self, plan: &Plan) -> Result<()> {
        if self.0.len() != 1 || !self.0.contains_key(&plan.administrator) {
            bail!("one exact own baseline required");
        }
        Ok(())
    }
    fn control_present(&self, before: &Self, name: &str) -> Result<()> {
        if self.0.len() != 2
            || !self.0.contains_key(name)
            || before
                .0
                .iter()
                .any(|(key, value)| self.0.get(key) != Some(value))
        {
            bail!("control absent or complete baseline drifted");
        }
        Ok(())
    }
}

pub(super) struct Receipt {
    pub exit: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
impl Receipt {
    fn successful(&self) -> Result<()> {
        if self.exit != 0 || !self.stderr.is_empty() {
            bail!("authenticated mutation failed; native output redacted");
        }
        Ok(())
    }
}

/// Production implementation retains original command outcomes and checks the
/// current private endpoint before every operation. An Err cannot be retried.
pub(super) trait Io {
    fn inventory(&mut self, family: Family, allowed: &[&str]) -> Result<Inventory>;
    fn mutation(
        &mut self,
        family: Family,
        challenge: Credentials,
        action: Action,
        credentials: Credentials,
    ) -> Result<Receipt>;
    fn record(&mut self, event: &'static str, value: serde_json::Value) -> Result<()>;
    fn denial_is_measured(&self, family: Family, kind: Credentials, receipt: &Receipt) -> bool;
}

pub(super) fn verify(io: &mut impl Io, plan: &Plan) -> Result<()> {
    for family in [Family::Agent, Family::Cluster] {
        let baseline = io.inventory(family, &[&plan.administrator])?;
        baseline.exact_baseline(plan)?;
        let name = plan.name(family, Credentials::Correct);
        io.mutation(
            family,
            Credentials::Correct,
            Action::Register,
            Credentials::Correct,
        )?
        .successful()?;
        let present = io.inventory(family, &[&plan.administrator, &name])?;
        present.control_present(&baseline, &name)?;
        io.mutation(
            family,
            Credentials::Correct,
            Action::Remove,
            Credentials::Correct,
        )?
        .successful()?;
        let restored = io.inventory(family, &[&plan.administrator])?;
        if restored != baseline {
            bail!("control failed complete baseline restoration");
        }
        io.record("correct_admin_mutation_observed", serde_json::json!({
            "family": family, "before": baseline.digest()?, "present": present.digest()?, "after": restored.digest()?,
            "product_ownership": false,
        }))?;
        for kind in [Credentials::WrongPassword, Credentials::ImplicitOs] {
            let before = io.inventory(family, &[&plan.administrator])?;
            if before != baseline {
                bail!("whole baseline changed before mutation challenge");
            }
            io.record(
                "authentication_challenge_intent",
                serde_json::json!({ "family": family, "kind": kind }),
            )?;
            let receipt = io.mutation(family, kind, Action::Register, kind)?;
            let name = plan.name(family, kind);
            let after = io.inventory(family, &[&plan.administrator, &name])?;
            let mutated = after.0.contains_key(&name);
            if !mutated && after != before {
                bail!("unrelated complete inventory change during challenge");
            }
            if mutated {
                after.control_present(&before, &name)?;
            }
            let admitted =
                !mutated && receipt.exit != 0 && io.denial_is_measured(family, kind, &receipt);
            io.record("administration_mutation_challenge", serde_json::json!({
                "family": family, "kind": kind, "exit": receipt.exit, "observed_mutation": mutated,
                "stdout_bytes": receipt.stdout.len(), "stderr_bytes": receipt.stderr.len(),
                "stdout_sha256": format!("{:X}", Sha256::digest(&receipt.stdout)),
                "stderr_sha256": format!("{:X}", Sha256::digest(&receipt.stderr)),
                "before": before.digest()?, "after": after.digest()?, "classified_denial": admitted,
                "product_ownership": false,
            }))?;
            // Only a proved completed challenge and unchanged baseline plus one
            // exact own addition allow removing that own throwaway. Unknown
            // command outcomes propagate above with no collector/removal/retry.
            if mutated {
                io.mutation(family, kind, Action::Remove, Credentials::Correct)?
                    .successful()?;
                if io.inventory(family, &[&plan.administrator])? != before {
                    bail!("own challenge addition failed exact cleanup");
                }
            }
            if !admitted {
                bail!("mutation challenge unclassified or OS bypass; no ownership authority");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
