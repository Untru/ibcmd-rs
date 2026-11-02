//! Portable in-memory policy contracts; these tests do not establish native platform support.
use ibcmd_rs::mcp::platform::{
    self, Access, Adapter, BoundSnapshot, Budget, Refusal, SnapshotAdapter, SnapshotControl,
    SnapshotScope, Target,
};
use std::{cell::Cell, rc::Rc};

#[test]
fn capability_matrix_and_operator_admission_are_distinct() {
    for (adapter, target, read, stage) in [
        (Adapter::UnixDescriptors, Target::RowsFixture, true, false),
        (Adapter::UnixDescriptors, Target::FileDatabase, true, true),
        (Adapter::UnixDescriptors, Target::SqlServer, false, false),
        (Adapter::Unsupported, Target::RowsFixture, false, false),
        (Adapter::Unsupported, Target::FileDatabase, false, false),
        (Adapter::Unsupported, Target::SqlServer, false, false),
    ] {
        let caps = platform::capabilities(adapter, target);
        assert_eq!(caps.allows(Access::Read), read);
        assert_eq!(caps.allows(Access::Stage), stage);
        for (access, available) in [(Access::Read, read), (Access::Stage, stage)] {
            for fixed in [false, true] {
                for opt_in in [false, true] {
                    let result = platform::admit(adapter, target, fixed, opt_in, access);
                    let expected = fixed && available && (access == Access::Read || opt_in);
                    assert_eq!(
                        result.is_ok(),
                        expected,
                        "{adapter:?}/{target:?}/{access:?}/{fixed}/{opt_in}"
                    );
                    if adapter == Adapter::Unsupported {
                        assert_eq!(result, Err(Refusal::UnsupportedAdapter));
                    }
                }
            }
        }
    }
    assert_eq!(
        platform::admit(
            Adapter::UnixDescriptors,
            Target::FileDatabase,
            true,
            false,
            Access::Read
        ),
        Ok(())
    );
    assert_eq!(
        platform::admit(
            Adapter::UnixDescriptors,
            Target::FileDatabase,
            true,
            false,
            Access::Stage
        ),
        Err(Refusal::WriteOptInRequired)
    );
    assert_eq!(
        platform::admit(
            Adapter::UnixDescriptors,
            Target::FileDatabase,
            true,
            true,
            Access::Stage
        ),
        Ok(())
    );
    assert_eq!(
        platform::admit(
            Adapter::UnixDescriptors,
            Target::RowsFixture,
            true,
            true,
            Access::Stage
        ),
        Err(Refusal::ReadOnlyTarget)
    );
    assert_eq!(
        platform::admit(
            Adapter::UnixDescriptors,
            Target::SqlServer,
            true,
            true,
            Access::Stage
        ),
        Err(Refusal::UnsupportedTarget)
    );
}

#[test]
fn production_host_selector_does_not_use_simulated_capabilities() {
    assert_eq!(
        platform::host_adapter(),
        if cfg!(unix) {
            Adapter::UnixDescriptors
        } else {
            Adapter::Unsupported
        }
    );
}

#[test]
fn every_tool_refuses_model_confirmation_and_target_policy_overrides() {
    for (tool, allowed) in [
        ("tree", "node"),
        ("read", "path"),
        ("status", "objects"),
        ("compare", "objects"),
        ("export", "objects"),
        ("import", "objects"),
    ] {
        assert_eq!(platform::validate_argument_keys(tool, [allowed]), Ok(()));
        assert_eq!(platform::validate_argument_keys(tool, []), Ok(()));
        for override_key in [
            "confirm",
            "settings",
            "policy",
            "database",
            "server",
            "db-path",
            "source-root",
            "agent-write",
            "auth",
            "password",
        ] {
            assert_eq!(
                platform::validate_argument_keys(tool, [allowed, override_key]),
                Err(Refusal::ArgumentOverride),
                "{tool}/{override_key}"
            );
        }
    }
    assert_eq!(
        platform::validate_argument_keys("import", ["path"]),
        Err(Refusal::ArgumentOverride)
    );
    assert_eq!(
        platform::validate_argument_keys("unrecognized", []),
        Err(Refusal::UnknownTool)
    );
}

