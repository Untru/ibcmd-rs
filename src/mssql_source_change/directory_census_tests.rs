//! Strict directory membership controls; no compiler or database fixture.
use super::*;

// Removal from the source census uses an owned parking sibling. Exact KEEP1
// retains this fresh fixture; ordinary test runs clean it after releasing custody.
struct Fixture {
    root: PathBuf,
    source: PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").as_deref()
            != Some(std::ffi::OsStr::new("1"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("ibcmd-directory-census-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source");
        fs::create_dir(&source).unwrap();
        fs::create_dir(root.join("parking")).unwrap();
        fs::write(source.join("keep.bin"), b"unchanged public bytes").unwrap();
        fs::create_dir(source.join("empty-before")).unwrap();
        Self { root, source }
    }
    fn capture(&self) -> HeldSourceRoot {
        HeldSourceRoot::open_with_traversal_limits(
            &self.source,
            SourceInventoryLimits {
                max_files: 8,
                max_file_bytes: 1024,
                max_total_bytes: 4096,
            },
            SourceTraversalLimits {
                max_entries: 32,
                max_depth: 8,
            },
        )
        .unwrap()
    }
    fn assert_refused(&self, held: &HeldSourceRoot) {
        assert_eq!(
            fs::read(self.source.join("keep.bin")).unwrap(),
            b"unchanged public bytes"
        );
        assert!(
            held.require_unchanged().is_err(),
            "directory membership changed below traversal limits but revalidation accepted it"
        );
    }
}

#[test]
fn unchanged_directory_census_is_accepted() {
    let fixture = Fixture::new();
    let held = fixture.capture();
    held.require_unchanged().unwrap();
    held.require_unchanged().unwrap();
}

#[test]
fn empty_directory_addition_invalidates_held_census() {
    let fixture = Fixture::new();
    let held = fixture.capture();
    fs::create_dir(fixture.source.join("empty-after")).unwrap();
    fixture.assert_refused(&held);
}

#[test]
fn empty_directory_membership_removal_invalidates_held_census() {
    let fixture = Fixture::new();
    let held = fixture.capture();
    fs::rename(
        fixture.source.join("empty-before"),
        fixture.root.join("parking/empty-before"),
    )
    .unwrap();
    fixture.assert_refused(&held);
}

#[test]
fn empty_directory_rename_invalidates_held_census() {
    let fixture = Fixture::new();
    let held = fixture.capture();
    fs::rename(
        fixture.source.join("empty-before"),
        fixture.source.join("renamed-after"),
    )
    .unwrap();
    fixture.assert_refused(&held);
}

#[test]
fn nested_empty_directory_rename_invalidates_held_census() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.source.join("empty-before/nested-before")).unwrap();
    let held = fixture.capture();
    fs::rename(
        fixture.source.join("empty-before/nested-before"),
        fixture.source.join("empty-before/nested-after"),
    )
    .unwrap();
    fixture.assert_refused(&held);
}

#[test]
fn case_only_empty_directory_rename_invalidates_held_census() {
    let fixture = Fixture::new();
    let held = fixture.capture();
    fs::rename(
        fixture.source.join("empty-before"),
        fixture.source.join("EMPTY-before"),
    )
    .unwrap();
    assert!(
        fs::read_dir(&fixture.source)
            .unwrap()
            .any(|entry| entry.unwrap().file_name() == "EMPTY-before")
    );
    fixture.assert_refused(&held);
}

#[test]
fn directory_replaced_by_file_is_not_unchanged() {
    let fixture = Fixture::new();
    let held = fixture.capture();
    fs::rename(
        fixture.source.join("empty-before"),
        fixture.root.join("parking/empty-before"),
    )
    .unwrap();
    fs::write(
        fixture.source.join("empty-before"),
        b"different member type",
    )
    .unwrap();
    fixture.assert_refused(&held);
}

#[test]
fn same_name_directory_replacement_invalidates_held_census() {
    let fixture = Fixture::new();
    let held = fixture.capture();
    fs::rename(
        fixture.source.join("empty-before"),
        fixture.root.join("parking/original-directory"),
    )
    .unwrap();
    fs::create_dir(fixture.source.join("empty-before")).unwrap();
    fixture.assert_refused(&held);
}

#[test]
fn file_only_empty_inventory_cannot_stand_for_filesystem_empty_census() {
    let fixture = Fixture::new();
    for name in ["keep.bin", "empty-before"] {
        fs::rename(
            fixture.source.join(name),
            fixture.root.join("parking").join(name),
        )
        .unwrap();
    }
    let held = fixture.capture();
    let captured = held.baseline();
    let unbound = SourceInventory::from_files(Vec::new()).unwrap();
    assert_eq!(captured.files().len(), 0);
    assert!(matches!(
        &captured.directories,
        DirectoryCensus::Filesystem(directories) if directories.is_empty()
    ));
    assert!(matches!(&unbound.directories, DirectoryCensus::Unbound));
    assert_eq!(captured, &unbound, "semantic file equality changed");
    assert!(captured.require_same_directories(&unbound).is_err());
    assert!(unbound.require_same_directories(captured).is_err());
    assert!(unbound.require_same_directories(&unbound).is_err());
    held.require_unchanged().unwrap();
}

