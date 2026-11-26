//! The source context answers through [`MetadataSource`] exactly as through
//! its own methods: the same values, the same memoised state, and the same
//! error values with the same messages.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;

use super::{HelpReference, MetadataSource, MetadataSourceContext, SourceBytes};

const CONFIGURATION: &str = "11111111-1111-4111-8111-111111111111";
const STYLE_ITEM: &str = "22222222-2222-4222-8222-222222222222";
const PICTURE: &str = "33333333-3333-4333-8333-333333333333";
const GROUP: &str = "44444444-4444-4444-8444-444444444444";
const CATALOG: &str = "55555555-5555-4555-8555-555555555555";
const CATALOG_REF: &str = "66666666-6666-4666-8666-666666666666";
const ATTRIBUTE: &str = "77777777-7777-4777-8777-777777777777";

/// A source tree under the temporary directory, removed on drop.
struct Tree(PathBuf);

impl Tree {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-metadata-source-seam-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let tree = Self(root);
        tree.write(
            "Configuration.xml",
            &format!("<Configuration uuid=\"{CONFIGURATION}\"><Properties><Name>Конфигурация</Name></Properties></Configuration>"),
        );
        tree.write(
            "StyleItems/Акцент.xml",
            &format!("<StyleItem uuid=\"{STYLE_ITEM}\"><Properties><Name>Акцент</Name></Properties></StyleItem>"),
        );
        // A file in the style items' folder that declares something else.
        tree.write(
            "StyleItems/Чужой.xml",
            &format!("<CommonPicture uuid=\"{PICTURE}\"><Properties><Name>Чужой</Name></Properties></CommonPicture>"),
        );
        tree.write(
            "CommonPictures/Логотип.xml",
            &format!("<CommonPicture uuid=\"{PICTURE}\"><Properties><Name>Логотип</Name></Properties></CommonPicture>"),
        );
        tree.write(
            "CommandGroups/Печать.xml",
            &format!("<CommandGroup uuid=\"{GROUP}\"><Properties><Name>Печать</Name></Properties></CommandGroup>"),
        );
        tree.write(
            "Catalogs/Товары.xml",
            &format!("<Catalog uuid=\"{CATALOG}\"><Properties><Name>Товары</Name><GeneratedTypes><GeneratedType name=\"CatalogRef.Товары\"><TypeId>{CATALOG_REF}</TypeId></GeneratedType></GeneratedTypes></Properties><ChildObjects><Attribute uuid=\"{ATTRIBUTE}\"><Properties><Name>Цена</Name></Properties></Attribute></ChildObjects></Catalog>"),
        );
        tree
    }

    fn write(&self, relative: &str, body: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" version=\"2.20\">{body}</MetaDataObject>\n"
            ),
        )
        .unwrap();
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The messages of an error's chain, outermost first, and the kind of the
/// I/O error at its root, if one is.
fn anatomy(error: &anyhow::Error) -> (Vec<String>, Option<ErrorKind>) {
    (
        error.chain().map(ToString::to_string).collect(),
        error
            .root_cause()
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind),
    )
}

/// Asserts that the trait and the inherent method answered alike, and
/// returns the answer.
fn same(through_trait: Result<String>, inherent: Result<String>) -> Result<String> {
    match (&through_trait, &inherent) {
        (Ok(left), Ok(right)) => assert_eq!(left, right),
        (Err(left), Err(right)) => assert_eq!(anatomy(left), anatomy(right)),
        _ => panic!("the trait answered {through_trait:?} where the context answered {inherent:?}"),
    }
    through_trait
}