#[test]
fn relative_path_syntax_has_the_same_contract_on_every_host() {
    for path in [
        "Configuration.xml",
        "CommonModules/Модуль/Ext/Module.bsl",
        ".hidden",
        "safe..name.xml",
    ] {
        assert_eq!(platform::validate_relative_path(path), Ok(()));
    }
    for path in [
        "",
        "/absolute",
        "//host/share",
        "C:/target",
        "C:relative",
        "\\\\host\\share",
        "a\\b",
        "../x",
        "a/../x",
        "a/./x",
        "a//x",
        "a/",
        "a\0b",
    ] {
        assert_eq!(
            platform::validate_relative_path(path),
            Err(Refusal::InvalidRelativePath),
            "{path:?}"
        );
    }
    assert_eq!(
        platform::validate_relative_path(&"a".repeat(4097)),
        Err(Refusal::InvalidRelativePath)
    );
}

#[test]
fn opaque_binding_equality_never_adopts_a_replacement() {
    assert_eq!(
        platform::verify_binding(&"held-native-witness", &"held-native-witness"),
        Ok(())
    );
    assert_eq!(
        platform::verify_binding(&"held-native-witness", &"replacement-witness"),
        Err(Refusal::BindingChanged)
    );
}

#[test]
fn cumulative_budget_is_checked_and_failed_charges_are_atomic() {
    let mut budget = Budget::new(8, 2);
    budget.charge(4, 1).unwrap();
    assert_eq!(budget.charge(5, 1), Err(Refusal::BudgetExceeded));
    assert_eq!(budget.used(), (4, 1));
    budget.charge(4, 1).unwrap();
    assert_eq!(budget.charge(0, 1), Err(Refusal::BudgetExceeded));
    assert_eq!(budget.used(), (8, 2));
    let mut overflow = Budget::new(usize::MAX, usize::MAX);
    overflow.charge(usize::MAX, usize::MAX).unwrap();
    assert_eq!(overflow.charge(1, 0), Err(Refusal::BudgetExceeded));
    assert_eq!(overflow.charge(0, 1), Err(Refusal::BudgetExceeded));
    assert_eq!(overflow.used(), (usize::MAX, usize::MAX));
}

// Synthetic witnesses represent no native handles and cannot authorize production I/O.
struct SyntheticAdapter {
    content: Rc<Cell<[u8; 4]>>,
    current: Rc<Cell<u64>>,
    reads: Rc<Cell<usize>>,
}
struct SyntheticSnapshot {
    content: [u8; 4],
    current_content: Rc<Cell<[u8; 4]>>,
    binding: u64,
    current: Rc<Cell<u64>>,
    reads: Rc<Cell<usize>>,
}
impl SnapshotAdapter for SyntheticAdapter {
    type Snapshot = SyntheticSnapshot;
    fn capture(
        &mut self,
        scope: SnapshotScope,
        control: &mut SnapshotControl<'_>,
    ) -> Result<Self::Snapshot, Refusal> {
        control.check()?;
        if scope != SnapshotScope::Target {
            return Err(Refusal::SnapshotScopeUnavailable);
        }
        control.charge(4, 1)?;
        self.reads.set(self.reads.get() + 1);
        Ok(SyntheticSnapshot {
            content: self.content.get(),
            current_content: self.content.clone(),
            binding: self.current.get(),
            current: self.current.clone(),
            reads: self.reads.clone(),
        })
    }
}
impl BoundSnapshot for SyntheticSnapshot {
    type Identity = u64;
    type Error = Refusal;
    fn binding(&self) -> &u64 {
        &self.binding
    }
    fn revalidate(&self, control: &mut SnapshotControl<'_>) -> Result<(), Refusal> {
        control.check()?;
        platform::verify_binding(self.binding(), &self.current.get())?;
        control.charge(4, 1)?;
        self.reads.set(self.reads.get() + 1);
        platform::verify_binding(&self.content, &self.current_content.get())
    }
}

