//! Internal lifetime authority for a product-created worker. Never deserialize
//! this type from a user's file or construct it from a connection inventory.
//!
//! The creator and observer are separate from SQL publication: a pre-commit
//! refusal leaves SQL untouched, whereas an unproved handoff after publication
//! must preserve the committed result and its recovery artifacts.

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use uuid::Uuid;

mod child;
mod command;
mod identity;
pub(crate) use identity::ProcessIdentity;
mod native;
// The guarded live undo is built and tested, but no command runs it yet; in
// the root its unused items were dead-code warnings.
#[allow(dead_code)]
pub(crate) mod undo;
#[doc(hidden)]
pub use native::{Creation, CreatorOptions, NativeRuntime, create};
#[doc(hidden)]
pub type NativeManagedWorker = ManagedWorker<native::NativeRuntime>;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
#[doc(hidden)]
pub struct LifetimeBinding {
    pub nonce: Uuid,
    pub root: PathBuf,
    pub cluster: Uuid,
    pub infobase: Uuid,
    pub database: String,
    pub agent: ProcessIdentity,
    pub ras: ProcessIdentity,
}

#[derive(Debug)]
pub(crate) struct Observation {
    pub binding: LifetimeBinding,
    /// Entire registration inventory, including disconnected records.
    pub registrations: BTreeSet<Uuid>,
    /// Observed loads are checked against the retained intent history; removing
    /// a registration or a connection never removes an admitted load.
    pub loaded: BTreeSet<Uuid>,
    pub worker: Option<(Uuid, ProcessIdentity)>,
    pub password_only_admins_exact: bool,
    pub authenticated_inventory_exact: bool,
    pub lease_original_handle_exact: bool,
    pub unknown_administration: bool,
}

