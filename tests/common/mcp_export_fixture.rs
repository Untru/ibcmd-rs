//! Disposable public fixtures shared by portable MCP payload and Unix SDK tests.
#![allow(dead_code)]
#[path = "selected_export_fixture.rs"]
mod selected;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[derive(Clone, Copy, Debug)]
pub enum Scenario {
    InvalidDescriptor,
    OpaqueForm,
    NestedInvalid,
    LegacyNestedInvalid,
    LegacyNestedValid,
    LegacyCommand,
    CommonForm,
}
pub struct Case {
    pub root: PathBuf,
    pub database: PathBuf,
    pub rows: PathBuf,
    pub source: PathBuf,
    pub settings: PathBuf,
    pub object: String,
    pub rows_dir: bool,
    /// Removes `root` with the case, also when the test fails.
    _scratch: Owned,
}
/// The folder of a case, removed with it unless `IBCMD_TEST_KEEP_FIXTURES=1`
/// keeps it for debugging. This file is also a module of the root library's
/// unit tests, so it has no `common` to borrow a guard from.
struct Owned(PathBuf);
impl Drop for Owned {
    fn drop(&mut self) {
        if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_none_or(|value| value != "1") {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
pub fn tree(path: &Path) -> BTreeMap<String, Vec<u8>> {
    walkdir::WalkDir::new(path)
        .into_iter()
        .map(Result::unwrap)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            (
                e.path()
                    .strip_prefix(path)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}
fn unpack(path: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(path).unwrap())
        .read_to_end(&mut out)
        .unwrap();
    out
}
fn set_form_body(database: &Path) {
    let mut reader = ibcmd_filedb::FileDb::open(database).unwrap();
    let mut rows = reader.config_rows("CONFIG").unwrap();
    drop(reader);
    let row = rows
        .iter_mut()
        .find(|r| r.file_name == "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaa1.0")
        .unwrap();
    let mut packed = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    packed
        .write_all(b"{4,{0},\"Procedure KeepMe()\r\nEndProcedure\r\n\",{0}}")
        .unwrap();
    row.binary = packed.finish().unwrap();
    row.data_size = row.binary.len() as u64;
    ibcmd_filedb::replace_config_table(database, "CONFIG", &rows).unwrap();
}
impl Case {
    pub fn new(scenario: Scenario, rows_dir: bool) -> Self {
        use ibcmd_rs::metadata_model::brace::Brace;
        let root =
            std::env::temp_dir().join(format!("mcp-export-completeness-{}", uuid::Uuid::new_v4()));
        let scratch = Owned(root.clone());
        fs::create_dir_all(root.join("db")).unwrap();
        let database = root.join("db/1Cv8.1CD");
        fs::write(&database, unpack(&fixture("file-infobase/tiny.1CD.gz"))).unwrap();
        let object = match scenario {
            Scenario::InvalidDescriptor => {
                selected::install_unadmitted_common_module_descriptor(&database);
                "CommonModule.ФайловаяБазаПроверка".into()
            }
            Scenario::OpaqueForm | Scenario::CommonForm => {
                selected::install_unadmitted_form_asset(&database, true);
                if matches!(scenario, Scenario::CommonForm) {
                    set_form_body(&database);
                }
                "CommonForm.OpaqueForm".into()
            }
            Scenario::LegacyCommand => {
                selected::install_legacy_fallback_object(&database, true, None)
            }
            Scenario::NestedInvalid
            | Scenario::LegacyNestedInvalid
            | Scenario::LegacyNestedValid => {
                let recorded = root.join("recorded.1CD");
                fs::write(
                    &recorded,
                    unpack(&fixture("native-export-commands/owned/database.1CD.gz")),
                )
                .unwrap();
                let mut reader = ibcmd_filedb::FileDb::open(&recorded).unwrap();
                let rows = reader.config_rows("CONFIG").unwrap();
                drop(reader);
                ibcmd_filedb::replace_config_table(&database, "CONFIG", &rows).unwrap();
                selected::mutate_descriptor(
                    &database,
                    "c099c30b-8a08-45b0-be7d-9594366e78a5",
                    |row| {
                        let wrapper = row.as_list_mut().unwrap()[1].as_list_mut().unwrap();
                        let form = wrapper[1].as_list_mut().unwrap();
                        assert_eq!(form[0].as_atom(), Some("13"));
                        assert_eq!(form[2].as_atom(), Some("0"));
                        form[2] = Brace::num(99);
                    },
                );
                "Catalog.CorpusList.Form.ListForm".into()
            }
        };
        let rows = root.join("rows");
        fs::create_dir(&rows).unwrap();
        let mut reader = ibcmd_filedb::FileDb::open(&database).unwrap();
        for row in reader.config_rows("CONFIG").unwrap() {
            fs::write(
                rows.join(format!("{}__part{}.bin", row.file_name, row.part_no)),
                row.binary,
            )
            .unwrap();
        }
        drop(reader);
        let source = root.join("source");
        fs::create_dir(&source).unwrap();
        for (name, bytes) in tree(&fixture("file-infobase/tiny-xml")) {
            let p = source.join(name);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, bytes).unwrap();
        }
        for path in [
            "CommonForms/OpaqueForm.xml",
            "CommonForms/OpaqueForm/Ext/Form.xml",
            "Catalogs/CorpusList/Forms/ListForm.xml",
        ] {
            let p = source.join(path);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, b"local selected sentinel").unwrap();
        }
        fs::write(source.join("sentinel.txt"), b"unrelated local sentinel").unwrap();
        let settings = root.join("mcp.toml");
        let backend = if rows_dir {
            "rows-dir='rows'"
        } else {
            "db-path='db'"
        };
        fs::write(&settings,format!("[[database]]\nname='demo'\nplatform='8.5.1'\n{backend}\nagent-write=false\n[mcp]\ndatabase='demo'\nsource-root='source'\n")).unwrap();
        let case = Self {
            root,
            database,
            rows,
            source,
            settings,
            object,
            rows_dir,
            _scratch: scratch,
        };
        if matches!(
            scenario,
            Scenario::LegacyNestedInvalid | Scenario::LegacyNestedValid
        ) {
            case.legacy_form(if matches!(scenario, Scenario::LegacyNestedValid) {
                Brace::flag(false)
            } else {
                Brace::num(99)
            });
        }
        case
    }
    pub fn input(&self) -> BTreeMap<String, Vec<u8>> {
        tree(&self.root)
    }
}

impl Case {
    pub fn nested_value(&self, value: ibcmd_rs::metadata_model::brace::Brace) {
        self.nested_record(value, false);
    }
    /// Exact inverse of the documented five-member Form12 -> Form13 retag.
    pub fn legacy_form(&self, value: ibcmd_rs::metadata_model::brace::Brace) {
        self.nested_record(value, true);
    }
    fn nested_record(&self, value: ibcmd_rs::metadata_model::brace::Brace, old: bool) {
        selected::mutate_descriptor(
            &self.database,
            "c099c30b-8a08-45b0-be7d-9594366e78a5",
            |row| {
                let fields = row.as_list_mut().unwrap()[1].as_list_mut().unwrap()[1]
                    .as_list_mut()
                    .unwrap();
                if old {
                    assert_eq!(fields.len(), 5);
                    assert_eq!(fields[0].as_atom(), Some("13"));
                    fields[0] = ibcmd_rs::metadata_model::brace::Brace::num(12);
                }
                fields[2] = value;
            },
        );
        let mut db = ibcmd_filedb::FileDb::open(&self.database).unwrap();
        let mut rows = db.config_rows("CONFIG").unwrap();
        drop(db);
        // The platform changes the descriptor's version when it writes the row.
        let version = rows.iter_mut().find(|r| r.file_name == "versions").unwrap();
        let mut plain = Vec::new();
        flate2::read::DeflateDecoder::new(version.binary.as_slice())
            .read_to_end(&mut plain)
            .unwrap();
        use ibcmd_rs::metadata_model::brace::{Brace, parse_row, serialize_row};
        let mut versions = parse_row(&plain).unwrap();
        let fields = versions.as_list_mut().unwrap();
        let at = fields
            .iter()
            .position(|node| node.as_str() == Some("c099c30b-8a08-45b0-be7d-9594366e78a5"))
            .unwrap();
        fields[at + 1] = Brace::uuid(&uuid::Uuid::new_v4().to_string());
        let plain = serialize_row(&versions);
        let mut packed =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        packed.write_all(&plain).unwrap();
        version.binary = packed.finish().unwrap();
        version.data_size = version.binary.len() as u64;
        ibcmd_filedb::replace_config_table(&self.database, "CONFIG", &rows).unwrap();
        for row in rows {
            fs::write(
                self.rows
                    .join(format!("{}__part{}.bin", row.file_name, row.part_no)),
                row.binary,
            )
            .unwrap();
        }
    }
}