#[test]
fn the_context_answers_through_the_trait_as_through_its_own_methods() {
    let tree = Tree::new();
    let context = MetadataSourceContext::new(tree.0.clone());
    let source: &dyn MetadataSource = &context;

    // The root and the memo are the context's own fields.
    assert!(std::ptr::eq(
        source.source_root(),
        context.source_root.as_path()
    ));
    assert_eq!(source.source_root(), tree.0.as_path());
    assert!(std::ptr::eq(
        source.interface_memo(),
        &*context.interface_memo
    ));

    // A file: the same bytes, read for the caller; a missing one, the same
    // I/O error.
    let configuration = tree.path("Configuration.xml");
    let (Ok(SourceBytes::Owned(left)), Ok(SourceBytes::Owned(right))) = (
        source.read_source(&configuration),
        MetadataSourceContext::read_source(&context, &configuration),
    ) else {
        panic!("the configuration file reads as owned bytes through both");
    };
    assert_eq!(left, right);
    assert_eq!(left, fs::read(&configuration).unwrap());
    let missing = tree.path("Нет.xml");
    let (Err(left), Err(right)) = (
        source.read_source(&missing),
        MetadataSourceContext::read_source(&context, &missing),
    ) else {
        panic!("a missing file reads through neither");
    };
    assert_eq!(left.kind(), ErrorKind::NotFound);
    assert_eq!(
        (left.kind(), left.to_string()),
        (right.kind(), right.to_string())
    );

    // Memoised answers: the same values, and the same shared objects.
    assert_eq!(source.configuration_uuid().as_deref(), Some(CONFIGURATION));
    assert_eq!(
        source.configuration_uuid(),
        MetadataSourceContext::configuration_uuid(&context)
    );
    let object = source.configuration_object("Catalog.Товары").unwrap();
    assert_eq!(
        (object.class.as_str(), object.uuid.as_str()),
        ("Catalog", CATALOG)
    );
    assert!(Arc::ptr_eq(
        &object,
        &MetadataSourceContext::configuration_object(&context, "Catalog.Товары").unwrap()
    ));
    assert!(source.configuration_object("Catalog.Нет").is_none());
    assert!(MetadataSourceContext::configuration_object(&context, "Catalog.Нет").is_none());
    assert!(std::ptr::eq(
        source.dcs_style_items(),
        MetadataSourceContext::dcs_style_items(&context)
    ));
    assert_eq!(
        source.dcs_style_items().get(STYLE_ITEM).map(String::as_str),
        Some("Акцент")
    );
    assert_eq!(
        source.dcs_generated_type_id("CatalogRef.Товары").as_deref(),
        Some(CATALOG_REF)
    );
    assert_eq!(
        source.dcs_generated_type_id("CatalogRef.Товары"),
        MetadataSourceContext::dcs_generated_type_id(&context, "CatalogRef.Товары")
    );
    assert_eq!(source.dcs_generated_type_id("DefinedType.Деньги"), None);
    let refs = source.moxel_object_refs().unwrap();
    assert_eq!(
        refs,
        MetadataSourceContext::moxel_object_refs(&context).unwrap()
    );
    assert_eq!(
        refs.get(PICTURE).map(String::as_str),
        Some("CommonPicture.Логотип")
    );
    assert_eq!(
        refs.get(STYLE_ITEM).map(String::as_str),
        Some("StyleItem.Акцент")
    );

    // The resolvers: the same value or the same error, message by message.
    let style = |reference: &str| {
        same(
            source.resolve_style_item_uuid(reference),
            MetadataSourceContext::resolve_style_item_uuid(&context, reference),
        )
    };
    assert_eq!(style("StyleItem.Акцент").unwrap(), STYLE_ITEM);
    assert_eq!(
        anatomy(&style("Color.Акцент").unwrap_err()),
        (
            vec!["unsupported StyleItem reference: Color.Акцент".to_string()],
            None
        )
    );
    let (messages, kind) = anatomy(&style("StyleItem.Нет").unwrap_err());
    assert_eq!(
        messages[0],
        format!(
            "failed to read StyleItem XML {}",
            tree.path("StyleItems").join("Нет.xml").display()
        )
    );
    assert_eq!((messages.len(), kind), (2, Some(ErrorKind::NotFound)));
    assert_eq!(
        anatomy(&style("StyleItem.Чужой").unwrap_err()),
        (
            vec![format!(
                "expected StyleItem XML at {}, got CommonPicture",
                tree.path("StyleItems").join("Чужой.xml").display()
            )],
            None
        )
    );

    let picture = |reference: &str| {
        same(
            source.resolve_common_picture_uuid(reference),
            MetadataSourceContext::resolve_common_picture_uuid(&context, reference),
        )
    };
    assert_eq!(picture("CommonPicture.Логотип").unwrap(), PICTURE);
    assert_eq!(
        anatomy(&picture("Picture.Логотип").unwrap_err()).0,
        ["unsupported CommonCommand Picture reference: Picture.Логотип"]
    );
    assert_eq!(
        anatomy(&picture("CommonPicture.Нет").unwrap_err()).1,
        Some(ErrorKind::NotFound)
    );

    let group = |reference: &str| {
        same(
            source.resolve_command_group_uuid(reference),
            MetadataSourceContext::resolve_command_group_uuid(&context, reference),
        )
    };
    assert_eq!(group("CommandGroup.Печать").unwrap(), GROUP);
    assert_eq!(
        anatomy(&group("Group.Печать").unwrap_err()).0,
        ["unsupported CommandGroup reference: Group.Печать"]
    );
    assert_eq!(
        anatomy(&group("CommandGroup.Нет").unwrap_err()).0[0],
        format!(
            "failed to read CommandGroup XML {}",
            tree.path("CommandGroups").join("Нет.xml").display()
        )
    );

    let object = |reference: &str| {
        same(
            source.resolve_metadata_reference_uuid(reference),
            MetadataSourceContext::resolve_metadata_reference_uuid(&context, reference),
        )
    };
    assert_eq!(object("Catalog.Товары").unwrap(), CATALOG);
    assert_eq!(
        anatomy(&object("Нечто.Товары").unwrap_err()).0,
        ["unsupported metadata reference for source resolution: Нечто.Товары"]
    );
    assert_eq!(
        anatomy(&object("Catalog.Нет").unwrap_err()).0[0],
        format!(
            "failed to read Catalog XML {}",
            tree.path("Catalogs").join("Нет.xml").display()
        )
    );

    let type_id = |reference: &str| {
        same(
            source.resolve_metadata_type_id(reference),
            MetadataSourceContext::resolve_metadata_type_id(&context, reference),
        )
    };
    assert_eq!(type_id("cfg:CatalogRef.Товары").unwrap(), CATALOG_REF);
    assert_eq!(
        anatomy(&type_id("cfg:НечтоRef.Товары").unwrap_err()).0,
        ["unsupported metadata type reference for source resolution: cfg:НечтоRef.Товары"]
    );
    assert_eq!(
        anatomy(&type_id("cfg:CatalogRef.Нет").unwrap_err()).0[0],
        format!(
            "failed to read metadata XML {}",
            tree.path("Catalogs").join("Нет.xml").display()
        )
    );
}

