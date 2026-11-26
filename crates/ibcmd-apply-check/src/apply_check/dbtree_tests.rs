//! Tests of a tree against a database's descriptors: synthetic XML, the
//! export of a stored descriptor replaced by the descriptor's own text.

use std::path::PathBuf;

use super::*;
use crate::metadata_model::export::NameIndex;
use crate::metadata_model::objects::parts::Compat;

const CATALOG: &str = "6bc2c3f8-6027-411e-97c5-6f1b7e4cea61";
const MODULE: &str = "ab132638-5188-470d-9432-de85f2b2c7d8";
const NEW_CATALOG: &str = "11111111-1111-4111-8111-111111111111";
const NEW_MODULE: &str = "22222222-2222-4222-8222-222222222222";

/// The "stored row" is the file text an export would write.
struct Text;

impl Export for Text {
    fn export_row(&self, _kind: &str, row: &[u8]) -> Result<String> {
        Ok(String::from_utf8(row.to_vec())?)
    }
}

fn document(body: &str) -> String {
    format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<MetaDataObject version=\"2.20\">\r\n\
         {body}\r\n</MetaDataObject>"
    )
}

fn catalog(uuid: &str, code_length: &str, synonym: &str) -> String {
    document(&format!(
        "<Catalog uuid=\"{uuid}\"><Properties><Name>Кассы</Name>\
         <Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>{synonym}</v8:content></v8:item></Synonym>\
         <CodeLength>{code_length}</CodeLength></Properties><ChildObjects/></Catalog>"
    ))
}

fn module(uuid: &str, global: &str) -> String {
    document(&format!(
        "<CommonModule uuid=\"{uuid}\"><Properties><Name>Заметки</Name><Global>{global}</Global></Properties></CommonModule>"
    ))
}

fn plan() -> Plan {
    let mut plan = Plan {
        configuration: None,
        module_group: None,
        kinds: HashMap::from([
            (CATALOG.to_string(), "Catalog"),
            (MODULE.to_string(), "CommonModule"),
        ]),
        children: HashMap::new(),
        names: NameIndex::default(),
        version: "2.20".to_string(),
        compat: Compat(8, 3, 27),
        errors: Vec::new(),
    };
    plan.names.insert_name(CATALOG, "Catalog.Кассы");
    plan.names.insert_name(MODULE, "CommonModule.Заметки");
    plan
}

fn inputs() -> Inputs {
    let mut inputs = Inputs::default();
    inputs.old_descriptors.insert(
        CATALOG.to_string(),
        catalog(CATALOG, "9", "Кассы").into_bytes(),
    );
    inputs
        .old_descriptors
        .insert(MODULE.to_string(), module(MODULE, "false").into_bytes());
    inputs.old_inventory = ["root", "version", "versions", CATALOG, MODULE]
        .iter()
        .map(|name| name.to_string())
        .collect();
    inputs
}

fn object(rel: &str, kind: &str, uuid: &str) -> TreeObject {
    TreeObject {
        rel: rel.to_string(),
        kind: kind.to_string(),
        uuid: uuid.to_string(),
    }
}

/// A tree of files held in memory.
struct Files {
    files: HashMap<String, Vec<u8>>,
    scan: TreeScan,
}

impl Files {
    fn same_as_database() -> Self {
        let mut files = Self {
            files: HashMap::new(),
            scan: TreeScan::default(),
        };
        files.put(
            "Catalogs/Кассы.xml",
            "Catalog",
            CATALOG,
            &catalog(CATALOG, "9", "Кассы"),
        );
        files.put(
            "CommonModules/Заметки.xml",
            "CommonModule",
            MODULE,
            &module(MODULE, "false"),
        );
        files
    }

    fn put(&mut self, rel: &str, kind: &str, uuid: &str, text: &str) {
        self.scan.objects.retain(|object| object.rel != rel);
        self.scan.objects.push(object(rel, kind, uuid));
        self.scan.files = self.scan.objects.len();
        self.files.insert(rel.to_string(), text.as_bytes().to_vec());
    }

