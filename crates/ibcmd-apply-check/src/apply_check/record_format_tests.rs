//! An object that a native apply restructured once is stored in the record
//! format of the platform that staged it, not the one the compatibility mode
//! stores: `Config` keeps the row exactly as the native import wrote it
//! (catalog tag 57 in a configuration kept at 8.3.24, which stores 56). The
//! second change to the same object is then compared against such a row, and
//! the check used to stop at it: "the Catalog row changed and cannot be
//! decoded: record version 57, the compatibility mode stores 56", class
//! `unknown`, and S1 refused the stage without a class of its own.
//!
//! The rows here are compiled by the model's own writer from a small
//! synthetic catalog, in the two compatibility modes; nothing is copied from
//! a corpus.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use super::check::{Labels, compare_descriptor};
use super::plan::{Decoder, Plan};
use super::s1::{self, RefusalCode};
use super::{ReasonClass, RuleId, Verdict};
use crate::metadata_model::export::NameIndex;
use crate::metadata_model::objects::parts::Compat;
use crate::metadata_model::{DescriptorContext, compile_descriptor};

const CATALOG: &str = "6bc2c3f8-6027-411e-97c5-6f1b7e4cea61";
const A1: &str = "aaaaaaaa-0000-4000-8000-000000000001";
const A2: &str = "aaaaaaaa-0000-4000-8000-000000000002";

const STANDARD: [&str; 9] = [
    "PredefinedDataName",
    "Predefined",
    "Ref",
    "DeletionMark",
    "IsFolder",
    "Owner",
    "Parent",
    "Description",
    "Code",
];

fn standard_attribute(name: &str) -> String {
    // The export writes these three differently from the rest, and the
    // 8.3.24 record cannot hold the difference: a row made from a uniform
    // template would read back changed.
    let (checking, filling, reduction) = match name {
        "Owner" => ("ShowError", "true", "Deny"),
        "Parent" => ("DontCheck", "true", "TransformValues"),
        "Description" => ("ShowError", "false", "TransformValues"),
        _ => ("DontCheck", "false", "TransformValues"),
    };
    format!(
        "<xr:StandardAttribute name=\"{name}\"><xr:LinkByType/><xr:FillChecking>{checking}</xr:FillChecking>         <xr:MultiLine>false</xr:MultiLine><xr:FillFromFillingValue>{filling}</xr:FillFromFillingValue>         <xr:CreateOnInput>Auto</xr:CreateOnInput><xr:TypeReductionMode>{reduction}</xr:TypeReductionMode>         <xr:MaxValue xsi:nil=\"true\"/><xr:ToolTip/><xr:ExtendedEdit>false</xr:ExtendedEdit><xr:Format/>         <xr:ChoiceForm/><xr:QuickChoice>Auto</xr:QuickChoice><xr:ChoiceHistoryOnInput>Auto</xr:ChoiceHistoryOnInput>         <xr:EditFormat/><xr:PasswordMode>false</xr:PasswordMode><xr:DataHistory>Use</xr:DataHistory>         <xr:MarkNegatives>false</xr:MarkNegatives><xr:MinValue xsi:nil=\"true\"/><xr:Synonym/><xr:Comment/>         <xr:FullTextSearch>Use</xr:FullTextSearch><xr:ChoiceParameterLinks/><xr:FillValue xsi:nil=\"true\"/>         <xr:Mask/><xr:ChoiceParameters/></xr:StandardAttribute>"
    )
}

/// A string attribute of a catalog, as the export writes it.
fn attribute(uuid: &str, name: &str, length: u32) -> String {
    format!(
        "<Attribute uuid=\"{uuid}\"><Properties><Name>{name}</Name><Synonym/><Comment/>\
         <Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>{length}</v8:Length>\
         <v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers></Type>\
         <PasswordMode>false</PasswordMode><Format/><EditFormat/><ToolTip/>\
         <MarkNegatives>false</MarkNegatives><Mask/><MultiLine>false</MultiLine>\
         <ExtendedEdit>false</ExtendedEdit><MinValue xsi:nil=\"true\"/><MaxValue xsi:nil=\"true\"/>\
         <FillFromFillingValue>false</FillFromFillingValue><FillValue xsi:nil=\"true\"/>\
         <FillChecking>DontCheck</FillChecking><ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems>\
         <ChoiceParameterLinks/><ChoiceParameters/><QuickChoice>Auto</QuickChoice>\
         <CreateOnInput>Auto</CreateOnInput><ChoiceForm/><LinkByType/>\
         <ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput><Use>ForItem</Use>\
         <Indexing>DontIndex</Indexing><FullTextSearch>Use</FullTextSearch>\
         <DataHistory>Use</DataHistory></Properties></Attribute>"
    )
}

