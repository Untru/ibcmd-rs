// EDT 2026.1.3 keeps the root ParentConfigurations.bin as Configuration.distr;
// its XML export writes the designer file again in the .distr item order.
use morph1c_core::version::{FormatVersion, with_roundtrip_target};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use std::path::Path;

const DISTR: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
    "<distributionSupport:DistributionSupport xmlns:distributionSupport=\"http://g5.1c.ru/v8/dt/distribution/model\" version=\"6\" fileState=\"Distributive\" updateAvailable=\"true\">\r\n",
    "  <parentConfigurationInfos id=\"0c936e66-8c83-43b3-bd07-a9cf3260d84d\" storeMode=\"Self\" configRelease=\"2.0.9.8\" providerName=\"Фирма &quot;1С&quot;\" configName=\"Пример\" configVersion=\"8a0205a6-ec03-469c-a93c-74578d3c2838\">\r\n",
    "    <rules existingIdenticalObjectsWithFreeModeKeepMode=\"true\" existingIdenticalObjectsWithWarningModeKeepMode=\"true\" existingDifferentObjectsWithFreeModeKeepMode=\"true\" existingDifferentObjectsWithFreeMode=\"ChangesAllowed\" existingDifferentObjectsWithWarningModeKeepMode=\"true\" existingDifferentObjectsWithWarningMode=\"ChangesAllowed\"/>\r\n",
    "    <supportLanguages/>\r\n",
    "    <items userId=\"d95fcb37-fc65-466d-93e2-489f41683276\" parentId=\"d95fcb37-fc65-466d-93e2-489f41683276\" parentMode=\"ProtectMode\" used=\"true\"/>\r\n",
    "    <items userId=\"0744e8be-747c-47bb-85f3-58b6f6f9b786\" parentId=\"0744e8be-747c-47bb-85f3-58b6f6f9b786\" userMode=\"ChangesAllowed\" used=\"true\"/>\r\n",
    "  </parentConfigurationInfos>\r\n",
    "</distributionSupport:DistributionSupport>\r\n",
);

const BIN: &str = "\u{feff}{6,1,1,0c936e66-8c83-43b3-bd07-a9cf3260d84d,1,8a0205a6-ec03-469c-a93c-74578d3c2838,\"2.0.9.8\",\"Фирма \"\"1С\"\"\",\"Пример\",2,\
0,2,d95fcb37-fc65-466d-93e2-489f41683276,d95fcb37-fc65-466d-93e2-489f41683276,\
1,0,0744e8be-747c-47bb-85f3-58b6f6f9b786,0744e8be-747c-47bb-85f3-58b6f6f9b786,\
0,0,0,1,0,0,0,1,0,1,0,1,1,1,1}";

fn options() -> ConvertOptions {
    ConvertOptions::default().with_target_version(FormatVersion::new(2, 21))
}

fn write(format: Format, c: &morph1c_core::ir::Configuration, path: &Path) {
    with_roundtrip_target(FormatVersion::new(2, 21), || write_config(format, c, path)).unwrap();
}

fn seed() -> morph1c_core::ir::Configuration {
    read_config(
        Format::Designer,
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &ConvertOptions::default().with_target_version(FormatVersion::new(2, 21)),
    )
    .unwrap()
    .0
}

fn root_dir(edt: &Path) -> std::path::PathBuf {
    let dir = edt.join("Configuration");
    assert!(dir.join("Configuration.mdo").is_file());
    dir
}

#[test]
fn an_edt_distr_file_reads_as_the_designer_supplier_list_and_writes_back() {
    let edt = tempfile::tempdir().unwrap();
    write(Format::Edt, &seed(), edt.path());
    std::fs::write(root_dir(edt.path()).join("Configuration.distr"), DISTR).unwrap();

    let c = read_config(Format::Edt, edt.path(), &options()).unwrap().0;
    let designer = tempfile::tempdir().unwrap();
    write(Format::Designer, &c, designer.path());
    assert_eq!(
        std::fs::read(designer.path().join("Ext/ParentConfigurations.bin")).unwrap(),
        BIN.as_bytes()
    );

    let again = tempfile::tempdir().unwrap();
    write(Format::Edt, &c, again.path());
    let dir = root_dir(again.path());
    assert_eq!(
        std::fs::read(dir.join("Configuration.distr")).unwrap(),
        DISTR.as_bytes()
    );
    assert!(!dir.join("ParentConfigurations.bin").exists());
}