    fn remove(&mut self, rel: &str) {
        self.scan.objects.retain(|object| object.rel != rel);
        self.scan.files = self.scan.objects.len();
        self.files.remove(rel);
    }

    fn check(&self, partial: bool) -> Verdict {
        let plan = plan();
        let labels = Labels::new(&plan, &plan);
        let read = |rel: &str| -> Result<Vec<u8>> {
            self.files
                .get(rel)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no such file: {rel}"))
        };
        compare_planned(
            &inputs(),
            &plan,
            &labels,
            &Text,
            &self.scan,
            &read,
            partial,
            Verdict::new("tree-db"),
        )
    }
}

#[test]
fn a_tree_that_is_the_export_of_the_database_needs_no_restructuring() {
    let verdict = Files::same_as_database().check(false);
    assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
    assert!(verdict.reasons.is_empty() && verdict.notes.is_empty());
    assert!(verdict.objects.is_empty());
    assert_eq!(verdict.source, "tree-db");
    assert_eq!(verdict.stats.staged_rows, 0);
    assert!(verdict.is_conclusive());
}

#[test]
fn an_edit_of_presentation_is_a_note_and_an_object_without_a_class() {
    let mut tree = Files::same_as_database();
    tree.put(
        "Catalogs/Кассы.xml",
        "Catalog",
        CATALOG,
        &catalog(CATALOG, "9", "Кассы (2)"),
    );
    let verdict = tree.check(false);
    assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
    assert_eq!(verdict.notes.len(), 1);
    assert_eq!(verdict.notes[0].object, "Catalog.Кассы");
    assert_eq!(verdict.objects.len(), 1);
    let changed = &verdict.objects[0];
    assert_eq!(changed.op, ObjectOp::Changed);
    assert_eq!(changed.class, None);
    assert_eq!(changed.id, CATALOG);
    assert_eq!(changed.file_name, "Catalogs/Кассы.xml");
    assert_eq!(changed.kind, "Catalog");
    assert_eq!(verdict.stats.descriptors_compared, 1);
    assert_eq!(verdict.stats.staged_rows, 1);
}

#[test]
fn an_edit_of_storage_is_a_reason_with_the_property_path() {
    let mut tree = Files::same_as_database();
    tree.put(
        "Catalogs/Кассы.xml",
        "Catalog",
        CATALOG,
        &catalog(CATALOG, "12", "Кассы"),
    );
    let verdict = tree.check(false);
    assert!(verdict.needs_restructuring);
    assert_eq!(verdict.reasons.len(), 1);
    assert_eq!(verdict.reasons[0].class, ReasonClass::Structure);
    assert_eq!(verdict.reasons[0].object, "Catalog.Кассы");
    assert_eq!(verdict.reasons[0].property, "Properties/CodeLength");
    assert_eq!(verdict.objects[0].class, Some(ReasonClass::Structure));
}

#[test]
fn a_file_written_differently_but_the_same_is_no_change() {
    let mut tree = Files::same_as_database();
    let mut text = catalog(CATALOG, "9", "Кассы");
    text.push_str("\r\n\r\n");
    tree.put("Catalogs/Кассы.xml", "Catalog", CATALOG, &text);
    let verdict = tree.check(false);
    assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
    assert!(verdict.objects.is_empty());
    assert_eq!(verdict.stats.staged_rows, 0);
    assert_eq!(verdict.stats.descriptors_compared, 1);
}

#[test]
fn objects_the_tree_adds_are_judged_by_their_kind() {
    let mut tree = Files::same_as_database();
    tree.put(
        "Catalogs/Новый.xml",
        "Catalog",
        NEW_CATALOG,
        &catalog(NEW_CATALOG, "9", "Новый"),
    );
    tree.put(
        "CommonModules/Новый.xml",
        "CommonModule",
        NEW_MODULE,
        &module(NEW_MODULE, "false"),
    );
    let verdict = tree.check(false);
    assert_eq!(verdict.reasons.len(), 1, "{:?}", verdict.reasons);
    assert_eq!(verdict.reasons[0].object, "Catalog.Новый");
    assert!(verdict.reasons[0].change.starts_with("added"));
    assert_eq!(verdict.notes.len(), 1);
    assert_eq!(verdict.notes[0].object, "CommonModule.Новый");
    assert_eq!(verdict.stats.added_files, 2);
    let ops = verdict
        .objects
        .iter()
        .map(|object| (object.id.as_str(), object.op, object.class))
        .collect::<Vec<_>>();
    assert!(ops.contains(&(NEW_CATALOG, ObjectOp::Added, Some(ReasonClass::Structure))));
    assert!(ops.contains(&(NEW_MODULE, ObjectOp::Added, None)));
}