#[test]
fn directory_proof_clone_is_shared_but_digest_projection_is_unbound() {
    let fixture = Fixture::new();
    let held = fixture.capture();
    let captured = held.baseline();
    let cloned = captured.clone();
    let (DirectoryCensus::Filesystem(before), DirectoryCensus::Filesystem(after)) =
        (&captured.directories, &cloned.directories)
    else {
        panic!("successful filesystem capture has no directory proof");
    };
    assert!(
        Arc::ptr_eq(before, after),
        "clone duplicated directory metadata"
    );
    captured.require_same_directories(&cloned).unwrap();
    let projection = captured.digest_projection();
    assert_eq!(
        captured, &projection,
        "digest projection changed file values"
    );
    assert!(matches!(&projection.directories, DirectoryCensus::Unbound));
    assert!(captured.require_same_directories(&projection).is_err());
    let unbound = SourceInventory::from_files(captured.files().cloned().collect()).unwrap();
    assert_eq!(captured, &unbound);
    assert!(captured.require_same_directories(&unbound).is_err());
}

#[test]
fn file_and_directory_aliases_share_one_logical_namespace() {
    let fixture = Fixture::new();
    let held = fixture.capture();
    let captured = held.baseline();
    let DirectoryCensus::Filesystem(directories) = &captured.directories else {
        panic!("successful filesystem capture has no directory proof");
    };
    // The same admission is used before either file or directory insertion.
    // These synthetic candidate spellings work on case-insensitive hosts too.
    for (candidate, original) in [("KEEP.bin", "keep.bin"), ("EMPTY-before", "empty-before")] {
        let error = require_distinct_source_member(
            &captured.by_windows_key,
            directories,
            &windows_path_key(candidate),
            candidate,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            SourceChangeError::WindowsPathCollision { first, second }
                if first == original && second == candidate
        ));
    }
}

#[test]
fn census_retention_budget_charges_files_and_directories_cumulatively() {
    let file = CensusRetentionBudget::charge(CensusMemberKind::File, 8, 8).unwrap();
    let directory = CensusRetentionBudget::charge(CensusMemberKind::Directory, 12, 12).unwrap();
    let mut budget = CensusRetentionBudget::new(2, file + directory);
    budget.admit(CensusMemberKind::File, 8, 8).unwrap();
    assert_eq!((budget.entries_left, budget.metadata_left), (1, directory));
    budget.admit(CensusMemberKind::Directory, 12, 12).unwrap();
    assert_eq!((budget.entries_left, budget.metadata_left), (0, 0));
    assert!(matches!(
        budget.admit(CensusMemberKind::File, 1, 1),
        Err(SourceChangeError::InventoryLimit(
            "filesystem census entry count"
        ))
    ));
}

#[test]
fn census_metadata_boundary_and_overflow_refuse_before_charging() {
    let charge = CensusRetentionBudget::charge(CensusMemberKind::Directory, 6, 6).unwrap();
    let mut exact = CensusRetentionBudget::new(1, charge);
    exact.admit(CensusMemberKind::Directory, 6, 6).unwrap();
    assert_eq!((exact.entries_left, exact.metadata_left), (0, 0));
    let mut short = CensusRetentionBudget::new(1, charge - 1);
    assert!(matches!(
        short.admit(CensusMemberKind::Directory, 6, 6),
        Err(SourceChangeError::InventoryLimit(
            "filesystem census metadata bytes"
        ))
    ));
    assert_eq!((short.entries_left, short.metadata_left), (1, charge - 1));
    let mut overflow = CensusRetentionBudget::new(1, usize::MAX);
    assert!(matches!(
        overflow.admit(CensusMemberKind::File, usize::MAX, 1),
        Err(SourceChangeError::InventoryLimit(
            "filesystem census metadata size overflow"
        ))
    ));
    assert_eq!(
        (overflow.entries_left, overflow.metadata_left),
        (1, usize::MAX)
    );
}

#[test]
fn census_metadata_charge_includes_both_string_lengths() {
    for kind in [CensusMemberKind::File, CensusMemberKind::Directory] {
        let original = CensusRetentionBudget::charge(kind, 5, 7).unwrap();
        assert_eq!(
            CensusRetentionBudget::charge(kind, 6, 7).unwrap(),
            original + 1
        );
        assert_eq!(
            CensusRetentionBudget::charge(kind, 5, 8).unwrap(),
            original + 1
        );
    }
}
