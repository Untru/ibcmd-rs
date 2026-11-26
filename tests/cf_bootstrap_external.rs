//! `cf bootstrap --base-free` of an external data processor or report from
//! its XML tree alone (issue #438): a fixture package exported, built back
//! from the tree with no base, and exported again gives the same tree --
//! also with every identity and the name changed, a form path into the
//! object, and a name the seed's own children have.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "ibcmd-cf-bootstrap-external-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn ibcmd(args: &[&Path]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
    for arg in args {
        command.arg(arg);
    }
    command.output().unwrap()
}

fn export(file: &Path, tree: &Path) {
    let out = ibcmd(&[
        Path::new("cf"),
        Path::new("export"),
        file,
        tree,
        Path::new("--overwrite"),
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn files(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                out.push((relative, fs::read(&path).unwrap()));
            }
        }
    }
    out.sort();
    out
}

fn bootstrap(tree: &Path, output: &Path) -> std::process::Output {
    ibcmd(&[
        Path::new("cf"),
        Path::new("bootstrap"),
        Path::new("--base-free"),
        Path::new("--platform"),
        Path::new("8.3.27"),
        tree,
        output,
    ])
}

fn build(tree: &Path, output: &Path) {
    let built = bootstrap(tree, output);
    assert!(
        built.status.success(),
        "{}{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr)
    );
}

fn round_trip(fixture: &str, suffix: &str, tag: &str) {
    let scratch = Scratch::new(tag);
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture);
    let tree = scratch.0.join("tree");
    export(&fixture, &tree);
    let output = scratch.0.join(format!("built.{suffix}"));
    build(&tree, &output);
    let back = scratch.0.join("back");
    export(&output, &back);
    assert_eq!(files(&tree), files(&back));
}

#[test]
fn a_processor_builds_from_its_tree_alone() {
    round_trip(
        "tests/fixtures/external/test_processor/input.epf",
        "epf",
        "processor",
    );
}

#[test]
fn a_report_builds_from_its_tree_alone() {
    round_trip(
        "tests/fixtures/external/test_report/input.erf",
        "erf",
        "report",
    );
}

/// The tree's own identities, in order of first appearance: object uuids
/// and the ids of its contained and generated types. Platform class ids
/// (`<xr:ClassId>`) stay.
fn uuids(tree: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for (_, bytes) in files(tree) {
        let text = String::from_utf8_lossy(&bytes);
        for opener in ["uuid=\"", "<xr:ObjectId>", "<xr:TypeId>", "<xr:ValueId>"] {
            for piece in text.split(opener).skip(1) {
                let id = piece.get(..36).unwrap_or_default().to_ascii_lowercase();
                if id.len() == 36 && !out.contains(&id) {
                    out.push(id);
                }
            }
        }
    }
    out
}

/// `tree` exported from `fixture` with every uuid replaced by a fresh one and
/// the object renamed from `from` to `to`, in the files and their paths.
fn renamed_tree(fixture: &str, scratch: &Path, from: &str, to: &str) -> PathBuf {
    let exported = scratch.join("exported");
    export(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture),
        &exported,
    );
    let fresh = uuids(&exported)
        .into_iter()
        .enumerate()
        .map(|(index, id)| {
            (
                id,
                format!("5eed{index:04x}-0000-4000-8000-00000000{index:04x}"),
            )
        })
        .collect::<Vec<_>>();
    let tree = scratch.join("tree");
    for (relative, bytes) in files(&exported) {
        let mut text = String::from_utf8(bytes).unwrap();
        for (old, new) in &fresh {
            text = text.replace(old.as_str(), new);
        }
        // Paths and the root's own name; the tabular section attribute and
        // the template keep theirs.
        for (old, new) in [
            (format!("<Name>{from}</Name>"), format!("<Name>{to}</Name>")),
            (format!(".{from}<"), format!(".{to}<")),
            (format!(".{from}."), format!(".{to}.")),
            (format!(".{from}\""), format!(".{to}\"")),
        ] {
            text = text.replacen(&old, &new, usize::MAX);
        }
        let path = tree.join(relative.replacen(from, to, 1));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    tree
}

/// The root's attribute `Строка` on the form, so its data path steps
/// through the object's own type.
fn add_object_field(tree: &Path, name: &str) {
    let form = tree.join(format!("{name}/Forms/Форма/Ext/Form.xml"));
    let text = fs::read_to_string(&form).unwrap();
    let field = "\t<ChildItems>\r\n\t\t<InputField name=\"Строка\" id=\"2\">\r\n\
                 \t\t\t<DataPath>Объект.Строка</DataPath>\r\n\
                 \t\t\t<ContextMenu name=\"СтрокаКонтекстноеМеню\" id=\"3\"/>\r\n\
                 \t\t\t<ExtendedTooltip name=\"СтрокаРасширеннаяПодсказка\" id=\"4\"/>\r\n\
                 \t\t</InputField>\r\n\t</ChildItems>\r\n\t<Attributes>";
    assert!(text.contains("\t<Attributes>"));
    fs::write(&form, text.replacen("\t<Attributes>", field, 1)).unwrap();
}