/// A help reference memoises its answer, an error as its message; through
/// the trait, the first answer and the memoised one are the context's.
#[test]
fn a_help_reference_answers_and_memoises_through_the_trait_as_the_context_does() {
    let tree = Tree::new();
    let through_trait = MetadataSourceContext::new(tree.0.clone());
    let inherent = MetadataSourceContext::new(tree.0.clone());
    let source: &dyn MetadataSource = &through_trait;
    for _ in 0..2 {
        let (Ok(HelpReference::Resolved(left)), Ok(HelpReference::Resolved(right))) = (
            source.help_reference("Catalog.Товары"),
            inherent.help_reference("Catalog.Товары"),
        ) else {
            panic!("the catalog resolves through both");
        };
        assert_eq!((left.as_str(), right.as_str()), (CATALOG, CATALOG));
        assert!(matches!(
            (
                source.help_reference("Catalog.Нет"),
                inherent.help_reference("Catalog.Нет")
            ),
            (Ok(HelpReference::Absent), Ok(HelpReference::Absent))
        ));
        let (Err(left), Err(right)) = (
            source.help_reference("StyleItem.Чужой"),
            inherent.help_reference("StyleItem.Чужой"),
        ) else {
            panic!("a file that declares another class refuses through both");
        };
        assert_eq!(anatomy(&left), anatomy(&right));
        assert_eq!(
            anatomy(&left),
            (
                vec![format!(
                    "{} declares CommonPicture.Чужой",
                    tree.path("StyleItems").join("Чужой.xml").display()
                )],
                None
            )
        );
    }
}

const PROCESSOR: &str = "88888888-8888-4888-8888-888888888888";
const PROCESSOR_COMMAND: &str = "99999999-9999-4999-8999-999999999999";
const ITEM_FORM: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const COMMON_FORM: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const ENUM: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const ENUM_REF: &str = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
const ENUM_VALUE: &str = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
const PREDEFINED: &str = "12121212-1212-4212-8212-121212121212";
const NESTED_PREDEFINED: &str = "13131313-1313-4313-8313-131313131313";
const REGISTER: &str = "14141414-1414-4414-8414-141414141414";
const ROLE: &str = "15151515-1515-4515-8515-151515151515";