#[test]
fn objects_the_tree_lacks_are_removed_unless_the_tree_is_partial() {
    let mut tree = Files::same_as_database();
    tree.remove("CommonModules/Заметки.xml");
    let verdict = tree.check(false);
    assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
    assert_eq!(verdict.stats.removed_files, 1);
    assert_eq!(verdict.objects.len(), 1);
    assert_eq!(verdict.objects[0].op, ObjectOp::Removed);
    assert_eq!(verdict.objects[0].id, MODULE);
    assert_eq!(verdict.objects[0].object, "CommonModule.Заметки");

    tree.remove("Catalogs/Кассы.xml");
    let verdict = tree.check(false);
    assert!(verdict.needs_restructuring);
    assert_eq!(verdict.reasons.len(), 1);
    assert_eq!(verdict.reasons[0].object, "Catalog.Кассы");
    assert!(verdict.reasons[0].change.starts_with("removed"));

    // A partial tree removes nothing.
    let verdict = tree.check(true);
    assert!(!verdict.needs_restructuring);
    assert!(verdict.objects.is_empty());
    assert_eq!(verdict.stats.removed_files, 0);
}

#[test]
fn another_kind_under_the_same_uuid_is_a_reason() {
    let mut tree = Files::same_as_database();
    tree.put(
        "Catalogs/Кассы.xml",
        "Document",
        CATALOG,
        &document(&format!(
            "<Document uuid=\"{CATALOG}\"><Properties><Name>Кассы</Name></Properties></Document>"
        )),
    );
    let verdict = tree.check(false);
    assert!(verdict.needs_restructuring);
    assert_eq!(verdict.reasons.len(), 1);
    assert!(verdict.reasons[0].change.starts_with("the kind changed"));
    assert_eq!(verdict.objects[0].class, Some(ReasonClass::Structure));
}

#[test]
fn a_file_that_cannot_be_parsed_or_read_is_unknown() {
    let mut tree = Files::same_as_database();
    tree.put(
        "Catalogs/Кассы.xml",
        "Catalog",
        CATALOG,
        "<MetaDataObject><Catalog>",
    );
    let verdict = tree.check(false);
    assert!(verdict.needs_restructuring);
    assert_eq!(verdict.reasons[0].class, ReasonClass::Unknown);
    assert_eq!(verdict.stats.unreadable, 1);
    assert_eq!(verdict.objects[0].class, Some(ReasonClass::Unknown));

    // Listed by the scan but gone from the disk.
    let mut tree = Files::same_as_database();
    tree.files.remove("Catalogs/Кассы.xml");
    let verdict = tree.check(false);
    assert_eq!(verdict.reasons[0].class, ReasonClass::Unknown);
    assert!(verdict.reasons[0].change.contains("cannot be read"));
}

#[test]
fn an_object_in_two_files_is_unknown() {
    let mut tree = Files::same_as_database();
    tree.put(
        "Catalogs/Копия.xml",
        "Catalog",
        CATALOG,
        &catalog(CATALOG, "9", "Кассы"),
    );
    let verdict = tree.check(false);
    assert!(verdict.needs_restructuring);
    assert!(verdict.reasons[0].change.contains("is also in"));
}