/// The catalog `K` with the given attributes.
fn catalog_xml(code_length: u32, attributes: &str) -> String {
    let generated = [
        ("Object", "CatalogObject"),
        ("Ref", "CatalogRef"),
        ("Selection", "CatalogSelection"),
        ("List", "CatalogList"),
        ("Manager", "CatalogManager"),
    ]
    .iter()
    .enumerate()
    .map(|(index, (category, prefix))| {
        format!(
            "<xr:GeneratedType name=\"{prefix}.K\" category=\"{category}\">\
             <xr:TypeId>1000000{index}-0000-4000-8000-000000000000</xr:TypeId>\
             <xr:ValueId>2000000{index}-0000-4000-8000-000000000000</xr:ValueId></xr:GeneratedType>"
        )
    })
    .collect::<String>();
    let standard = STANDARD
        .iter()
        .map(|name| standard_attribute(name))
        .collect::<String>();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" \
         xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
         version=\"2.20\"><Catalog uuid=\"{CATALOG}\"><InternalInfo>{generated}</InternalInfo><Properties>\
         <Name>K</Name><Synonym/><Comment/><Hierarchical>false</Hierarchical>\
         <HierarchyType>HierarchyFoldersAndItems</HierarchyType><LimitLevelCount>false</LimitLevelCount>\
         <LevelCount>2</LevelCount><FoldersOnTop>true</FoldersOnTop><UseStandardCommands>true</UseStandardCommands>\
         <Owners/><SubordinationUse>ToItems</SubordinationUse><CodeLength>{code_length}</CodeLength>\
         <DescriptionLength>25</DescriptionLength><CodeType>String</CodeType>\
         <CodeAllowedLength>Variable</CodeAllowedLength><CodeSeries>WholeCatalog</CodeSeries>\
         <CheckUnique>true</CheckUnique><Autonumbering>true</Autonumbering>\
         <DefaultPresentation>AsDescription</DefaultPresentation><StandardAttributes>{standard}</StandardAttributes>\
         <Characteristics/><PredefinedDataUpdate>Auto</PredefinedDataUpdate><EditType>InDialog</EditType>\
         <QuickChoice>false</QuickChoice><ChoiceMode>BothWays</ChoiceMode><InputByString/>\
         <SearchStringModeOnInputByString>Begin</SearchStringModeOnInputByString>\
         <FullTextSearchOnInputByString>DontUse</FullTextSearchOnInputByString>\
         <ChoiceDataGetModeOnInputByString>Directly</ChoiceDataGetModeOnInputByString>\
         <DefaultObjectForm/><DefaultFolderForm/><DefaultListForm/><DefaultChoiceForm/>\
         <DefaultFolderChoiceForm/><AuxiliaryObjectForm/><AuxiliaryFolderForm/><AuxiliaryListForm/>\
         <AuxiliaryChoiceForm/><AuxiliaryFolderChoiceForm/><IncludeHelpInContents>false</IncludeHelpInContents>\
         <BasedOn/><DataLockFields/><DataLockControlMode>Managed</DataLockControlMode>\
         <FullTextSearch>Use</FullTextSearch><ObjectPresentation/><ExtendedObjectPresentation/>\
         <ListPresentation/><ExtendedListPresentation/><Explanation/><CreateOnInput>Use</CreateOnInput>\
         <ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput><DataHistory>DontUse</DataHistory>\
         <UpdateDataHistoryImmediatelyAfterWrite>false</UpdateDataHistoryImmediatelyAfterWrite>\
         <ExecuteAfterWriteDataHistoryVersionProcessing>false</ExecuteAfterWriteDataHistoryVersionProcessing>\
         </Properties><ChildObjects>{attributes}</ChildObjects></Catalog></MetaDataObject>"
    )
}

