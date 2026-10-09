//! Current-observer recovery; actual original live proof is retained pre-AUTH.
use super::*;

fn same_row(left: &CensusIdentity, right: &CensusIdentity) -> bool {
    left.pid == right.pid
        && left.parent == right.parent
        && left.birth_filetime == right.birth_filetime
        && left.executable == right.executable
        && left.command == right.command
}

impl RetainedAuthenticationDiagnostic {
    fn census_half(&mut self, deadline: Instant) -> Result<DiagnosticBoundaryHalf> {
        crate::mssql_managed_worker::child::require_deadline(deadline)?;
        let rows = self.runtime.shutdown_census()?;
        let record = self
            .runtime
            .census_observers
            .last()
            .context("current original observer record absent")?;
        if record.bound.is_none() {
            bail!("current original observer binding absent");
        }
        let index = record.original_index;
        let original = self
            .runtime
            .collectors
            .get_mut(index)
            .context("current original observer collector absent")?;
        original.require_command_current()?;
        let (exit, raw, stderr) = original.terminal_raw()?;
        if exit != 0 || !stderr.is_empty() {
            bail!("current observer direct/BOTH/status unproved");
        }
        let census: ShutdownCensus = serde_json::from_slice(raw)?;
        if rows.len() != census.owned.len()
            || rows.iter().zip(&census.owned).any(|(a, b)| !same_row(a, b))
        {
            bail!("current observer returned/raw identities differ");
        }
        original.require_command_current()?;
        crate::mssql_managed_worker::child::require_deadline(deadline)?;
        Ok(DiagnosticBoundaryHalf {
            original_collector: index,
            rows,
            listeners: census.listeners,
        })
    }

    pub(in crate::mssql_managed_worker::native::diagnostic) fn prepare_original_recovery_handles(
        &mut self,
        deadline: Instant,
    ) -> Result<()> {
        if self.recovery.preparation_attempted {
            bail!("original recovery handles already attempted; no reopen");
        }
        self.recovery.preparation_attempted = true;
        let first = self.census_half(deadline)?;
        let second = self.census_half(deadline)?;
        let a = self.runtime.descendants(&first.rows)?;
        let b = self.runtime.descendants(&second.rows)?;
        self.runtime.console_tool.check()?;
        require_shutdown_graph(
            &a,
            &b,
            &self.runtime.options.platform_bin,
            &self.runtime.console_tool.path,
            &self.runtime.binding,
        )?;
        for rows in [&first.rows, &second.rows] {
            if rows.iter().any(|r| {
                r.command.to_ascii_lowercase().contains(
                    &self
                        .runtime
                        .binding
                        .root
                        .to_string_lossy()
                        .to_ascii_lowercase(),
                ) && !b.iter().any(|owned| owned.pid == r.pid)
            }) {
                bail!("foreign process names current diagnostic root");
            }
        }
        for (agent, expected) in [
            (true, self.runtime.binding.agent.clone()),
            (false, self.runtime.binding.ras.clone()),
        ] {
            let row = second
                .rows
                .iter()
                .find(|r| r.pid == expected.pid)
                .context("current live anchor absent")?;
            let original = if agent {
                &mut self.runtime.agent
            } else {
                &mut self.runtime.ras
            };
            let proof = original
                .as_mut()
                .context("original live anchor absent")?
                .diagnostic_live_proof(row, deadline)?;
            proof.require_selected(&expected)?;
            if self.recovery.live_anchors.contains_key(&expected.pid) {
                bail!("actual returned original anchor identity differs");
            }
            self.recovery.live_anchors.insert(expected.pid, proof);
            crate::mssql_managed_worker::child::require_deadline(deadline)?;
        }
        let descendants: Vec<_> = b
            .iter()
            .filter(|r| {
                r.pid != self.runtime.binding.agent.pid && r.pid != self.runtime.binding.ras.pid
            })
            .map(|r| (**r).clone())
            .collect();
        for row in descendants {
            if self.runtime.shutdown_handles.contains_key(&row.pid) {
                bail!("original descendant slot duplicate");
            }
            let pid = row.pid;
            self.recovery
                .pending_handles
                .push(PendingShutdownHandle::new(row));
            let pending = self
                .recovery
                .pending_handles
                .last_mut()
                .context("pending original slot absent")?;
            let handle = pending.bind_once(deadline)?;
            self.runtime.shutdown_handles.insert(pid, handle);
            crate::mssql_managed_worker::child::require_deadline(deadline)?;
        }
        self.recovery
            .boundaries
            .push(DiagnosticBoundary { first, second });
        self.journal.append(
            "diagnostic_live_kernel_originals_retained",
            self.runtime
                .shutdown_handles
                .values()
                .map(|h| &h.identity)
                .collect::<Vec<_>>(),
        )?;
        self.selection.require_current()?;
        crate::mssql_managed_worker::child::require_deadline(deadline)?;
        Ok(())
    }

