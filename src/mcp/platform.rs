//! Portable admission predicates, never a substitute for native descriptor custody.
//! No value in this module selects a backend from model-controlled tool arguments.
use std::fmt;

pub const MAX_RESULT: usize = 1024 * 1024;
pub const MAX_OBJECTS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Adapter {
    UnixDescriptors,
    Unsupported,
}

/// Only this host selector is used by the production MCP adapter.
pub const fn host_adapter() -> Adapter {
    if cfg!(unix) {
        Adapter::UnixDescriptors
    } else {
        Adapter::Unsupported
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    RowsFixture,
    FileDatabase,
    SqlServer,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Access {
    Read,
    Stage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    read: bool,
    stage: bool,
}
impl Capabilities {
    pub const fn allows(self, access: Access) -> bool {
        match access {
            Access::Read => self.read,
            Access::Stage => self.stage,
        }
    }
}

pub const fn capabilities(adapter: Adapter, target: Target) -> Capabilities {
    match (adapter, target) {
        (Adapter::UnixDescriptors, Target::FileDatabase) => Capabilities {
            read: true,
            stage: true,
        },
        (Adapter::UnixDescriptors, Target::RowsFixture) => Capabilities {
            read: true,
            stage: false,
        },
        _ => Capabilities {
            read: false,
            stage: false,
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    UnsupportedAdapter,
    UnsupportedTarget,
    FixedBindingRequired,
    ReadOnlyTarget,
    WriteOptInRequired,
    InvalidRelativePath,
    ArgumentOverride,
    UnknownTool,
    BindingChanged,
    SnapshotScopeUnavailable,
    BudgetExceeded,
    Cancelled,
}
impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Deliberately no filesystem paths, credentials, SQL or model input.
        f.write_str(match self {
            Self::UnsupportedAdapter => "secure MCP snapshot adapter unavailable",
            Self::UnsupportedTarget => "MCP target backend unavailable",
            Self::FixedBindingRequired => "MCP fixed operator binding required",
            Self::ReadOnlyTarget => "MCP target is read-only",
            Self::WriteOptInRequired => "MCP explicit operator agent-write required",
            Self::InvalidRelativePath => "invalid relative path",
            Self::ArgumentOverride => "parameter not allowed for this tool",
            Self::UnknownTool => "unknown tool",
            Self::BindingChanged => "target_changed: restart required",
            Self::SnapshotScopeUnavailable => "required snapshot scope unavailable",
            Self::BudgetExceeded => "resource limit exceeded",
            Self::Cancelled => "cancelled before publication",
        })
    }
}
impl std::error::Error for Refusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        static CANCELLED: crate::cancel::Cancelled = crate::cancel::Cancelled;
        match self {
            Self::Cancelled => Some(&CANCELLED),
            _ => None,
        }
    }
}

pub fn admit(
    adapter: Adapter,
    target: Target,
    fixed: bool,
    operator_write: bool,
    access: Access,
) -> Result<(), Refusal> {
    if adapter == Adapter::Unsupported {
        return Err(Refusal::UnsupportedAdapter);
    }
    if !fixed {
        return Err(Refusal::FixedBindingRequired);
    }
    if target == Target::SqlServer {
        return Err(Refusal::UnsupportedTarget);
    }
    if !capabilities(adapter, target).allows(access) {
        return Err(Refusal::ReadOnlyTarget);
    }
    if access == Access::Stage && !operator_write {
        return Err(Refusal::WriteOptInRequired);
    }
    Ok(())
}

/// Lexical syntax only. The real adapter must still prove native confinement.
pub fn validate_relative_path(path: &str) -> Result<(), Refusal> {
    if path.is_empty()
        || path.len() > 4096
        || path.contains(['\\', ':', '\0'])
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(Refusal::InvalidRelativePath);
    }
    Ok(())
}