/// The tree of [`Tree::new`] with what the six members the form writer
/// reads beyond the packers' seam look up: a command, two forms, an enum
/// value, predefined items, an information register's dimensions and a
/// role.
fn form_writer_tree() -> Tree {
    let tree = Tree::new();
    tree.write(
        "DataProcessors/Загрузка.xml",
        &format!("<DataProcessor uuid=\"{PROCESSOR}\"><Properties><Name>Загрузка</Name></Properties><ChildObjects><Command uuid=\"{PROCESSOR_COMMAND}\"><Properties><Name>Загрузить</Name></Properties></Command></ChildObjects></DataProcessor>"),
    );
    tree.write(
        "Catalogs/Товары/Forms/ФормаЭлемента.xml",
        &format!(
            "<Form uuid=\"{ITEM_FORM}\"><Properties><Name>ФормаЭлемента</Name></Properties></Form>"
        ),
    );
    tree.write(
        "CommonForms/Поиск.xml",
        &format!("<CommonForm uuid=\"{COMMON_FORM}\"><Properties><Name>Поиск</Name></Properties></CommonForm>"),
    );
    tree.write(
        "Enums/Способы.xml",
        &format!("<Enum uuid=\"{ENUM}\"><Properties><Name>Способы</Name><GeneratedTypes><GeneratedType name=\"EnumRef.Способы\"><TypeId>{ENUM_REF}</TypeId></GeneratedType></GeneratedTypes></Properties><ChildObjects><EnumValue uuid=\"{ENUM_VALUE}\"><Properties><Name>Ручной</Name></Properties></EnumValue></ChildObjects></Enum>"),
    );
    tree.write(
        "Catalogs/Товары/Ext/Predefined.xml",
        &format!("<PredefinedData><Item id=\"{PREDEFINED}\"><Name>Основной</Name><Code/><Description/><IsFolder>true</IsFolder><ChildItems><Item id=\"{NESTED_PREDEFINED}\"><Name>Вложенный</Name><Code/><Description/><IsFolder>false</IsFolder></Item></ChildItems></Item></PredefinedData>"),
    );
    tree.write(
        "InformationRegisters/Курсы.xml",
        &format!("<InformationRegister uuid=\"{REGISTER}\"><Properties><Name>Курсы</Name></Properties><ChildObjects><Dimension><Properties><Name>Валюта</Name><Master>true</Master></Properties></Dimension><Dimension><Properties><Name>Период</Name><Master>false</Master></Properties></Dimension><Dimension><Properties><Name>Банк</Name><Master>true</Master></Properties></Dimension></ChildObjects></InformationRegister>"),
    );
    tree.write(
        "Roles/Администратор.xml",
        &format!(
            "<Role uuid=\"{ROLE}\"><Properties><Name>Администратор</Name></Properties></Role>"
        ),
    );
    tree
}

/// [`same`] for answers of any type.
fn alike<T: PartialEq + std::fmt::Debug>(
    through_trait: Result<T>,
    inherent: Result<T>,
) -> Result<T> {
    match (&through_trait, &inherent) {
        (Ok(left), Ok(right)) => assert_eq!(left, right),
        (Err(left), Err(right)) => assert_eq!(anatomy(left), anatomy(right)),
        _ => panic!("the trait answered {through_trait:?} where the context answered {inherent:?}"),
    }
    through_trait
}