/// A scratch tree that is removed with the value.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        Self(std::env::temp_dir().join(format!(
            "ibcmd-record-format-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        )))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

/// The row the model's writer makes of `xml` in a configuration kept at
/// `compatibility` (`Version8_3_24` stores catalog tag 56, `Version8_3_27`
/// 57 -- the tag a native import writes), and the names of the tree.
fn compile(xml: &str, compatibility: &str) -> (Vec<u8>, NameIndex) {
    let scratch = Scratch::new(compatibility);
    let root = &scratch.0;
    fs::create_dir_all(root.join("Catalogs")).unwrap();
    fs::write(
        root.join("Configuration.xml"),
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" \
             version=\"2.20\"><Configuration uuid=\"11111111-1111-1111-1111-111111111111\"><Properties>\
             <Name>C</Name><CompatibilityMode>{compatibility}</CompatibilityMode></Properties>\
             </Configuration></MetaDataObject>"
        ),
    )
    .unwrap();
    let path = root.join("Catalogs").join("K.xml");
    fs::write(&path, xml).unwrap();
    let context = DescriptorContext::new(root, "2.20").unwrap();
    let row = compile_descriptor("Catalog", &path, xml.as_bytes(), &context).unwrap();
    (row, NameIndex::from_config_index(&context.index))
}

/// The plan of a configuration that holds the catalog.
fn plan(names: NameIndex) -> Plan {
    Plan {
        configuration: None,
        module_group: None,
        kinds: HashMap::from([(CATALOG.to_string(), "Catalog")]),
        children: HashMap::new(),
        names,
        version: "2.20".to_string(),
        compat: Compat(8, 3, 24),
        errors: Vec::new(),
    }
}

/// The check of one changed row: `old` is the active `Config` row, `staged`
/// the ConfigSave's.
fn compare(old: &[u8], staged: &[u8], names: NameIndex) -> Verdict {
    let plan = plan(names.clone());
    let labels = Labels::new(&plan, &plan);
    // What `check` builds: the active mode, then the newest the model knows.
    let decoder = Decoder::for_comparison(
        names,
        "2.20",
        Compat(8, 3, 24),
        &[Compat(8, 3, 24), Compat(8, 3, 27)],
    );
    let mut verdict = Verdict::new("rows");
    compare_descriptor(
        CATALOG,
        old,
        staged,
        &|uuid| plan.kinds.get(uuid).copied(),
        &labels,
        &decoder,
        &mut verdict,
    );
    verdict
}

/// The pair of a configuration kept at 8.3.24 whose catalog a native apply
/// has restructured once: the row `Config` held before (56), the row it holds
/// now (57, with `Rcheck1`), and the names.
struct Twin {
    before: Vec<u8>,
    after_apply: Vec<u8>,
    names: NameIndex,
}

fn twin() -> Twin {
    let first = catalog_xml(9, &attribute(A1, "Rcheck1", 30));
    let (before, _) = compile(&catalog_xml(9, ""), "Version8_3_24");
    let (after_apply, names) = compile(&first, "Version8_3_27");
    Twin {
        before,
        after_apply,
        names,
    }
}

fn head(row: &[u8]) -> String {
    String::from_utf8_lossy(row)
        .trim_start_matches('\u{feff}')
        .chars()
        .take(40)
        .collect()
}

#[test]
fn the_twin_rows_are_the_two_record_formats() {
    let twin = twin();
    assert!(
        head(&twin.before).starts_with("{1,\r\n{56,"),
        "{}",
        head(&twin.before)
    );
    assert!(
        head(&twin.after_apply).starts_with("{1,\r\n{57,"),
        "{}",
        head(&twin.after_apply)
    );
}