/// The schema's single key is the entire model-controlled surface for each tool.
/// In particular a model cannot submit confirmation, credentials or target policy.
pub fn validate_argument_keys<'a>(
    tool: &str,
    keys: impl IntoIterator<Item = &'a str>,
) -> Result<(), Refusal> {
    let permitted = match tool {
        "tree" => "node",
        "read" => "path",
        "status" | "compare" | "export" | "import" => "objects",
        _ => return Err(Refusal::UnknownTool),
    };
    if keys.into_iter().any(|key| key != permitted) {
        return Err(Refusal::ArgumentOverride);
    }
    Ok(())
}

/// Equality is useful only when both witnesses come from authoritative snapshots.
/// A model-supplied path/id or a matching hash alone is not authorization.
pub fn verify_binding<T: Eq>(expected: &T, current: &T) -> Result<(), Refusal> {
    if expected != current {
        Err(Refusal::BindingChanged)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Budget {
    max_bytes: usize,
    max_entries: usize,
    bytes: usize,
    entries: usize,
}
impl Budget {
    pub const fn new(max_bytes: usize, max_entries: usize) -> Self {
        Self {
            max_bytes,
            max_entries,
            bytes: 0,
            entries: 0,
        }
    }
    pub fn charge(&mut self, bytes: usize, entries: usize) -> Result<(), Refusal> {
        let next_bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(Refusal::BudgetExceeded)?;
        let next_entries = self
            .entries
            .checked_add(entries)
            .ok_or(Refusal::BudgetExceeded)?;
        if next_bytes > self.max_bytes || next_entries > self.max_entries {
            return Err(Refusal::BudgetExceeded);
        }
        self.bytes = next_bytes;
        self.entries = next_entries;
        Ok(())
    }
    pub const fn used(&self) -> (usize, usize) {
        (self.bytes, self.entries)
    }
}

/// One cumulative control belongs to the operation, not a new budget per pager.
pub struct SnapshotControl<'a> {
    budget: Budget,
    cancelled: &'a dyn Fn() -> bool,
}
impl<'a> SnapshotControl<'a> {
    pub fn new(budget: Budget, cancelled: &'a dyn Fn() -> bool) -> Self {
        Self { budget, cancelled }
    }
    pub fn check(&self) -> Result<(), Refusal> {
        if (self.cancelled)() {
            Err(Refusal::Cancelled)
        } else {
            Ok(())
        }
    }
    pub fn charge(&mut self, bytes: usize, entries: usize) -> Result<(), Refusal> {
        self.check()?;
        self.budget.charge(bytes, entries)
    }
}

/// status/compare/import additionally require the fixed source root's complete
/// bounded census and final revalidation; a target-only snapshot cannot serve them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotScope {
    Target,
    TargetAndSource,
}

pub fn snapshot_scope(tool: &str) -> Result<SnapshotScope, Refusal> {
    match tool {
        "tree" | "read" | "export" => Ok(SnapshotScope::Target),
        "status" | "compare" | "import" => Ok(SnapshotScope::TargetAndSource),
        _ => Err(Refusal::UnknownTool),
    }
}

/// Required interface for future adapters; no default or Windows implementation.
/// Capture/revalidation must use admitted native handles and actual content,
/// preserving confinement, cumulative limits and cancellation before publication.
/// An opaque Identity is only an equality witness, never an authorization grant.
pub trait SnapshotAdapter {
    type Snapshot: BoundSnapshot;
    fn capture(
        &mut self,
        scope: SnapshotScope,
        control: &mut SnapshotControl<'_>,
    ) -> Result<Self::Snapshot, <Self::Snapshot as BoundSnapshot>::Error>;
}
pub trait BoundSnapshot {
    type Identity: Eq;
    type Error: std::error::Error;
    fn binding(&self) -> &Self::Identity;
    fn revalidate(&self, control: &mut SnapshotControl<'_>) -> Result<(), Self::Error>;
}