#[test]
fn a_designer_supplier_list_stays_a_bin_file_in_edt() {
    let designer = tempfile::tempdir().unwrap();
    write(Format::Designer, &seed(), designer.path());
    std::fs::create_dir_all(designer.path().join("Ext")).unwrap();
    std::fs::write(designer.path().join("Ext/ParentConfigurations.bin"), BIN).unwrap();
    let c = read_config(Format::Designer, designer.path(), &options())
        .unwrap()
        .0;
    let edt = tempfile::tempdir().unwrap();
    write(Format::Edt, &c, edt.path());
    let dir = root_dir(edt.path());
    assert_eq!(
        std::fs::read(dir.join("ParentConfigurations.bin")).unwrap(),
        BIN.as_bytes()
    );
    assert!(!dir.join("Configuration.distr").exists());
}

#[test]
fn an_edt_distr_file_beside_a_bin_file_is_refused() {
    let edt = tempfile::tempdir().unwrap();
    write(Format::Edt, &seed(), edt.path());
    let dir = root_dir(edt.path());
    std::fs::write(dir.join("Configuration.distr"), DISTR).unwrap();
    std::fs::write(dir.join("ParentConfigurations.bin"), BIN).unwrap();
    assert!(read_config(Format::Edt, edt.path(), &options()).is_err());
    std::fs::remove_file(dir.join("ParentConfigurations.bin")).unwrap();
    let unknown = DISTR.replace("\"ProtectMode\"", "\"Locked\"");
    std::fs::write(dir.join("Configuration.distr"), unknown).unwrap();
    assert!(read_config(Format::Edt, edt.path(), &options()).is_err());
}

// Lab check over real EDT 2026.1.3 imports: IBCMD_EDT_DISTR_PAIRS lists
// `<Configuration.distr>|<native ParentConfigurations.bin>` pairs, `;`-separated.
// The designer file must hold the native atoms (EDT reorders the items) and an
// EDT write must give the .distr bytes back.
#[test]
#[ignore = "needs IBCMD_EDT_DISTR_PAIRS"]
fn real_distr_files_match_their_native_supplier_lists() {
    let pairs = std::env::var("IBCMD_EDT_DISTR_PAIRS").unwrap();
    for pair in pairs.split(';') {
        let (distr, native) = pair.split_once('|').unwrap();
        let distr = std::fs::read(distr).unwrap();
        let native = std::fs::read(native).unwrap();
        let edt = tempfile::tempdir().unwrap();
        write(Format::Edt, &seed(), edt.path());
        std::fs::write(root_dir(edt.path()).join("Configuration.distr"), &distr).unwrap();
        let c = read_config(Format::Edt, edt.path(), &options()).unwrap().0;
        let designer = tempfile::tempdir().unwrap();
        write(Format::Designer, &c, designer.path());
        let bin = std::fs::read(designer.path().join("Ext/ParentConfigurations.bin")).unwrap();
        let atoms = |bytes: &[u8]| {
            let mut atoms: Vec<Vec<u8>> = bytes.split(|b| *b == b',').map(<[u8]>::to_vec).collect();
            atoms.sort();
            atoms
        };
        assert_eq!(bin.len(), native.len(), "{pair}");
        assert_eq!(atoms(&bin), atoms(&native), "{pair}");
        let again = tempfile::tempdir().unwrap();
        write(Format::Edt, &c, again.path());
        assert_eq!(
            std::fs::read(root_dir(again.path()).join("Configuration.distr")).unwrap(),
            distr,
            "{pair}"
        );
    }
}

#[test]
fn invalid_distribution_identifiers_are_refused_before_conversion() {
    let edt = tempfile::tempdir().unwrap();
    write(Format::Edt, &seed(), edt.path());
    for (key, valid) in [
        ("id", "0c936e66-8c83-43b3-bd07-a9cf3260d84d"),
        ("configVersion", "8a0205a6-ec03-469c-a93c-74578d3c2838"),
        ("userId", "d95fcb37-fc65-466d-93e2-489f41683276"),
        ("parentId", "d95fcb37-fc65-466d-93e2-489f41683276"),
    ] {
        for invalid in ["", "not-a-uuid", "d95fcb37-fc65-466d-93e2-489f41683276,0"] {
            let text = DISTR.replacen(
                &format!("{key}=\"{valid}\""),
                &format!("{key}=\"{invalid}\""),
                1,
            );
            assert_ne!(text, DISTR);
            std::fs::write(root_dir(edt.path()).join("Configuration.distr"), text).unwrap();
            assert!(
                read_config(Format::Edt, edt.path(), &options()).is_err(),
                "accepted {key}={invalid:?}"
            );
        }
    }
}
