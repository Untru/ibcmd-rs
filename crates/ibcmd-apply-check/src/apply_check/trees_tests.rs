//! Tests of the check on two trees: synthetic XML, one change for every rule.

use super::*;

/// Two trees in the temp folder; removed with the value.
struct Trees {
    root: PathBuf,
}

impl Trees {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-apply-check-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(root.join("old")).unwrap();
        fs::create_dir_all(root.join("new")).unwrap();
        Self { root }
    }

    fn put(&self, side: &str, rel: &str, text: &str) {
        let path = self.root.join(side).join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// The same file on both sides.
    fn both(&self, rel: &str, text: &str) {
        self.put("old", rel, text);
        self.put("new", rel, text);
    }

    fn check(&self) -> Verdict {
        check_trees(&self.root.join("old"), &self.root.join("new")).unwrap()
    }
}

impl Drop for Trees {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

fn object(kind: &str, uuid: &str, name: &str, properties: &str, children: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<MetaDataObject version=\"2.20\">\r\n\
         <{kind} uuid=\"{uuid}\"><Properties><Name>{name}</Name>{properties}</Properties>\
         <ChildObjects>{children}</ChildObjects></{kind}></MetaDataObject>"
    )
}

const CATALOG: &str = "6bc2c3f8-6027-411e-97c5-6f1b7e4cea61";

fn catalog(properties: &str, children: &str) -> String {
    object("Catalog", CATALOG, "C", properties, children)
}

fn attribute(uuid: &str, name: &str, properties: &str) -> String {
    format!(
        "<Attribute uuid=\"{uuid}\"><Properties><Name>{name}</Name>{properties}</Properties></Attribute>"
    )
}

const A1: &str = "aaaaaaaa-0000-4000-8000-000000000001";
const A2: &str = "aaaaaaaa-0000-4000-8000-000000000002";

#[test]
fn two_equal_trees_need_no_restructuring() {
    let trees = Trees::new("equal");
    trees.both("Catalogs/C.xml", &catalog("<CodeLength>9</CodeLength>", ""));
    trees.both(
        "Catalogs/C/Ext/ObjectModule.bsl",
        "Процедура X()\nКонецПроцедуры\n",
    );
    trees.both(
        "Configuration.xml",
        &object("Configuration", A1, "Cfg", "", ""),
    );
    let verdict = trees.check();
    assert!(!verdict.needs_restructuring);
    assert!(verdict.reasons.is_empty() && verdict.notes.is_empty());
    assert_eq!(verdict.stats.staged_rows, 0);
    assert_eq!(verdict.stats.old_files, 3);
}

#[test]
fn code_forms_templates_help_rights_and_pictures_are_copied_rows() {
    let trees = Trees::new("copied");
    let base = catalog("", "");
    trees.both("Catalogs/C.xml", &base);
    trees.put("old", "Catalogs/C/Ext/ObjectModule.bsl", "// 1\n");
    trees.put("new", "Catalogs/C/Ext/ObjectModule.bsl", "// 2\n");
    trees.put("old", "Catalogs/C/Forms/F/Ext/Form.xml", "<Form a=\"1\"/>");
    trees.put("new", "Catalogs/C/Forms/F/Ext/Form.xml", "<Form a=\"2\"/>");
    trees.put(
        "new",
        "Catalogs/C/Forms/F/Ext/Form/Module.bsl",
        "// module\n",
    );
    trees.put("new", "Catalogs/C/Templates/T/Ext/Template.xml", "<t/>");
    trees.put("new", "Catalogs/C/Ext/Help/ru.html", "<p/>");
    trees.put("old", "Roles/R/Ext/Rights.xml", "<Rights a=\"1\"/>");
    trees.put("new", "Roles/R/Ext/Rights.xml", "<Rights a=\"2\"/>");
    trees.put("new", "CommonPictures/P/Ext/Picture/Picture.png", "PNG");
    let verdict = trees.check();
    assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
    let roles = &verdict.stats.body_rows_by_role;
    assert_eq!(roles["Module"], 1);
    assert_eq!(roles["Form"], 2);
    assert_eq!(roles["Template"], 1);
    assert_eq!(roles["Help"], 1);
    assert_eq!(roles["Rights"], 1);
    assert_eq!(roles["Picture"], 1);
}

