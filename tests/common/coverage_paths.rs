use std::path::PathBuf;

pub fn root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/1c-fixtures");
    assert!(
        root.join("corpus-index.json").is_file(),
        "1c-fixtures is missing; run git submodule update --init --recursive"
    );
    root
}