#[test]
fn snapshot_contract_preserves_cumulative_limits_and_cancel_before_work() {
    let cancelled = Cell::new(false);
    let check = || cancelled.get();
    let mut control = SnapshotControl::new(Budget::new(8, 2), &check);
    let reads = Rc::new(Cell::new(0));
    let mut adapter = SyntheticAdapter {
        content: Rc::new(Cell::new(*b"base")),
        current: Rc::new(Cell::new(1)),
        reads: reads.clone(),
    };
    let snapshot = adapter
        .capture(SnapshotScope::Target, &mut control)
        .unwrap();
    snapshot.revalidate(&mut control).unwrap();
    assert_eq!(reads.get(), 2);
    assert_eq!(
        snapshot.revalidate(&mut control),
        Err(Refusal::BudgetExceeded)
    );
    assert_eq!(reads.get(), 2);
    cancelled.set(true);
    assert_eq!(snapshot.revalidate(&mut control), Err(Refusal::Cancelled));
    assert_eq!(
        adapter.capture(SnapshotScope::Target, &mut control).err(),
        Some(Refusal::Cancelled)
    );
    assert_eq!(reads.get(), 2);
}

#[test]
fn snapshot_binding_change_refuses_before_more_input_is_consumed() {
    let current = Rc::new(Cell::new(1));
    let reads = Rc::new(Cell::new(0));
    let mut adapter = SyntheticAdapter {
        content: Rc::new(Cell::new(*b"base")),
        current: current.clone(),
        reads: reads.clone(),
    };
    let not_cancelled = || false;
    let mut control = SnapshotControl::new(Budget::new(64, 16), &not_cancelled);
    let snapshot = adapter
        .capture(SnapshotScope::Target, &mut control)
        .unwrap();
    current.set(2);
    assert_eq!(
        snapshot.revalidate(&mut control),
        Err(Refusal::BindingChanged)
    );
    assert_eq!(*snapshot.binding(), 1);
    assert_eq!(reads.get(), 1);
}

#[test]
fn cancellation_marker_survives_context_without_misclassifying_denial() {
    let cancelled = anyhow::Error::from(Refusal::Cancelled).context("snapshot operation");
    assert!(ibcmd_rs::cancel::is_cancelled(&cancelled));
    let denied = anyhow::Error::from(Refusal::UnsupportedAdapter).context("snapshot operation");
    assert!(!ibcmd_rs::cancel::is_cancelled(&denied));
}

#[test]
fn same_binding_content_change_refuses_snapshot_revalidation() {
    let content = Rc::new(Cell::new(*b"base"));
    let reads = Rc::new(Cell::new(0));
    let mut adapter = SyntheticAdapter {
        content: content.clone(),
        current: Rc::new(Cell::new(1)),
        reads: reads.clone(),
    };
    let not_cancelled = || false;
    let mut control = SnapshotControl::new(Budget::new(8, 2), &not_cancelled);
    let snapshot = adapter
        .capture(SnapshotScope::Target, &mut control)
        .unwrap();
    content.set(*b"next");
    assert_eq!(*snapshot.binding(), 1);
    assert_eq!(snapshot.content, *b"base");
    assert_eq!(
        snapshot.revalidate(&mut control),
        Err(Refusal::BindingChanged)
    );
    assert_eq!(
        reads.get(),
        2,
        "revalidation reads were charged before content comparison"
    );
}

#[test]
fn source_operations_require_a_census_capability_beyond_target_read() {
    for tool in ["tree", "read", "export"] {
        assert_eq!(platform::snapshot_scope(tool), Ok(SnapshotScope::Target));
    }
    for tool in ["status", "compare", "import"] {
        assert_eq!(
            platform::snapshot_scope(tool),
            Ok(SnapshotScope::TargetAndSource)
        );
    }
    assert_eq!(
        platform::snapshot_scope("unknown"),
        Err(Refusal::UnknownTool)
    );
    let reads = Rc::new(Cell::new(0));
    let mut adapter = SyntheticAdapter {
        content: Rc::new(Cell::new(*b"base")),
        current: Rc::new(Cell::new(1)),
        reads: reads.clone(),
    };
    let not_cancelled = || false;
    let mut control = SnapshotControl::new(Budget::new(8, 2), &not_cancelled);
    for tool in ["status", "compare", "import"] {
        assert_eq!(
            adapter
                .capture(platform::snapshot_scope(tool).unwrap(), &mut control)
                .err(),
            Some(Refusal::SnapshotScopeUnavailable)
        );
    }
    assert_eq!(reads.get(), 0);
}