#[test]
fn data_the_platform_keeps_is_not_a_copied_row() {
    let trees = Trees::new("data");
    trees.both("Catalogs/C.xml", &catalog("", ""));
    trees.put("old", "Catalogs/C/Ext/Predefined.xml", "<items/>");
    trees.put(
        "new",
        "Catalogs/C/Ext/Predefined.xml",
        "<items><Item/></items>",
    );
    trees.put("new", "ExchangePlans/E/Ext/Content.xml", "<Content/>");
    trees.put("new", "BusinessProcesses/B/Ext/Flowchart.xml", "<f/>");
    trees.put("new", "Catalogs/C/Ext/Mystery.dat", "?");
    let verdict = trees.check();
    assert!(verdict.needs_restructuring);
    let found = |object: &str, property: &str, class: ReasonClass| {
        verdict
            .reasons
            .iter()
            .any(|r| r.object == object && r.property == property && r.class == class)
    };
    assert!(found("Catalog.C", "Predefined", ReasonClass::Data));
    assert!(found("ExchangePlan.E", "Content", ReasonClass::Structure));
    assert!(found("BusinessProcess.B", "Flowchart", ReasonClass::Data));
    assert!(verdict.reasons.iter().any(
        |r| r.file_name == "Catalogs/C/Ext/Mystery.dat" && r.class == ReasonClass::Unknown
    ));
    assert_eq!(verdict.reasons.len(), 4);
}

#[test]
fn descriptors_are_compared_property_by_property() {
    let trees = Trees::new("properties");
    trees.put(
        "old",
        "Catalogs/C.xml",
        &catalog(
            "<CodeLength>9</CodeLength><QuickChoice>false</QuickChoice>",
            "",
        ),
    );
    trees.put(
        "new",
        "Catalogs/C.xml",
        &catalog(
            "<CodeLength>12</CodeLength><QuickChoice>true</QuickChoice>",
            "",
        ),
    );
    let verdict = trees.check();
    assert!(verdict.needs_restructuring);
    assert_eq!(verdict.reasons.len(), 1);
    assert_eq!(verdict.reasons[0].object, "Catalog.C");
    assert_eq!(verdict.reasons[0].property, "Properties/CodeLength");
    assert_eq!(verdict.notes.len(), 1);
    assert_eq!(verdict.notes[0].property, "Properties/QuickChoice");
    assert_eq!(verdict.stats.descriptors_compared, 1);
}

#[test]
fn attributes_added_dropped_and_reworded() {
    let trees = Trees::new("attributes");
    let one = attribute(
        A1,
        "Код",
        "<Indexing>DontIndex</Indexing><ToolTip>a</ToolTip>",
    );
    trees.put("old", "Catalogs/C.xml", &catalog("", &one));
    // Reworded: a tooltip. Added: a second attribute.
    let reworded = attribute(
        A1,
        "Код",
        "<Indexing>DontIndex</Indexing><ToolTip>b</ToolTip>",
    );
    let added = attribute(A2, "Имя", "");
    trees.put(
        "new",
        "Catalogs/C.xml",
        &catalog("", &format!("{reworded}{added}")),
    );
    let verdict = trees.check();
    assert_eq!(verdict.reasons.len(), 1, "{:?}", verdict.reasons);
    assert_eq!(verdict.reasons[0].property, "ChildObjects/Attribute[Имя]");
    assert_eq!(verdict.reasons[0].change.split(' ').next(), Some("added"));
    assert_eq!(verdict.notes.len(), 1);
    assert_eq!(
        verdict.notes[0].property,
        "ChildObjects/Attribute[Код]/Properties/ToolTip"
    );

    // An index flag is storage.
    let indexed = attribute(A1, "Код", "<Indexing>Index</Indexing><ToolTip>a</ToolTip>");
    trees.put("new", "Catalogs/C.xml", &catalog("", &indexed));
    let verdict = trees.check();
    assert!(verdict.needs_restructuring);
    assert_eq!(verdict.reasons.len(), 1);
    assert_eq!(
        verdict.reasons[0].property,
        "ChildObjects/Attribute[Код]/Properties/Indexing"
    );
}

#[test]
fn objects_added_and_removed_are_judged_by_their_kind() {
    let trees = Trees::new("lifecycle");
    trees.both("Catalogs/C.xml", &catalog("", ""));
    trees.put(
        "new",
        "CommonModules/M.xml",
        &object("CommonModule", A1, "M", "<Global>false</Global>", ""),
    );
    trees.put("new", "CommonModules/M/Ext/Module.bsl", "// m\n");
    let verdict = trees.check();
    assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
    assert_eq!(verdict.notes.len(), 1);
    assert_eq!(verdict.notes[0].object, "CommonModule.M");
    assert_eq!(verdict.stats.added_files, 2);

    trees.put(
        "new",
        "Documents/D.xml",
        &object("Document", A2, "D", "", ""),
    );
    let verdict = trees.check();
    assert!(verdict.needs_restructuring);
    assert_eq!(verdict.reasons.len(), 1);
    assert_eq!(verdict.reasons[0].object, "Document.D");

    // A catalog dropped.
    let trees = Trees::new("dropped");
    trees.put("old", "Catalogs/C.xml", &catalog("", ""));
    let verdict = trees.check();
    assert!(verdict.needs_restructuring);
    assert!(verdict.reasons[0].change.starts_with("removed"));
    assert_eq!(verdict.stats.removed_files, 1);
}