/// The six members the native form writer reads beyond the packers' seam
/// answer through the trait as the context's own methods do: the same
/// values, and the same refusals message by message.
#[test]
fn the_form_writers_members_answer_through_the_trait_as_the_context_does() {
    let tree = form_writer_tree();
    let context = MetadataSourceContext::new(tree.0.clone());
    let source: &dyn MetadataSource = &context;

    let command = |reference: &str| {
        same(
            source.resolve_command_reference_uuid(reference),
            MetadataSourceContext::resolve_command_reference_uuid(&context, reference),
        )
    };
    assert_eq!(
        command("DataProcessor.Загрузка.Command.Загрузить").unwrap(),
        PROCESSOR_COMMAND
    );
    // A reference that names no command is an object's.
    assert_eq!(command("Catalog.Товары").unwrap(), CATALOG);
    assert_eq!(
        anatomy(&command("Нечто.Загрузка.Command.Загрузить").unwrap_err()).0,
        [
            "unsupported command owner reference for source resolution: Нечто.Загрузка.Command.Загрузить"
        ]
    );
    let (messages, kind) = anatomy(&command("DataProcessor.Нет.Command.Загрузить").unwrap_err());
    assert_eq!(
        messages[0],
        format!(
            "failed to read command owner XML {}",
            tree.path("DataProcessors").join("Нет.xml").display()
        )
    );
    assert_eq!((messages.len(), kind), (2, Some(ErrorKind::NotFound)));
    assert_eq!(
        anatomy(&command("DataProcessor.Загрузка.Command.Нет").unwrap_err()).0[0],
        "failed to resolve command DataProcessor.Загрузка.Command.Нет"
    );

    let form = |reference: &str| {
        same(
            source.resolve_form_uuid(reference),
            MetadataSourceContext::resolve_form_uuid(&context, reference),
        )
    };
    assert_eq!(
        form("Catalog.Товары.Form.ФормаЭлемента").unwrap(),
        ITEM_FORM
    );
    assert_eq!(form("CommonForm.Поиск").unwrap(), COMMON_FORM);
    // A form the configuration no longer has is its bare uuid, lowercase.
    assert_eq!(form(&ITEM_FORM.to_uppercase()).unwrap(), ITEM_FORM);
    assert_eq!(
        anatomy(&form("Товары").unwrap_err()).0,
        ["unsupported form reference: Товары"]
    );
    let (messages, kind) = anatomy(&form("Catalog.Товары.Form.Нет").unwrap_err());
    assert_eq!(
        messages[0],
        format!(
            "failed to read form XML {}",
            tree.path("Catalogs")
                .join("Товары")
                .join("Forms")
                .join("Нет.xml")
                .display()
        )
    );
    assert_eq!((messages.len(), kind), (2, Some(ErrorKind::NotFound)));

    let child = |owner: &str, kind: &str, name: &str| {
        same(
            source.resolve_metadata_child_uuid(owner, kind, name),
            MetadataSourceContext::resolve_metadata_child_uuid(&context, owner, kind, name),
        )
    };
    assert_eq!(
        child("Enum.Способы", "EnumValue", "Ручной").unwrap(),
        ENUM_VALUE
    );
    assert_eq!(
        child("Catalog.Товары", "Attribute", "Цена").unwrap(),
        ATTRIBUTE
    );
    assert_eq!(
        anatomy(&child("Нечто.Способы", "EnumValue", "Ручной").unwrap_err()).0,
        ["unsupported child owner reference for source resolution: Нечто.Способы"]
    );
    assert_eq!(
        anatomy(&child("Enum.Способы", "EnumValue", "Нет").unwrap_err()).0[0],
        "failed to resolve child Enum.Способы.EnumValue.Нет"
    );

    let predefined = |owner: &str, name: &str| {
        same(
            source.resolve_predefined_item_id(owner, name),
            MetadataSourceContext::resolve_predefined_item_id(&context, owner, name),
        )
    };
    assert_eq!(
        predefined("Catalog.Товары", "Основной").unwrap(),
        PREDEFINED
    );
    // The name is looked up over the whole nested tree.
    assert_eq!(
        predefined("Catalog.Товары", "Вложенный").unwrap(),
        NESTED_PREDEFINED
    );
    assert_eq!(
        anatomy(&predefined("Catalog.Товары", "Нет").unwrap_err()).0,
        ["Catalog.Товары declares no predefined item named Нет"]
    );
    let (messages, kind) = anatomy(&predefined("Catalog.Цены", "Основной").unwrap_err());
    assert_eq!(
        messages[0],
        format!(
            "failed to read Predefined XML {}",
            tree.path("Catalogs")
                .join("Цены")
                .join("Ext")
                .join("Predefined.xml")
                .display()
        )
    );
    assert_eq!((messages.len(), kind), (2, Some(ErrorKind::NotFound)));

    let dimensions = |register: &str| {
        alike(
            source.information_register_master_dimensions(register),
            MetadataSourceContext::information_register_master_dimensions(&context, register),
        )
    };
    assert_eq!(dimensions("Курсы").unwrap(), ["Валюта", "Банк"]);
    let (messages, kind) = anatomy(&dimensions("Нет").unwrap_err());
    assert_eq!(
        messages[0],
        format!(
            "failed to read InformationRegister XML {}",
            tree.path("InformationRegisters").join("Нет.xml").display()
        )
    );
    assert_eq!((messages.len(), kind), (2, Some(ErrorKind::NotFound)));

    let simple = |reference: &str, kind: &str, folder: &str, prefix: &str| {
        same(
            source.resolve_simple_metadata_uuid(reference, kind, folder, prefix),
            MetadataSourceContext::resolve_simple_metadata_uuid(
                &context, reference, kind, folder, prefix,
            ),
        )
    };
    assert_eq!(
        simple("Role.Администратор", "Role", "Roles", "Role.").unwrap(),
        ROLE
    );
    assert_eq!(
        anatomy(&simple("Администратор", "Role", "Roles", "Role.").unwrap_err()).0,
        ["unsupported Role reference: Администратор"]
    );
    assert_eq!(
        anatomy(&simple("StyleItem.Чужой", "StyleItem", "StyleItems", "StyleItem.").unwrap_err()),
        (
            vec![format!(
                "expected StyleItem XML at {}, got CommonPicture",
                tree.path("StyleItems").join("Чужой.xml").display()
            )],
            None
        )
    );
}