    fn refresh_natural_anchors(&mut self, deadline: Instant) -> Result<()> {
        for original in [&mut self.runtime.agent, &mut self.runtime.ras] {
            let original = original.as_mut().context("SAME original anchor absent")?;
            let proof = self
                .recovery
                .live_anchors
                .get(&original.pid())
                .context("actual original live kernel cache absent")?;
            original.diagnostic_natural_exit(proof, deadline)?;
        }
        crate::mssql_managed_worker::child::require_deadline(deadline)
    }

    fn require_half(&mut self, half: &DiagnosticBoundaryHalf, deadline: Instant) -> Result<()> {
        // The exact original observer has already validated ALL foreign rows,
        // selected listeners, duplicate PIDs and typed observer binding.
        self.refresh_natural_anchors(deadline)?;
        for (original, expected) in [
            (&mut self.runtime.agent, &self.runtime.binding.agent),
            (&mut self.runtime.ras, &self.runtime.binding.ras),
        ] {
            let original = original.as_mut().context("original anchor absent")?;
            let row = half.rows.iter().find(|r| r.pid == expected.pid);
            if let Some(row) = row {
                require_shutdown_identity(row, expected)?;
            }
            if original.direct_exit_proved() {
                continue;
            }
            let row = row.context("original live anchor absent in complete current census")?;
            let current = original.diagnostic_live_proof(row, deadline)?;
            let prior = self
                .recovery
                .live_anchors
                .get(&expected.pid)
                .context("original live anchor cache absent")?;
            current.require_same(prior)?;
        }
        for h in self.runtime.shutdown_handles.values() {
            let row = half.rows.iter().find(|r| r.pid == h.identity.pid);
            if let Some(row) = row {
                require_shutdown_identity(row, &h.identity)?;
            }
            if h.diagnostic_exited(deadline)? {
                continue;
            }
            h.diagnostic_live_current(
                row.context("current original live descendant absent")?,
                deadline,
            )?;
        }
        for row in half
            .rows
            .iter()
            .filter(|r| self.runtime.is_shutdown_member(r))
        {
            if row.pid != self.runtime.binding.agent.pid
                && row.pid != self.runtime.binding.ras.pid
                && !self.runtime.shutdown_handles.contains_key(&row.pid)
            {
                bail!("new or unknown descendant; no reopen/adoption");
            }
        }
        for listener in &half.listeners {
            let alive = if listener.pid == self.runtime.binding.agent.pid {
                !self
                    .runtime
                    .agent
                    .as_ref()
                    .context("original agent absent")?
                    .direct_exit_proved()
            } else if listener.pid == self.runtime.binding.ras.pid {
                !self
                    .runtime
                    .ras
                    .as_ref()
                    .context("original RAS absent")?
                    .direct_exit_proved()
            } else if let Some(h) = self.runtime.shutdown_handles.get(&listener.pid) {
                !h.diagnostic_exited(deadline)?
            } else {
                false
            };
            if !alive {
                bail!("foreign/exited original owns selected listener");
            }
        }
        crate::mssql_managed_worker::child::require_deadline(deadline)
    }

    pub(super) fn current_boundary(&mut self, deadline: Instant) -> Result<usize> {
        self.selection.require_current()?;
        self.refresh_natural_anchors(deadline)?;
        let first = self.census_half(deadline)?;
        let second = self.census_half(deadline)?;
        self.require_half(&first, deadline)?;
        self.require_half(&second, deadline)?;
        for row in &second.rows {
            // Missing first row can only be tolerated for a directly exited
            // original; a still-live original must be complete and stable.
            let alive = if row.pid == self.runtime.binding.agent.pid {
                !self
                    .runtime
                    .agent
                    .as_ref()
                    .context("agent absent")?
                    .direct_exit_proved()
            } else if row.pid == self.runtime.binding.ras.pid {
                !self
                    .runtime
                    .ras
                    .as_ref()
                    .context("RAS absent")?
                    .direct_exit_proved()
            } else if let Some(h) = self.runtime.shutdown_handles.get(&row.pid) {
                !h.diagnostic_exited(deadline)?
            } else {
                false
            };
            if alive && !first.rows.iter().any(|prior| same_row(prior, row)) {
                bail!("original live C1/C2 identity drift");
            }
        }
        let index = self.recovery.boundaries.len();
        self.recovery
            .boundaries
            .push(DiagnosticBoundary { first, second });
        crate::mssql_managed_worker::child::require_deadline(deadline)?;
        Ok(index)
    }

