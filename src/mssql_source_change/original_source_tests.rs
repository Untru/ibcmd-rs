//! Production-owned original-source controls; no SQL or native platform.
use super::*;
use crate::module_blob::{MetadataSourceContext, SourceBytes};
use std::io::Write;

struct Fixture {
    path: PathBuf,
    parent: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let parent = std::env::temp_dir();
        let path = parent.join(format!("ibcmd-original-source-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self { path, parent }
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        let path = self.path.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn original(&self) -> Arc<HeldSourceRoot> {
        Arc::new(HeldSourceRoot::open_compiler_operation(&self.path).unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only this newly created UUID directory can be removed by this test.
        assert_eq!(self.path.parent(), Some(self.parent.as_path()));
        assert!(
            self.path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("ibcmd-original-source-")
        );
        if self.path.is_dir() {
            fs::remove_dir_all(&self.path).unwrap();
        }
    }
}

#[test]
fn streamed_census_then_same_original_shared_consumption() {
    let fixture = Fixture::new();
    let bytes = (0..196_613).map(|n| (n % 251) as u8).collect::<Vec<_>>();
    fixture.write("Files/payload.bin", &bytes);
    let original = fixture.original();
    let member = original
        .baseline()
        .file("Files/payload.bin")
        .unwrap()
        .unwrap();
    assert_eq!(member.size_bytes(), bytes.len() as u64);
    assert_eq!(member.sha256(), &<[u8; 32]>::from(Sha256::digest(&bytes)));
    assert!(member.verified_bytes.is_none());
    assert!(member.original.is_none());
    let context = MetadataSourceContext::with_original_source(original.clone());
    let first = context
        .read_source(&fixture.path.join("Files/payload.bin"))
        .unwrap();
    let second = context
        .read_source(&fixture.path.join("Files/payload.bin"))
        .unwrap();
    let (SourceBytes::Shared(first), SourceBytes::Shared(second)) = (first, second) else {
        panic!("not the admitted original-byte owner");
    };
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.as_slice(), bytes);
    assert_eq!(original.consumed.lock().unwrap().len(), 1);
    context.require_original_unchanged().unwrap();
}

#[test]
fn explicit_caller_budget_retained_normal_operation_has_no_budget_sentinel() {
    let fixture = Fixture::new();
    fixture.write("one.bin", b"abcdef");
    fixture.write("two.bin", b"ghijkl");
    assert!(matches!(
        HeldSourceRoot::open(
            &fixture.path,
            SourceInventoryLimits {
                max_files: 1,
                max_file_bytes: 6,
                max_total_bytes: 6,
            }
        ),
        Err(SourceChangeError::InventoryLimit(_))
    ));
    let original = fixture.original();
    assert_eq!(original.baseline().files().len(), 2);
    original.require_unchanged().unwrap();
}

#[test]
fn selected_classification_uses_one_immutable_original_owner() {
    let fixture = Fixture::new();
    let selected = "CommonModules/M/Ext/Module.bsl";
    fixture.write("Configuration.xml", b"root");
    fixture.write("CommonModules/M.xml", b"metadata");
    fixture.write(selected, b"new");
    let original = HeldSourceRoot::open_source_operation(&fixture.path, selected).unwrap();
    let active = SourceInventory::from_files(vec![
        SourceFileDigest::for_bytes("Configuration.xml", b"root").unwrap(),
        SourceFileDigest::for_bytes("CommonModules/M.xml", b"metadata").unwrap(),
        SourceFileDigest::for_bytes(selected, b"old").unwrap(),
    ])
    .unwrap();
    let classified = classify_source_change(
        &active,
        original.baseline(),
        selected,
        ActivationTarget::Main,
        ActivationMode::Online,
    )
    .unwrap();
    let bytes = original.source_bytes(selected).unwrap();
    assert_eq!(classified.state(), SourceChangeState::Changed);
    assert!(std::ptr::eq(
        classified
            .verified_source(selected)
            .unwrap()
            .bytes()
            .as_ptr(),
        bytes.as_ptr()
    ));
    assert_eq!(bytes.as_slice(), b"new");
    original.require_unchanged().unwrap();
}

#[test]
fn original_context_refuses_absent_outside_and_case_alias_without_disk_fallback() {
    let fixture = Fixture::new();
    fixture.write("Files/Exact.bin", b"bound");
    let original = fixture.original();
    let context = MetadataSourceContext::with_original_source(original);
    assert_eq!(
        &*context
            .read_source(&fixture.path.join("Files/Exact.bin"))
            .unwrap(),
        b"bound"
    );
    fixture.write("Files/late.bin", b"never admitted");
    for path in [
        fixture.path.join("Files/late.bin"),
        fixture.path.join("Files/exact.bin"),
        fixture.path.join("../outside.bin"),
    ] {
        assert!(context.read_source(&path).is_err(), "{}", path.display());
    }
}

#[test]
fn whole_census_detects_unconsumed_same_size_edit_and_new_member() {
    for add in [false, true] {
        let fixture = Fixture::new();
        fixture.write("used.bin", b"source");
        fixture.write("unused.bin", b"before");
        let original = fixture.original();
        let bytes = original.source_bytes("used.bin").unwrap();
        if add {
            fixture.write("new.bin", b"new");
        } else {
            fixture.write("unused.bin", b"change");
        }
        assert_eq!(bytes.as_slice(), b"source");
        assert!(original.require_unchanged().is_err());
    }
}

#[test]
fn lazy_consumer_refuses_actual_extent_growth_and_truncation() {
    for bytes in [b"x".as_slice(), b"longer payload".as_slice()] {
        let fixture = Fixture::new();
        fixture.write("payload.bin", b"initial");
        let original = fixture.original();
        fixture.write("payload.bin", bytes);
        assert!(original.source_bytes("payload.bin").is_err());
        assert!(original.require_unchanged().is_err());
    }
}

#[test]
fn fingerprint_stream_is_exact_old_name_size_content_order() {
    let fixture = Fixture::new();
    let bytes = vec![0x6b; 196_613];
    fixture.write("payload.bin", &bytes);
    let original = fixture.original();
    let paths = vec!["payload.bin".to_owned()];
    let mut expected = Sha256::new();
    expected.update((paths[0].len() as u64).to_le_bytes());
    expected.update(paths[0].as_bytes());
    expected.update((bytes.len() as u64).to_le_bytes());
    expected.update(&bytes);
    assert_eq!(
        original.fingerprint_paths(&paths).unwrap(),
        <[u8; 32]>::from(expected.finalize())
    );
    assert!(original.consumed.lock().unwrap().is_empty());
    assert!(
        original
            .baseline()
            .files()
            .all(|file| file.verified_bytes.is_none())
    );
}

#[test]
fn hardlink_alias_is_refused_before_original_consumption() {
    let fixture = Fixture::new();
    fixture.write("original.bin", b"one original");
    fs::hard_link(
        fixture.path.join("original.bin"),
        fixture.path.join("alias.bin"),
    )
    .unwrap();
    assert!(HeldSourceRoot::open_compiler_operation(&fixture.path).is_err());
}

#[cfg(windows)]
#[test]
fn windows_required_original_and_ancestors_exclude_write_delete_share() {
    let fixture = Fixture::new();
    fixture.write("Files/payload.bin", b"bound");
    let original = fixture.original();
    let bytes = original.source_bytes("Files/payload.bin").unwrap();
    assert_eq!(bytes.as_slice(), b"bound");
    assert!(fs::write(fixture.path.join("Files/payload.bin"), b"other").is_err());
    assert!(fs::remove_file(fixture.path.join("Files/payload.bin")).is_err());
    assert!(fs::rename(fixture.path.join("Files"), fixture.path.join("Moved")).is_err());
    original.require_unchanged().unwrap();
    drop(original);
    fs::write(fixture.path.join("Files/payload.bin"), b"other").unwrap();
}

#[cfg(unix)]
#[test]
fn unix_cached_bytes_stay_original_and_publication_refuses_current_write() {
    let fixture = Fixture::new();
    fixture.write("payload.bin", b"before");
    let original = fixture.original();
    let context = MetadataSourceContext::with_original_source(original);
    let first = context
        .read_source(&fixture.path.join("payload.bin"))
        .unwrap();
    fixture.write("payload.bin", b"change");
    assert_eq!(&*first, b"before");
    assert_eq!(
        &*context
            .read_source(&fixture.path.join("payload.bin"))
            .unwrap(),
        b"before"
    );
    assert!(context.require_original_unchanged().is_err());
    fixture.write("payload.bin", b"before");
    assert_eq!(
        &*context
            .read_source(&fixture.path.join("payload.bin"))
            .unwrap(),
        b"before"
    );
}

#[cfg(unix)]
#[test]
fn unix_same_bytes_replacement_never_adopts_original_identity() {
    let fixture = Fixture::new();
    fixture.write("payload.bin", b"unchanged");
    let original = fixture.original();
    let first = original.source_bytes("payload.bin").unwrap();
    fs::rename(
        fixture.path.join("payload.bin"),
        fixture.path.join("old.bin"),
    )
    .unwrap();
    fixture.write("payload.bin", b"unchanged");
    assert_eq!(
        original.source_bytes("payload.bin").unwrap().as_slice(),
        first.as_slice()
    );
    assert!(original.require_unchanged().is_err());
}

#[cfg(unix)]
#[test]
fn unix_no_follow_root_ancestor_and_entry_and_case_directory_aliases() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    fixture.write("real/sub/file.bin", b"actual");
    symlink(fixture.path.join("real"), fixture.path.join("link")).unwrap();
    assert!(HeldSourceRoot::open_compiler_operation(&fixture.path.join("link")).is_err());
    assert!(HeldSourceRoot::open_compiler_operation(&fixture.path.join("link/sub")).is_err());
    assert!(HeldSourceRoot::open_compiler_operation(&fixture.path).is_err());
    fs::remove_file(fixture.path.join("link")).unwrap();
    fixture.write("Directory/one.bin", b"one");
    fixture.write("directory/two.bin", b"two");
    assert!(matches!(
        HeldSourceRoot::open_compiler_operation(&fixture.path),
        Err(SourceChangeError::WindowsPathCollision { .. })
    ));
}

fn write_actual_extent(path: &Path, length: u64) -> [u8; 32] {
    let mut file = fs::File::create(path).unwrap();
    let block = [0x63; 64 * 1024];
    let mut hash = Sha256::new();
    let mut remaining = length;
    while remaining > 0 {
        let count = usize::try_from(remaining.min(block.len() as u64)).unwrap();
        file.write_all(&block[..count]).unwrap();
        hash.update(&block[..count]);
        remaining -= count as u64;
    }
    file.sync_all().unwrap();
    hash.finalize().into()
}

#[test]
#[ignore = "ROOT-only actual >256MiB ordinary original file; ~257MiB disk and memory"]
fn actual_file_over_256_mib_hash_and_consumption() {
    let fixture = Fixture::new();
    let length = 256 * 1024 * 1024 + 1;
    let expected = write_actual_extent(&fixture.path.join("large.bin"), length);
    let original = fixture.original();
    let member = original.baseline().file("large.bin").unwrap().unwrap();
    assert_eq!(member.size_bytes(), length);
    assert_eq!(member.sha256(), &expected);
    let bytes = original.source_bytes("large.bin").unwrap();
    assert_eq!(bytes.len() as u64, length);
    assert_eq!(<[u8; 32]>::from(Sha256::digest(bytes.as_slice())), expected);
    original.require_unchanged().unwrap();
}

#[test]
#[ignore = "ROOT-only actual >4GiB tree; ~4.25GiB disk, streamed census and no body retention"]
fn actual_tree_over_4_gib_complete_streamed_inventory() {
    let fixture = Fixture::new();
    let length = 256 * 1024 * 1024;
    let mut expected = Vec::new();
    for index in 0..17 {
        expected.push(write_actual_extent(
            &fixture.path.join(format!("row{index}.bin")),
            length,
        ));
    }
    let original = fixture.original();
    assert_eq!(original.baseline().files().len(), 17);
    assert!(
        original
            .baseline()
            .files()
            .map(SourceFileDigest::size_bytes)
            .sum::<u64>()
            > 4 * 1024 * 1024 * 1024
    );
    for (index, expected) in expected.iter().enumerate() {
        let file = original
            .baseline()
            .file(&format!("row{index}.bin"))
            .unwrap()
            .unwrap();
        assert_eq!(file.sha256(), expected);
        assert!(file.verified_bytes.is_none());
    }
    original.require_unchanged().unwrap();
}

#[test]
#[ignore = "ROOT-only actual 500001 files; inode/directory space plus O(files) census metadata"]
fn actual_census_over_500000_files() {
    let fixture = Fixture::new();
    for index in 0..500_001 {
        fixture.write(&format!("d{:04}/f{index}.bin", index / 1000), b"x");
    }
    let original = fixture.original();
    assert_eq!(original.baseline().files().len(), 500_001);
    let expected = <[u8; 32]>::from(Sha256::digest(b"x"));
    assert!(
        original
            .baseline()
            .files()
            .all(|file| file.size_bytes() == 1
                && file.sha256() == &expected
                && file.verified_bytes.is_none())
    );
    original.require_unchanged().unwrap();
}