/// A source that answers from a table what the form writer asks of it, and
/// refuses everything else as a read the writer does not make.
#[derive(Debug)]
struct Answers(std::collections::BTreeMap<String, String>);

impl Answers {
    fn answer(&self, question: String) -> Result<String> {
        self.0
            .get(&question)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("the table has no answer to {question}"))
    }
}

impl MetadataSource for Answers {
    fn resolve_metadata_type_id(&self, reference: &str) -> Result<String> {
        self.answer(format!("type {reference}"))
    }
    fn resolve_metadata_reference_uuid(&self, reference: &str) -> Result<String> {
        self.answer(format!("object {reference}"))
    }
    fn resolve_command_reference_uuid(&self, reference: &str) -> Result<String> {
        self.answer(format!("command {reference}"))
    }
    fn resolve_metadata_child_uuid(&self, owner: &str, kind: &str, name: &str) -> Result<String> {
        self.answer(format!("child {owner} {kind} {name}"))
    }
    fn resolve_predefined_item_id(&self, owner: &str, name: &str) -> Result<String> {
        self.answer(format!("predefined {owner} {name}"))
    }
    fn information_register_master_dimensions(&self, register: &str) -> Result<Vec<String>> {
        Ok(self
            .answer(format!("dimensions {register}"))?
            .split(',')
            .map(str::to_owned)
            .collect())
    }
    fn resolve_simple_metadata_uuid(
        &self,
        reference: &str,
        kind: &str,
        folder: &str,
        prefix: &str,
    ) -> Result<String> {
        self.answer(format!("simple {reference} {kind} {folder} {prefix}"))
    }
    fn configuration_object(
        &self,
        key: &str,
    ) -> Option<Arc<ibcmd_form_native::ConfigurationObject>> {
        unreachable!("the writer's functions under test read no data path ({key})")
    }
    fn source_root(&self) -> &std::path::Path {
        unreachable!("the writer reads no file")
    }
    fn read_source(&self, _: &std::path::Path) -> std::io::Result<SourceBytes> {
        unreachable!("the writer reads no file")
    }
    fn configuration_uuid(&self) -> Option<String> {
        unreachable!("the writer's functions under test read no configuration uuid")
    }
    fn resolve_form_uuid(&self, reference: &str) -> Result<String> {
        self.answer(format!("form {reference}"))
    }
    fn resolve_style_item_uuid(&self, _: &str) -> Result<String> {
        unreachable!("the writer's functions under test name no style item")
    }
    fn resolve_common_picture_uuid(&self, _: &str) -> Result<String> {
        unreachable!("the writer's functions under test name no picture")
    }
    fn resolve_command_group_uuid(&self, _: &str) -> Result<String> {
        unreachable!("the writer's functions under test name no command group")
    }
    fn dcs_generated_type_id(&self, _: &str) -> Option<String> {
        unreachable!("the writer's functions under test have no data-composition schema")
    }
    fn dcs_style_items(&self) -> &std::collections::BTreeMap<String, String> {
        unreachable!("the writer's functions under test have no data-composition schema")
    }
    fn moxel_object_refs(&self) -> Result<std::collections::BTreeMap<String, String>> {
        unreachable!("the writer's functions under test have no chart")
    }
    fn help_reference(&self, _: &str) -> Result<HelpReference> {
        unreachable!("the writer's functions under test have no help page")
    }
    fn interface_memo(&self) -> &std::sync::Mutex<super::InterfaceResolutionMemo> {
        unreachable!("the writer's functions under test have no command interface")
    }
}

