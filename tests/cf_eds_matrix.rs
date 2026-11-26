//! End-to-end export of the checked-in matrix configuration: the ExternalData
//! children must land under `ExternalDataSources/<root>/...` and be byte-equal
//! to the native 8.3.27.2214 tree. The CF is attached so the claim is
//! reproducible without the source workspace.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Tree(PathBuf);

impl Tree {
    fn new() -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "ibcmd-eds-matrix-{}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

fn export(cf: &Path, output: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export", "--overwrite", "--platform", "8.3.27.2214"])
        .arg(cf)
        .arg(output)
        .env("PATH", "")
        .output()
        .unwrap()
}

#[test]
fn matrix_eds_children_are_written_under_the_root_and_match_the_platform() {
    let tree = Tree::new();
    let output_dir = tree.0.join("exported");
    let output = export(&fixture("eds-matrix/configuration.cf"), &output_dir);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true, "{report}");
    let entries = report["export"]["storage"]["entries"].as_array().unwrap();
    let expectations = [
        (
            "35f0f3eb-6bc6-455e-8200-2a8c1664122b",
            "ExternalDataSources/ВнешнийИсточникДанных1.xml",
            "root.xml",
        ),
        (
            "959bb0a7-39af-49f7-97cd-b9702bcf6cf0",
            "ExternalDataSources/ВнешнийИсточникДанных1/Cubes/Куб1.xml",
            "cube.xml",
        ),
        (
            "87f7ae12-3120-4257-92a1-11beacd7b790",
            "ExternalDataSources/ВнешнийИсточникДанных1/Tables/Таблица1.xml",
            "table.xml",
        ),
        (
            "79581bb0-b72d-43bb-b988-8f45c602139a",
            "ExternalDataSources/ВнешнийИсточникДанных1/Cubes/Куб1/DimensionTables/ТаблицаИзмерения1.xml",
            "dimension.xml",
        ),
    ];
    for (uuid, relative, expected) in expectations {
        let entry = entries
            .iter()
            .find(|entry| entry["logical_name"] == uuid)
            .unwrap_or_else(|| panic!("{uuid} is missing from the export report"));
        assert_eq!(entry["disposition"], "supported", "{entry}");
        assert_eq!(entry["outputs"], serde_json::json!([relative]), "{entry}");
        let actual = fs::read(output_dir.join(relative))
            .unwrap_or_else(|error| panic!("{relative}: {error}"));
        let expected = fs::read(fixture(&format!("eds-matrix/{expected}"))).unwrap();
        assert_eq!(actual, expected, "{relative} differs from the platform");
    }

    for wrong in [
        "IntegrationServices/Куб1.xml",
        "IntegrationServices/ТаблицаИзмерения1.xml",
    ] {
        assert!(
            !output_dir.join(wrong).exists(),
            "{wrong} must not be produced"
        );
    }
}

#[test]
fn matrix_metadata_roots_are_byte_exact() {
    let tree = Tree::new();
    let output_dir = tree.0.join("exported");
    let output = export(&fixture("eds-matrix/configuration.cf"), &output_dir);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for relative in [
        "Catalogs/Справочник1.xml",
        "Documents/Документ1.xml",
        "BusinessProcesses/БизнесПроцесс1.xml",
        "ChartsOfCharacteristicTypes/ПланВидовХарактеристик1.xml",
        "Tasks/Задача1.xml",
        "ChartsOfAccounts/ПланСчетов1.xml",
        "ExchangePlans/ПланОбмена1.xml",
        "HTTPServices/HTTPСервис1.xml",
        "CalculationRegisters/РегистрРасчета1.xml",
        "IntegrationServices/СервисИнтеграции1.xml",
        "Constants/КнстРеквизитОтрицательные.xml",
        "Constants/КнстРеквизитРасшрПравка.xml",
    ] {
        let actual = fs::read(output_dir.join(relative))
            .unwrap_or_else(|error| panic!("{relative}: {error}"));
        let expected = fs::read(fixture(&format!("eds-matrix/{relative}"))).unwrap();
        assert_eq!(actual, expected, "{relative} differs from the platform");
    }
}
