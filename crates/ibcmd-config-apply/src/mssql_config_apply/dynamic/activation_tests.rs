//! A test of the main activation's script with the config apply's source-owner preimages
//! (`SourceOwnerPreimages::test_fixture`, a test fixture of `dynamic`), formerly the root test of
//! `src/mssql_main_activation.rs`, so that it goes with the config apply. Its fixture restates the
//! activation crate's test helpers.

use crate::mssql_main_activation::{
    MainActivationMode, MainActivationPlan, MainActivationSnapshot, MainStorageRow,
    prepare_main_activation, render_main_activation_sql,
};
use flate2::{Compression, write::DeflateEncoder};
use std::io::Write;

const OLD: &str = "719baa18-69ed-439a-8962-1de53d98e05e";
const NEW: &str = "d66867e4-febb-4c6e-9ef1-230dac3e25fa";
const BODY: &str = "b27aebc8-f190-4658-a81d-fd1406905f39";

fn deflate(text: &str) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(text.as_bytes()).unwrap();
    encoder.finish().unwrap()
}

fn row(name: &str, data: Vec<u8>) -> MainStorageRow {
    MainStorageRow {
        file_name: name.to_owned(),
        part_no: 0,
        creation: "2026-08-29T00:00:00Z".to_owned(),
        modified: "2026-08-29T00:00:00Z".to_owned(),
        attributes: 0,
        data_size: data.len() as u64,
        binary_data: data,
    }
}

fn versions(generation: &str) -> Vec<u8> {
    deflate(&format!("{{1,4,\"\",{generation},\"root\",x}}"))
}

fn fixture(mode: MainActivationMode) -> MainActivationPlan {
    let staged = vec![
        row(BODY, b"new descriptor".to_vec()),
        row(&format!("{BODY}.0"), b"new body".to_vec()),
        row("root", b"new root".to_vec()),
        row("version", b"new version".to_vec()),
        row("versions", versions(NEW)),
    ];
    let active = vec![
        row(BODY, b"old descriptor".to_vec()),
        row(&format!("{BODY}.0"), b"old body".to_vec()),
        row("root", b"old root".to_vec()),
        row("version", b"old version".to_vec()),
        row("versions", versions(OLD)),
    ];
    prepare_main_activation(
        mode,
        staged,
        MainActivationSnapshot {
            config_rows: active,
            config_dynamically_updated: None,
            params_dynamically_updated: None,
        },
        &[BODY.to_owned(), format!("{BODY}.0")],
        true,
    )
    .unwrap()
}

#[test]
fn source_initial_dependency_preimages_precede_all_script_publications() {
    let original = crate::mssql_config_apply::dynamic::SourceOwnerPreimages::test_fixture("lab");
    for mode in [
        MainActivationMode::Online,
        MainActivationMode::Live,
        MainActivationMode::Worker,
    ] {
        let plan = fixture(mode).with_precondition_sql(original.precondition_sql("lab").unwrap());
        let tail = (mode == MainActivationMode::Live).then_some(r"C:\tail.trn");
        let sql = render_main_activation_sql("lab", &plan, tail).unwrap().sql;
        let begin = sql.find("BEGIN TRANSACTION;").unwrap();
        let initial = sql.find("-- initial source dependency preimages").unwrap();
        let first_write = sql.find("DELETE FROM dbo.Config WHERE").unwrap();
        assert!(begin < initial && initial < first_write, "{mode:?}");
        assert!(sql[initial..first_write].contains("UPDLOCK, HOLDLOCK"));
        assert!(sql[initial..first_write].contains("HASHBYTES('SHA2_256', BinaryData)"));
    }
}