/// The form writer's functions that read the six members give, from a
/// source that only answers a table, what they give from the context over
/// a tree on disk that declares the same: the writer reads them through
/// the trait and nowhere else. The data-path walk's question goes through
/// the trait too, to the context's own memoised object.
#[test]
fn the_form_writer_answers_a_table_source_as_the_source_context() {
    let tree = form_writer_tree();
    let context = MetadataSourceContext::new(tree.0.clone());
    let table = Answers(
        [
            ("type EnumRef.Способы", ENUM_REF),
            ("type CatalogRef.Товары", CATALOG_REF),
            ("child Enum.Способы EnumValue Ручной", ENUM_VALUE),
            ("predefined Catalog.Товары Вложенный", NESTED_PREDEFINED),
            ("object InformationRegister.Курсы", REGISTER),
            ("dimensions Курсы", "Валюта,Банк"),
            ("object DataProcessor.Загрузка", PROCESSOR),
            (
                "command DataProcessor.Загрузка.Command.Загрузить",
                PROCESSOR_COMMAND,
            ),
            ("simple Role.Администратор Role Roles Role.", ROLE),
        ]
        .into_iter()
        .map(|(question, answer)| (question.to_owned(), answer.to_owned()))
        .collect(),
    );
    let through = |source: &dyn MetadataSource| {
        let choice = |reference: &str| super::native_choice_list_reference(reference, Some(source));
        let standard = |reference: &str| {
            super::native_object_standard_command(reference, Some(source)).unwrap()
        };
        let interface = |reference: &str| {
            super::native_command_interface_command(
                &super::FormXmlCommandInterfaceItem {
                    command: Some(reference.to_owned()),
                    ..Default::default()
                },
                &super::FormXmlBodyProperties::default(),
                "",
                Some(source),
            )
            .unwrap()
        };
        let rights = super::native_rights(
            &super::FormXmlRights {
                common: Some(false),
                values: vec![("Role.Администратор".to_owned(), Some(true))],
            },
            "<UserVisible>",
            Some(source),
        )
        .unwrap();
        (
            [
                choice("Enum.Способы.EnumValue.Ручной").unwrap(),
                choice("Catalog.Товары.Вложенный").unwrap(),
                choice("Catalog.Товары.EmptyRef").unwrap(),
            ],
            [
                standard("InformationRegister.Курсы.StandardCommand.OpenByValue.Банк"),
                standard("DataProcessor.Загрузка.StandardCommand.Open"),
            ],
            interface("DataProcessor.Загрузка.Command.Загрузить"),
            rights,
        )
    };
    let from_tree = through(&context);
    assert_eq!(from_tree, through(&table));
    assert_eq!(
        from_tree,
        (
            [
                (ENUM_REF.to_owned(), ENUM_VALUE.to_owned()),
                (CATALOG_REF.to_owned(), NESTED_PREDEFINED.to_owned()),
                (
                    CATALOG_REF.to_owned(),
                    "00000000-0000-0000-0000-000000000000".to_owned()
                ),
            ],
            [format!("{{4,{REGISTER}}}"), format!("{{0,{PROCESSOR}}}")],
            format!("{{0,{PROCESSOR_COMMAND}}}"),
            format!("{{0,{{0,{{\"B\",0}},1,{ROLE},{{\"B\",1}}}}}}"),
        )
    );

    // The data-path walk asks for an object through the trait: the very
    // object the context's own `ConfigurationObjects` answer memoises.
    use ibcmd_form_native::ConfigurationObjects;
    let walked = super::SourceConfigurationObjects(&context)
        .object("Catalog.Товары")
        .unwrap();
    assert!(Arc::ptr_eq(
        &walked,
        &ConfigurationObjects::object(&context, "Catalog.Товары").unwrap()
    ));
    assert!(
        super::SourceConfigurationObjects(&context)
            .object("Catalog.Нет")
            .is_none()
    );
}