#[test]
fn the_files_that_are_not_descriptors_are_counted_not_compared() {
    let mut tree = Files::same_as_database();
    tree.scan.safe_bodies.insert("Module".to_string(), 3);
    tree.scan.not_compared = vec!["Catalogs/Кассы/Ext/Predefined.xml".to_string()];
    let verdict = tree.check(false);
    assert!(!verdict.needs_restructuring);
    assert_eq!(verdict.stats.body_rows_by_role["Module"], 3);
    assert_eq!(verdict.stats.body_files_not_compared, 1);
    // "No restructuring" cannot be trusted for the file that was not compared.
    assert!(verdict.incomplete);
    assert!(!verdict.is_conclusive());
    assert!(
        verdict
            .render_text()
            .contains("файлы с данными не сравнивались")
    );
}

#[test]
fn the_head_of_a_metadata_file_names_the_object() {
    let text = catalog(CATALOG, "9", "Кассы");
    assert_eq!(
        read_head(text.as_bytes()),
        Head::Object {
            kind: "Catalog".to_string(),
            uuid: CATALOG.to_string()
        }
    );
    // Upper case uuids are compared as lower case.
    let upper = text.replace(CATALOG, &CATALOG.to_ascii_uppercase());
    assert_eq!(
        read_head(upper.as_bytes()),
        Head::Object {
            kind: "Catalog".to_string(),
            uuid: CATALOG.to_string()
        }
    );
    assert_eq!(read_head(b"<Form xmlns=\"x\"/>"), Head::NotMetadata);
    assert!(matches!(
        read_head(b"<MetaDataObject version=\"2.20\"><Catalog>"),
        Head::Unreadable(_)
    ));
    assert!(matches!(
        read_head(b"<MetaDataObject version=\"2.20\">"),
        Head::Unreadable(_)
    ));
}

/// A tree on disk, removed with the value.
struct Folder {
    root: PathBuf,
}

impl Folder {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-apply-check-dbtree-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn put(&self, rel: &str, text: &str) {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

#[test]
fn a_scan_tells_descriptors_from_the_other_files_by_role() {
    let folder = Folder::new("scan");
    folder.put(
        "Configuration.xml",
        &document("<Configuration uuid=\"c0c0c0c0-0000-4000-8000-000000000000\"/>"),
    );
    folder.put("Catalogs/Кассы.xml", &catalog(CATALOG, "9", "Кассы"));
    folder.put(
        "Catalogs/Кассы/Ext/ObjectModule.bsl",
        "Процедура X()\nКонецПроцедуры\n",
    );
    folder.put("Catalogs/Кассы/Ext/Predefined.xml", "<PredefinedData/>");
    folder.put("Catalogs/Кассы/Forms/Список/Ext/Form.xml", "<Form/>");
    folder.put("Roles/Р/Ext/Rights.xml", "<Rights/>");
    folder.put("Ext/Unknown.bin", "?");
    folder.put("ConfigDumpInfo.xml", "<ConfigDumpInfo/>");
    let scanned = scan(&folder.root, false).unwrap();
    assert_eq!(scanned.files, 7, "the dump info is not counted");
    let objects = scanned
        .objects
        .iter()
        .map(|object| (object.rel.as_str(), object.kind.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        objects,
        vec![
            ("Catalogs/Кассы.xml", "Catalog"),
            ("Configuration.xml", "Configuration")
        ]
    );
    assert_eq!(scanned.safe_bodies["Module"], 1);
    assert_eq!(scanned.safe_bodies["Rights"], 1);
    assert_eq!(scanned.safe_bodies["Form"], 1);
    assert_eq!(
        scanned.not_compared,
        vec![
            "Catalogs/Кассы/Ext/Predefined.xml".to_string(),
            "Ext/Unknown.bin".to_string()
        ]
    );
    assert!(scanned.unreadable.is_empty());
}

#[test]
fn a_folder_without_a_configuration_is_refused_unless_partial() {
    let folder = Folder::new("partial");
    folder.put("Catalogs/Кассы.xml", &catalog(CATALOG, "9", "Кассы"));
    let error = scan(&folder.root, false).unwrap_err().to_string();
    assert!(error.contains("Configuration.xml"), "{error}");
    let scanned = scan(&folder.root, true).unwrap();
    assert_eq!(scanned.objects.len(), 1);
    assert!(scan(&folder.root.join("nothing"), true).is_err());
}