#[test]
fn a_moved_object_is_one_object() {
    let trees = Trees::new("moved");
    trees.put("old", "Catalogs/C.xml", &catalog("", ""));
    trees.put("new", "Catalogs/Renamed.xml", &catalog("", ""));
    let verdict = trees.check();
    assert!(verdict.reasons.is_empty(), "{:?}", verdict.reasons);
    // Moved and changed: the change is judged, not a drop and an add.
    trees.put(
        "new",
        "Catalogs/Renamed.xml",
        &catalog("<CodeLength>3</CodeLength>", ""),
    );
    let verdict = trees.check();
    assert_eq!(verdict.reasons.len(), 1);
    assert_eq!(verdict.reasons[0].property, "Properties/CodeLength");
}

#[test]
fn the_dump_info_is_derived_and_never_a_change() {
    let trees = Trees::new("dumpinfo");
    trees.both("Catalogs/C.xml", &catalog("", ""));
    trees.put("old", "ConfigDumpInfo.xml", "<a/>");
    trees.put("new", "ConfigDumpInfo.xml", "<b/>");
    assert!(trees.check().reasons.is_empty());
}

#[test]
fn a_directory_that_is_not_one_is_an_error() {
    assert!(check_trees(Path::new("no/such/dir"), Path::new("no/such/dir")).is_err());
}

#[test]
fn names_follow_the_folders() {
    assert_eq!(full_name("Configuration.xml"), "Configuration");
    assert_eq!(full_name("Catalogs/X.xml"), "Catalog.X");
    assert_eq!(full_name("Catalogs/X/Forms/F.xml"), "Catalog.X.Form.F");
    assert_eq!(
        full_name("Subsystems/A/Subsystems/B.xml"),
        "Subsystem.A.Subsystem.B"
    );
    assert_eq!(body_owner("Ext/SessionModule.bsl"), "Configuration");
    assert_eq!(body_owner("Catalogs/X/Ext/ObjectModule.bsl"), "Catalog.X");
    assert_eq!(
        body_owner("Catalogs/X/Forms/F/Ext/Form/Module.bsl"),
        "Catalog.X.Form.F"
    );
}

#[test]
fn the_content_of_an_exchange_plan_is_compared_as_a_set() {
    let content = |items: &[(&str, &str)]| {
        let body = items
            .iter()
            .map(|(metadata, record)| {
                format!(
                    "<Item><Metadata>{metadata}</Metadata><AutoRecord>{record}</AutoRecord></Item>"
                )
            })
            .collect::<String>();
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<ExchangePlanContent version=\"2.20\">{body}</ExchangePlanContent>"
        )
    };
    let trees = Trees::new("content");
    let rel = "ExchangePlans/E/Ext/Content.xml";
    trees.put(
        "old",
        rel,
        &content(&[("Catalog.A", "Allow"), ("Catalog.B", "Deny")]),
    );
    trees.put(
        "new",
        rel,
        &content(&[("Catalog.B", "Deny"), ("Catalog.A", "Allow")]),
    );
    let verdict = trees.check();
    assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
    assert_eq!(verdict.notes.len(), 1);
    assert_eq!(verdict.notes[0].property, "Content");

    // Another flag of automatic registration on the same objects: no table changes.
    trees.put(
        "new",
        rel,
        &content(&[("Catalog.B", "Deny"), ("Catalog.A", "Deny")]),
    );
    let verdict = trees.check();
    assert!(!verdict.needs_restructuring, "{:?}", verdict.reasons);
    assert!(verdict.notes[0].change.contains("flags"));

    for changed in [
        content(&[("Catalog.B", "Deny")]),
        content(&[("Catalog.B", "Deny"), ("Catalog.C", "Allow")]),
        "not a content file".to_string(),
    ] {
        trees.put("new", rel, &changed);
        let verdict = trees.check();
        assert!(verdict.needs_restructuring, "{changed}");
        assert_eq!(verdict.reasons[0].property, "Content");
    }
}