#[test]
fn a_stored_row_in_the_newer_format_is_read_like_a_staged_one() {
    let twin = twin();
    let decoder = Decoder::for_comparison(
        twin.names.clone(),
        "2.20",
        Compat(8, 3, 24),
        &[Compat(8, 3, 27)],
    );
    // The active mode alone does not read it: this is what stopped the check.
    let error = decoder.decode("Catalog", &twin.after_apply).unwrap_err();
    assert!(
        format!("{error:#}").contains("record version 57, the compatibility mode stores 56"),
        "{error:#}"
    );
    // A stored row is read in the other mode, and says what it holds.
    let stored = decoder.decode_stored("Catalog", &twin.after_apply).unwrap();
    assert!(
        stored
            .path(&["ChildObjects"])
            .is_some_and(|children| children.children.iter().any(|child| child
                .path(&["Properties", "Name"])
                .is_some_and(|name| name.text == "Rcheck1")))
    );
    // The tree-against-database export of the same row goes the same way.
    let text = decoder.export("Catalog", &twin.after_apply).unwrap();
    assert!(text.contains("Rcheck1"), "{}", &text[..text.len().min(200)]);
    // The old format still reads in the active mode.
    assert!(decoder.decode("Catalog", &twin.before).is_ok());
}

#[test]
fn a_second_attribute_after_a_native_apply_is_an_operation_of_s1() {
    let twin = twin();
    let second = catalog_xml(
        9,
        &format!(
            "{}{}",
            attribute(A1, "Rcheck1", 30),
            attribute(A2, "Rcheck2", 20)
        ),
    );
    let (staged, _) = compile(&second, "Version8_3_27");
    let verdict = compare(&twin.after_apply, &staged, twin.names.clone());
    assert_eq!(verdict.reasons.len(), 1, "{:#?}", verdict.reasons);
    let reason = &verdict.reasons[0];
    assert_eq!(reason.rule, RuleId::ColumnAddedOrDropped);
    assert_eq!(reason.class, ReasonClass::Structure);
    assert_eq!(reason.property, "ChildObjects/Attribute[Rcheck2]");
    let class = s1::classify(&verdict);
    assert!(class.accepted(), "{:#?}", class.refusals);
    assert_eq!(class.operations.len(), 1);
    assert_eq!(class.operations[0].name(), "add-attribute");
    assert_eq!(class.operations[0].object().full_name(), "Catalog.K");
}

#[test]
fn a_wider_string_after_a_native_apply_is_widened() {
    let twin = twin();
    let (staged, _) = compile(
        &catalog_xml(9, &attribute(A1, "Rcheck1", 60)),
        "Version8_3_27",
    );
    let verdict = compare(&twin.after_apply, &staged, twin.names.clone());
    let class = s1::classify(&verdict);
    assert!(
        class.accepted(),
        "{:#?} {:#?}",
        verdict.reasons,
        class.refusals
    );
    assert_eq!(class.operations.len(), 1);
    assert_eq!(class.operations[0].name(), "widen-string");
}

#[test]
fn another_property_after_a_native_apply_is_refused_by_name_not_unknown() {
    let twin = twin();
    let (staged, _) = compile(
        &catalog_xml(12, &attribute(A1, "Rcheck1", 30)),
        "Version8_3_27",
    );
    let verdict = compare(&twin.after_apply, &staged, twin.names.clone());
    assert_eq!(verdict.reasons.len(), 1, "{:#?}", verdict.reasons);
    assert_eq!(verdict.reasons[0].class, ReasonClass::Structure);
    assert_eq!(verdict.reasons[0].property, "Properties/CodeLength");
    let class = s1::classify(&verdict);
    assert_eq!(class.refusals.len(), 1);
    assert_eq!(class.refusals[0].code, RefusalCode::PropertyOutsideS1);
}

#[test]
fn a_staged_row_written_for_the_compatibility_mode_is_no_noise_against_the_newer_stored_row() {
    // The same descriptor staged in the older format (an import that writes
    // for the compatibility mode) over the promoted newer row: the bytes
    // differ, the descriptor does not, and the difference is the record
    // format read the other way round.
    let twin = twin();
    let (staged, _) = compile(
        &catalog_xml(9, &attribute(A1, "Rcheck1", 30)),
        "Version8_3_24",
    );
    assert_ne!(staged, twin.after_apply);
    let verdict = compare(&twin.after_apply, &staged, twin.names.clone());
    assert!(!verdict.needs_restructuring, "{:#?}", verdict.reasons);
    assert_eq!(verdict.stats.format_upgrades, 1);
}