    pub(super) fn physical_stop(&mut self, permit: &DiagnosticRecoveryPermit) -> Result<()> {
        if !self.recovery.preparation_attempted || self.recovery.live_anchors.len() != 2 {
            bail!("pre-effect original live kernel custody absent");
        }
        self.recovery.state = RecoveryState::Signalling;
        for (agent, expected) in [
            (true, self.runtime.binding.agent.clone()),
            (false, self.runtime.binding.ras.clone()),
        ] {
            let boundary = self.current_boundary(permit.deadline)?;
            let original = if agent {
                &self.runtime.agent
            } else {
                &self.runtime.ras
            };
            if original
                .as_ref()
                .context("original anchor absent")?
                .direct_exit_proved()
            {
                continue;
            }
            let row = self.recovery.boundaries[boundary]
                .second
                .rows
                .iter()
                .find(|r| r.pid == expected.pid)
                .context("current live anchor absent")?
                .clone();
            self.journal
                .append("diagnostic_direct_stop_intent", &expected)?;
            self.selection.require_current()?;
            self.signal_once(expected.pid, permit.deadline)?;
            let proof = self
                .recovery
                .live_anchors
                .get(&expected.pid)
                .context("original live anchor proof absent")?;
            let original = if agent {
                &mut self.runtime.agent
            } else {
                &mut self.runtime.ras
            };
            original
                .as_mut()
                .context("original anchor absent")?
                .stop_exact_at(&row, &expected, proof, permit.deadline)?;
            self.journal
                .append("diagnostic_direct_stop_confirmed", &expected)?;
        }
        let ids: Vec<_> = self.runtime.shutdown_handles.keys().copied().collect();
        for pid in &ids {
            let boundary = self.current_boundary(permit.deadline)?;
            let h = self
                .runtime
                .shutdown_handles
                .get(pid)
                .context("SAME original descendant absent")?;
            if h.identity
                .executable
                .to_string_lossy()
                .eq_ignore_ascii_case(&self.runtime.console_tool.path.to_string_lossy())
            {
                continue;
            }
            if h.diagnostic_exited(permit.deadline)? {
                continue;
            }
            let row = self.recovery.boundaries[boundary]
                .second
                .rows
                .iter()
                .find(|r| r.pid == *pid)
                .context("current original descendant absent")?
                .clone();
            let identity = h.identity.clone();
            self.journal
                .append("diagnostic_direct_stop_intent", &identity)?;
            self.selection.require_current()?;
            self.signal_once(*pid, permit.deadline)?;
            self.runtime
                .shutdown_handles
                .get(pid)
                .context("original descendant absent")?
                .stop_exact_at(&row, permit.deadline)?;
            self.journal
                .append("diagnostic_direct_stop_confirmed", &identity)?;
        }
        // Producer anchors are directly exited before their consoles receive
        // a natural-only SAME wait. An already exited console needs no Image.
        for pid in &ids {
            let h = self
                .runtime
                .shutdown_handles
                .get(pid)
                .context("original natural descendant absent")?;
            if h.identity
                .executable
                .to_string_lossy()
                .eq_ignore_ascii_case(&self.runtime.console_tool.path.to_string_lossy())
            {
                if self.recovery.signals.contains(pid) {
                    bail!("console signal is forbidden");
                }
                let code = h.wait_natural_at(permit.deadline)?;
                self.recovery.natural_exits.insert(*pid, code);
                crate::mssql_managed_worker::child::require_deadline(permit.deadline)?;
            }
        }
        for original in [&mut self.runtime.agent, &mut self.runtime.ras] {
            let result = original
                .as_mut()
                .context("original anchor absent")?
                .completed_at(permit.deadline)?;
            self.recovery.anchor_outputs.push(result);
            crate::mssql_managed_worker::child::require_deadline(permit.deadline)?;
        }
        self.current_boundary(permit.deadline)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_original_boundary_identity_changes_never_compare_equal() {
        let row = CensusIdentity {
            pid: 42,
            parent: 41,
            birth_filetime: 1000,
            executable: PathBuf::from("F:/platform/rmngr.exe"),
            command: "original".into(),
        };
        assert!(same_row(&row, &row));
        for n in 0..5 {
            let mut changed = row.clone();
            match n {
                0 => changed.pid += 1,
                1 => changed.parent += 1,
                2 => changed.birth_filetime += 1,
                3 => changed.executable = PathBuf::from("F:/foreign.exe"),
                _ => changed.command.push('x'),
            };
            assert!(!same_row(&row, &changed));
        }
    }
}