/// This interface is crate-private. Its creator must own the original agent,
/// RAS and lease handles from birth; a current endpoint or a JSON certificate
/// is deliberately not an implementation of lifetime creation.
pub(crate) trait OwnedRuntime {
    fn observe(&mut self) -> Result<Observation>;
    fn register(&mut self) -> Result<Uuid>;
    fn load(&mut self, infobase: Uuid) -> Result<()>;
    fn turn_off(&mut self, worker_id: Uuid, identity: &ProcessIdentity) -> Result<()>;
    fn stop_owned(&mut self, _journal: &mut Journal) -> Result<()> {
        bail!("owned cold shutdown is unsupported by this runtime")
    }
    fn require_cold(&mut self) -> Result<()> {
        bail!("original owned cold lifetime is not proved")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[doc(hidden)]
pub enum PublicationState {
    NotStarted,
    StageUnproved,
    Staged,
    CommitUnproved,
    Committed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[doc(hidden)]
pub struct ManagedOutcome {
    pub publication: PublicationState,
    pub handoff_proved: bool,
    pub diagnostic: Option<String>,
}

/// An exclusive original journal handle is retained for the entire session.
/// Records are flushed before dispatch. Any failed append poisons authority;
/// there is no replay API which turns a user's journal into a live capability.
#[doc(hidden)]
pub struct Journal {
    file: File,
    path: PathBuf,
    length: u64,
    sequence: u64,
    nonce: Uuid,
    failed: bool,
}

impl Journal {
    pub(crate) fn create(path: &Path, nonce: Uuid) -> Result<Self> {
        if !path.is_absolute() || nonce.is_nil() {
            bail!("fully qualified fresh managed journal and nonce required");
        }
        // The creator separately validates the fresh root and its ancestry.
        // Creation cannot truncate any historical journal.
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let file = options
            .open(path)
            .context("create exclusive managed journal")?;
        Ok(Self {
            file,
            path: path.to_owned(),
            length: 0,
            sequence: 0,
            nonce,
            failed: false,
        })
    }

    /// The journal's path, for a diagnostic. Reading it grants no authority.
    #[doc(hidden)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The lifetime's nonce the journal records, for a diagnostic.
    #[doc(hidden)]
    pub fn nonce(&self) -> Uuid {
        self.nonce
    }

    fn append(&mut self, event: &str, value: impl Serialize) -> Result<()> {
        if self.failed {
            bail!("managed journal already unproved");
        }
        let result = (|| -> Result<()> {
            self.require_original()?;
            self.sequence = self
                .sequence
                .checked_add(1)
                .context("journal sequence overflow")?;
            let bytes = serde_json::to_vec(&serde_json::json!({
                "nonce": self.nonce.to_string(), "sequence": self.sequence,
                "event": event, "value": value,
            }))?;
            if bytes.len() > 64 * 1024 {
                bail!("managed journal record exceeds bound");
            }
            self.file.seek(SeekFrom::End(0))?;
            self.file.write_all(&bytes)?;
            self.file.write_all(b"\n")?;
            self.file.sync_all()?;
            self.length = self
                .length
                .checked_add(bytes.len() as u64 + 1)
                .context("journal byte count overflow")?;
            Ok(())
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn require_original(&self) -> Result<()> {
        if self.failed
            || !self.file.metadata()?.is_file()
            || self.file.metadata()?.len() != self.length
        {
            bail!("original managed journal handle or length drift");
        }
        for path in self.path.ancestors() {
            let metadata = std::fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() {
                bail!("managed journal ancestry replaced");
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    bail!("managed journal reparse ancestry");
                }
            }
        }
        // share_mode(0) prevents another Windows open, rename or replacement
        // during this original file's lifetime. Compare pathname attributes
        // without opening a second, conflicting handle.
        let current = std::fs::symlink_metadata(&self.path)?;
        let original = self.file.metadata()?;
        if !current.is_file()
            || current.len() != self.length
            || current.created()? != original.created()?
        {
            bail!("managed journal pathname no longer names original exclusive file");
        }
        Ok(())
    }
}

#[doc(hidden)]
// `OwnedRuntime` stays crate-private: no other crate implements a runtime.
#[allow(private_bounds)]
pub struct ManagedWorker<R: OwnedRuntime> {
    runtime: R,
    binding: LifetimeBinding,
    journal: Journal,
    ever_loaded: BTreeSet<Uuid>,
    registered: bool,
    admitted_worker: Option<(Uuid, ProcessIdentity)>,
    tainted: bool,
    stopped: bool,
}

#[allow(private_bounds)]
impl<R: OwnedRuntime> ManagedWorker<R> {
    /// Called only by the product creator after its authenticated fresh-birth
    /// barrier. The first observation must have no registrations or loads.
    #[cfg(test)]
    fn from_fresh_creator(runtime: R, binding: LifetimeBinding, journal: Journal) -> Result<Self> {
        if binding.nonce.is_nil()
            || binding.cluster.is_nil()
            || binding.database.is_empty()
            || !binding.root.is_absolute()
            || journal.nonce != binding.nonce
        {
            bail!("fresh managed creator identity incomplete");
        }
        let mut session = Self {
            runtime,
            binding,
            journal,
            ever_loaded: BTreeSet::new(),
            registered: false,
            admitted_worker: None,
            tainted: false,
            stopped: false,
        };
        let observation = session.checked_observation()?;
        if !observation.registrations.is_empty()
            || !observation.loaded.is_empty()
            || observation.worker.is_some()
        {
            bail!("managed creator requires empty lifetime at authenticated birth barrier");
        }
        session
            .journal
            .append("creator_barrier", session.binding.nonce.to_string())?;
        Ok(session)
    }

    /// The lifetime this worker is bound to, for the command glue's target
    /// and diagnostics. Reading it grants no authority.
    #[doc(hidden)]
    pub fn binding(&self) -> &LifetimeBinding {
        &self.binding
    }

    /// The exclusive journal this worker appends to, for a diagnostic.
    #[doc(hidden)]
    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    fn checked_observation(&mut self) -> Result<Observation> {
        if self.tainted || self.journal.failed || self.stopped {
            bail!("managed lifetime already unproved; retained journal requires recovery");
        }
        let result = (|| -> Result<Observation> {
            self.journal.require_original()?;
            let o = self.runtime.observe()?;
            if o.binding != self.binding
                || !o.password_only_admins_exact
                || !o.authenticated_inventory_exact
                || !o.lease_original_handle_exact
                || o.unknown_administration
            {
                bail!("managed identity, authenticated administration or original lease drift");
            }
            if o.loaded.iter().any(|id| !self.ever_loaded.contains(id))
                || o.registrations
                    .iter()
                    .any(|id| *id != self.binding.infobase)
                || self
                    .ever_loaded
                    .iter()
                    .any(|id| *id != self.binding.infobase)
            {
                bail!("second or unjournaled infobase in managed lifetime");
            }
            if o.worker.as_ref().is_some_and(|(id, worker)| {
                id.is_nil()
                    || worker.pid == 0
                    || worker.parent == 0
                    || worker.birth_100ns == 0
                    || !worker.executable.is_absolute()
                    || worker.command_sha256.len() != 64
                    || !worker.command_sha256.bytes().all(|c| c.is_ascii_hexdigit())
            }) {
                bail!("complete original working-process identity required");
            }
            Ok(o)
        })();
        if result.is_err() {
            self.tainted = true;
        }
        result
    }

    #[doc(hidden)]
    pub fn register_and_load(&mut self) -> Result<()> {
        self.checked_observation()?;
        if self.registered || !self.ever_loaded.is_empty() {
            bail!("single managed registration/load only");
        }
        // Record all potential effects before the child can do anything.
        self.journal
            .append("registration_intent", &self.binding.database)?;
        let id = match self.runtime.register() {
            Ok(id) if !id.is_nil() => id,
            result => {
                self.tainted = true;
                return Err(result
                    .err()
                    .unwrap_or_else(|| anyhow::anyhow!("nil registration identity"))
                    .context("registration outcome unproved; intent retained"));
            }
        };
        self.binding.infobase = id;
        self.registered = true;
        self.journal
            .append("registration_confirmed", id.to_string())?;
        let o = self.checked_observation()?;
        if o.registrations != BTreeSet::from([id]) {
            self.tainted = true;
            bail!("exact managed registration missing");
        }
        self.journal.append("load_intent", id.to_string())?;
        self.ever_loaded.insert(id); // Intent history survives failed load too.
        if let Err(e) = self.runtime.load(id) {
            self.tainted = true;
            return Err(e.context("load outcome unproved; admitted lifetime retained"));
        }
        self.journal.append("load_confirmed", id.to_string())?;
        self.prepare()?;
        Ok(())
    }

    #[doc(hidden)]
    pub fn prepare(&mut self) -> Result<ProcessIdentity> {
        let o = self.checked_observation()?;
        if !self.registered
            || self.ever_loaded != BTreeSet::from([self.binding.infobase])
            || o.registrations != BTreeSet::from([self.binding.infobase])
        {
            bail!(
                "sole managed target registration and retained load intent required before stage"
            );
        }
        let current = o
            .worker
            .context("sole owned working process required before stage")?;
        if self
            .admitted_worker
            .as_ref()
            .is_some_and(|old| old != &current)
        {
            self.tainted = true;
            bail!(
                "original managed working UUID or complete process identity drift before publication"
            );
        }
        self.admitted_worker = Some(current.clone());
        Ok(current.1)
    }

    #[doc(hidden)]
    pub fn stage_source<T>(&mut self, stage: impl FnOnce() -> Result<T>) -> Result<T> {
        let worker = self.prepare()?;
        self.journal.append("source_stage_intent", &worker)?;
        match stage() {
            Ok(value) => {
                self.journal.append("source_stage_confirmed", &worker)?;
                Ok(value)
            }
            Err(error) => {
                self.tainted = true;
                self.journal.append("source_stage_unproved", &worker)?;
                Err(error.context("managed source stage outcome unproved; journal retained"))
            }
        }
    }

    /// Retains the original runtime and exclusive journal on failure. There is
    /// no automatic Drop cleanup, replay constructor or PID-absence shortcut.
    #[doc(hidden)]
    pub fn shutdown(&mut self) -> Result<ColdSession<'_, R>> {
        self.prepare()?;
        self.journal
            .append("cold_shutdown_intent", self.binding.nonce.to_string())?;
        let result = self.runtime.stop_owned(&mut self.journal);
        // Even a fully proved stop invalidates every publication capability.
        self.stopped = true;
        if let Err(error) = result {
            self.tainted = true;
            self.journal.append("cold_shutdown_unproved", ())?;
            return Err(error.context("owned shutdown unproved; lifetime retained"));
        }
        if let Err(error) = self.runtime.require_cold() {
            self.tainted = true;
            self.journal.append("cold_shutdown_unproved", ())?;
            return Err(error);
        }
        self.journal.append("cold_shutdown_confirmed", ())?;
        Ok(ColdSession {
            session: self,
            undo_attempted: false,
        })
    }

    /// Real orchestration seam: stage and commit callbacks are unreachable on
    /// ownership refusal. Uncertain SQL and post-commit handoff are returned
    /// as distinct states; neither can be rewritten as a successful rollback.
    #[doc(hidden)]
    pub fn publish_and_handoff(
        &mut self,
        stage: impl FnOnce() -> Result<()>,
        commit: impl FnOnce() -> Result<()>,
    ) -> Result<ManagedOutcome> {
        let original = self.prepare()?;
        let original_id = self
            .admitted_worker
            .as_ref()
            .context("original managed worker UUID absent")?
            .0;
        self.journal.append("stage_intent", &original)?;
        let mut state = PublicationState::StageUnproved;
        let result = (|| -> Result<()> {
            stage()?;
            state = PublicationState::Staged;
            self.journal.append("stage_confirmed", &original)?;
            if self.prepare()? != original {
                bail!("worker changed before commit");
            }
            self.journal.append("commit_intent", &original)?;
            state = PublicationState::CommitUnproved;
            commit()?;
            state = PublicationState::Committed;
            self.journal.append("commit_confirmed", &original)?;
            let o = self.checked_observation()?;
            let (worker_id, current) = o.worker.context("working process missing after commit")?;
            if current != original || worker_id != original_id {
                bail!("worker changed after commit; handoff unproved");
            }
            self.journal
                .append("handoff_intent", (&original, worker_id.to_string()))?;
            self.runtime.turn_off(worker_id, &original)?;
            self.journal
                .append("handoff_command_confirmed", &original)?;
            // Successful RAC exit is not proof of a replacement process.
            let replacement = self.checked_observation()?;
            let (replacement_id, replacement_worker) =
                replacement.worker.context("replacement process missing")?;
            if replacement_worker == original || replacement_id == worker_id {
                bail!("handoff command returned but replacement not proved");
            }
            self.journal
                .append("replacement_confirmed", &replacement_worker)?;
            self.admitted_worker = Some((replacement_id, replacement_worker));
            Ok(())
        })();
        if let Err(error) = result {
            self.tainted = true;
            let outcome = ManagedOutcome {
                publication: state,
                handoff_proved: false,
                diagnostic: Some(error.to_string()),
            };
            self.journal
                .append("publication_retained_unproved", &outcome)?;
            return Ok(outcome);
        }
        Ok(ManagedOutcome {
            publication: state,
            handoff_proved: true,
            diagnostic: None,
        })
    }
}

/// A borrowed live authority, never serialized or recovered from a user's
/// certificate. The runtime/lease and original exited handles stay retained.
#[doc(hidden)]
#[allow(private_bounds)]
pub struct ColdSession<'a, R: OwnedRuntime> {
    session: &'a mut ManagedWorker<R>,
    // Read by the undo's executor seam, which no command runs yet.
    #[allow(dead_code)]
    undo_attempted: bool,
}

#[allow(private_bounds)]
impl<R: OwnedRuntime> ColdSession<'_, R> {
    #[doc(hidden)]
    pub fn require_current(&mut self) -> Result<()> {
        self.session.journal.require_original()?;
        if self.session.tainted || !self.session.stopped {
            bail!("original cold lifetime authority lost");
        }
        let result = self.session.runtime.require_cold();
        if result.is_err() {
            self.session.tainted = true;
        }
        result
    }

    /// Internal executor seam only. The caller's transaction must perform
    /// database identity and full published-postimage CAS before its writes.
    /// Unknown SQL outcome never allows a second dispatch with this authority.
    #[allow(dead_code)] // No command runs the guarded undo yet.
    pub(crate) fn run_cold_transaction_once<T>(
        &mut self,
        undo: impl FnOnce(&LifetimeBinding) -> Result<T>,
    ) -> Result<T> {
        if self.undo_attempted {
            bail!("undo already dispatched; retained outcome must be inspected");
        }
        self.require_current()?;
        self.session.journal.append("guarded_undo_intent", ())?;
        self.undo_attempted = true;
        match undo(&self.session.binding) {
            Ok(value) => {
                self.session
                    .journal
                    .append("guarded_undo_sql_confirmed", ())?;
                // Report a late restarted process as failure, never silently
                // claim the committed SQL was rolled back.
                self.require_current()
                    .context("undo SQL confirmed but cold postcondition unproved")?;
                Ok(value)
            }
            Err(error) => {
                self.session.tainted = true;
                self.session
                    .journal
                    .append("guarded_undo_sql_unproved", ())?;
                Err(error.context("undo SQL outcome unproved; no automatic retry"))
            }
        }
    }
}

/// The target a managed command names, as plain values. The command's
/// arguments are the root CLI's; its glue (`cli.rs`) builds this from them,
/// so the lifetime authority checks a target without reading CLI types.
#[derive(Debug, Clone, Copy)]
#[doc(hidden)]
pub struct ManagedTarget<'a> {
    /// The command's activation mode is `worker`.
    pub worker_mode: bool,
    /// The command names a `sqlcmd` executable.
    pub sqlcmd: bool,
    /// The command names a `bcp` executable.
    pub bcp_executable: bool,
    /// The command names an extension (a source change only).
    pub extension: bool,
    pub database: &'a str,
    pub cluster_id: Option<Uuid>,
    pub infobase_id: Option<Uuid>,
    pub rac: &'a Path,
    pub ras_endpoint: &'a str,
    pub server: &'a str,
}

impl NativeManagedWorker {
    #[doc(hidden)]
    pub fn require_source_target(&mut self, target: &ManagedTarget<'_>) -> Result<()> {
        if !target.worker_mode
            || target.sqlcmd
            || target.bcp_executable
            || target.extension
            || target.database != self.binding.database
            || target.cluster_id != Some(self.binding.cluster)
            || target.infobase_id != Some(self.binding.infobase)
        {
            bail!("managed source target differs from internally created lifetime");
        }
        self.runtime
            .require_endpoint(target.rac, target.ras_endpoint, target.server)?;
        self.prepare()?;
        Ok(())
    }
    #[doc(hidden)]
    pub fn verify_target_profile(
        &mut self,
        claimed: crate::mssql_platform_profile::MssqlNativePlatformProfile,
        options: crate::mssql_platform_profile::MssqlNativeProfileVerificationOptions<'_>,
    ) -> Result<crate::mssql_platform_profile::MssqlNativeProfileVerification> {
        self.prepare()?;
        self.runtime.verify_target_profile(claimed, options)
    }

    /// Reads every field of `target` but `extension`, which an activation does
    /// not name.
    #[doc(hidden)]
    pub fn require_activation_target(&mut self, target: &ManagedTarget<'_>) -> Result<()> {
        if !target.worker_mode
            || target.sqlcmd
            || target.bcp_executable
            || target.database != self.binding.database
            || target.cluster_id != Some(self.binding.cluster)
            || target.infobase_id != Some(self.binding.infobase)
        {
            bail!("managed activation target differs from internally created lifetime");
        }
        self.runtime
            .require_endpoint(target.rac, target.ras_endpoint, target.server)?;
        self.prepare()?;
        Ok(())
    }
}