/// The text of the `copyinfo` entry of `file`.
fn copyinfo(file: &Path, scratch: &Path) -> String {
    let dir = scratch.join("copyinfo");
    let out = ibcmd(&[
        Path::new("cf"),
        Path::new("extract"),
        file,
        Path::new("copyinfo"),
        &dir,
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(fs::read(dir.join("unpacked.bin")).unwrap()).unwrap()
}

/// The text between `<tag>` and `</tag>` after `after` in `xml`.
fn value_after<'a>(xml: &'a str, after: &str, tag: &str) -> &'a str {
    let from = xml.find(after).unwrap();
    let start = from + xml[from..].find(&format!("<{tag}>")).unwrap() + tag.len() + 2;
    let end = start + xml[start..].find(&format!("</{tag}>")).unwrap();
    &xml[start..end]
}

fn renamed_processor_builds(name: &str, tag: &str) {
    let scratch = Scratch::new(tag);
    let tree = renamed_tree(
        "tests/fixtures/external/test_processor/input.epf",
        &scratch.0,
        "ТестОбработка",
        name,
    );
    add_object_field(&tree, name);
    let output = scratch.0.join("built.epf");
    build(&tree, &output);
    let back = scratch.0.join("back");
    export(&output, &back);
    assert_eq!(files(&tree), files(&back));

    let root = fs::read_to_string(tree.join(format!("{name}.xml"))).unwrap();
    let attribute = root
        .split("<Attribute uuid=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned();
    let type_id = value_after(&root, "category=\"Object\"", "xr:TypeId");
    assert!(attribute.starts_with("5eed") && type_id.starts_with("5eed"));
    let copyinfo = copyinfo(&output, &scratch.0);
    assert!(
        copyinfo.contains(&format!("{{0,{attribute}}},{type_id},")),
        "{copyinfo}"
    );
}

#[test]
fn a_processor_with_new_identities_keeps_its_form_paths() {
    renamed_processor_builds("НоваяОбработка", "fresh");
}

#[test]
fn a_processor_may_take_the_name_of_a_seed_child() {
    renamed_processor_builds("Число", "child-name");
}

#[test]
fn a_report_with_new_identities_builds() {
    let scratch = Scratch::new("fresh-report");
    let tree = renamed_tree(
        "tests/fixtures/external/test_report/input.erf",
        &scratch.0,
        "ТестОтчет",
        "НовыйОтчет",
    );
    let output = scratch.0.join("built.erf");
    build(&tree, &output);
    let back = scratch.0.join("back");
    export(&output, &back);
    assert_eq!(files(&tree), files(&back));
    assert!(back.join("НовыйОтчет.xml").is_file());
}

#[test]
fn a_reference_to_the_configuration_is_refused_without_output() {
    let scratch = Scratch::new("refused");
    let tree = renamed_tree(
        "tests/fixtures/external/test_processor/input.epf",
        &scratch.0,
        "ТестОбработка",
        "СоСсылкой",
    );
    let root = tree.join("СоСсылкой.xml");
    let text = fs::read_to_string(&root).unwrap();
    fs::write(
        &root,
        text.replacen(
            "<v8:Type>xs:string</v8:Type>",
            "<v8:Type>cfg:CatalogRef.Валюты</v8:Type>",
            1,
        ),
    )
    .unwrap();
    let output = scratch.0.join("built.epf");
    let refused = bootstrap(&tree, &output);
    let report = String::from_utf8_lossy(&refused.stdout).into_owned()
        + &String::from_utf8_lossy(&refused.stderr);
    assert!(!refused.status.success(), "{report}");
    assert!(report.contains("cfg:CatalogRef.Валюты"), "{report}");
    assert!(!output.exists());
}

/// A form path through a platform object (a standard period) is refused
/// after the load, and the refused build leaves no file.
#[test]
fn an_unmeasured_form_step_is_refused_without_output() {
    let scratch = Scratch::new("platform-step");
    let tree = renamed_tree(
        "tests/fixtures/external/test_processor/input.epf",
        &scratch.0,
        "ТестОбработка",
        "СПериодом",
    );
    let form = tree.join("СПериодом/Forms/Форма/Ext/Form.xml");
    let text = fs::read_to_string(&form).unwrap();
    let text = text
        .replacen(
            "\t<Attributes>",
            "\t<ChildItems>\r\n\t\t<InputField name=\"Начало\" id=\"2\">\r\n\
             \t\t\t<DataPath>Период.StartDate</DataPath>\r\n\
             \t\t\t<ContextMenu name=\"НачалоКонтекстноеМеню\" id=\"3\"/>\r\n\
             \t\t\t<ExtendedTooltip name=\"НачалоРасширеннаяПодсказка\" id=\"4\"/>\r\n\
             \t\t</InputField>\r\n\t</ChildItems>\r\n\t<Attributes>",
            1,
        )
        .replacen(
            "\t</Attributes>",
            "\t\t<Attribute name=\"Период\" id=\"5\">\r\n\t\t\t<Type>\r\n\
             \t\t\t\t<v8:Type>v8:StandardPeriod</v8:Type>\r\n\t\t\t</Type>\r\n\
             \t\t</Attribute>\r\n\t</Attributes>",
            1,
        );
    fs::write(&form, text).unwrap();
    let output = scratch.0.join("built.epf");
    let refused = bootstrap(&tree, &output);
    let report = String::from_utf8_lossy(&refused.stdout).into_owned()
        + &String::from_utf8_lossy(&refused.stderr);
    assert!(!refused.status.success(), "{report}");
    assert!(report.contains("StartDate"), "{report}");
    assert!(!output.exists());
}
