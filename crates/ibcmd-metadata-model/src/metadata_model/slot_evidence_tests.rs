//! Slot evidence for the register and chart families (issue #389).
//!
//! The extension `_ДемоРасширение` of the БСП 8.3.27 demonstration base owns
//! an accounting register, a calculation register and three charts. Their XML
//! is what the platform exported and their rows are the inflated `Config`
//! rows of the same base
//! (`tests/fixtures/native-evidence/extension-register-slots/`). Unlike the
//! main configurations they separate properties that every other object on
//! record holds at one value: `AutoOrderByCode` false, `DependenceOnCalculationTypes`
//! DontUse, `BasePeriod` false, an accounting register that is Managed.
//!
//! Each test edits ONE property of a fixture, compiles it and requires that
//! exactly the evidenced slot of the owner record moves, to the evidenced
//! code. The slot numbers come from the correlation of property values with
//! slot values over ten accounting registers, six charts of accounts, five
//! charts of calculation types, 36 charts of characteristic types and five
//! calculation registers of the БСП 8.3.27, БСП 8.5, ERP УХ 8.3.27 and
//! extension corpora.

use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::brace::{Brace, parse_row, serialize};
use super::export::{ExportContext, NameIndex, export_descriptor};
use super::objects::parts::compatibility;
use super::{DescriptorContext, compile_descriptor};

struct Fixture {
    kind: &'static str,
    folder: &'static str,
    stem: &'static str,
    xml: &'static str,
    row: &'static str,
}

macro_rules! fixture {
    ($kind:literal, $folder:literal, $stem:literal) => {
        Fixture {
            kind: $kind,
            folder: $folder,
            stem: $stem,
            xml: include_str!(concat!(
                "../../../../tests/fixtures/native-evidence/extension-register-slots/",
                $kind,
                ".xml"
            )),
            row: include_str!(concat!(
                "../../../../tests/fixtures/native-evidence/extension-register-slots/",
                $kind,
                ".row.txt"
            )),
        }
    };
}

const FIXTURES: [Fixture; 5] = [
    fixture!(
        "AccountingRegister",
        "AccountingRegisters",
        "_ДемоРегистрБухгалтерииРасширение"
    ),
    fixture!(
        "ChartOfAccounts",
        "ChartsOfAccounts",
        "_ДемоПланСчетовРасширение"
    ),
    fixture!(
        "ChartOfCalculationTypes",
        "ChartsOfCalculationTypes",
        "_ДемоПланВидовРасчетаРасширение"
    ),
    fixture!(
        "ChartOfCharacteristicTypes",
        "ChartsOfCharacteristicTypes",
        "_ДемоПланВидовХарактеристикРасширение"
    ),
    fixture!(
        "CalculationRegister",
        "CalculationRegisters",
        "_ДемоРегистрРасчетаРасширение"
    ),
];

/// The compatibility the extension's rows were stored in (the
/// `Configuration.xml` of an extension names none).
const CONFIGURATION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20"><Configuration uuid="11111111-1111-1111-1111-111111111111"><Properties><Name>C</Name><CompatibilityMode>Version8_3_24</CompatibilityMode></Properties></Configuration></MetaDataObject>"#;

/// What the two registers' dimensions and resources name: the catalog of the
/// extended configuration the extension adopts, reduced to the one generated
/// type they refer to.
const NOMENCLATURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20"><Catalog uuid="2cb40f25-2c61-41e9-9689-e1e4c639e8cb"><InternalInfo><xr:GeneratedType name="CatalogRef._ДемоНоменклатура" category="Ref"><xr:TypeId>5aa1e03d-4e6c-466a-b4d0-e7acaa599773</xr:TypeId><xr:ValueId>f7e76c5c-892b-4146-972e-8231cdd9763c</xr:ValueId></xr:GeneratedType></InternalInfo><Properties><Name>_ДемоНоменклатура</Name></Properties></Catalog></MetaDataObject>"#;

fn fixture(kind: &str) -> &'static Fixture {
    FIXTURES
        .iter()
        .find(|fixture| fixture.kind == kind)
        .expect("fixture kind")
}

/// The five fixtures as one source tree in a folder of their own.
struct Tree {
    root: std::path::PathBuf,
}

impl Tree {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "ibcmd-slot-evidence-{}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Configuration.xml"), CONFIGURATION).unwrap();
        fs::create_dir_all(root.join("Catalogs")).unwrap();
        fs::write(
            root.join("Catalogs").join("_ДемоНоменклатура.xml"),
            NOMENCLATURE,
        )
        .unwrap();
        for fixture in &FIXTURES {
            let folder = root.join(fixture.folder);
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join(format!("{}.xml", fixture.stem)), fixture.xml).unwrap();
        }
        Self { root }
    }

    fn context(&self) -> DescriptorContext {
        DescriptorContext::new(&self.root, "2.20").unwrap()
    }

    /// The row of one fixture, its XML edited first: `(from, to)` pairs,
    /// each applied to the first occurrence, which must lie in the owner's
    /// own properties (they come before its child objects).
    fn compile(&self, kind: &str, edits: &[(&str, &str)]) -> Vec<u8> {
        let fixture = fixture(kind);
        let mut xml = fixture.xml.to_string();
        for (from, to) in edits {
            let at = xml
                .find(from)
                .unwrap_or_else(|| panic!("{kind} has no `{from}`"));
            assert!(
                at < xml.find("<ChildObjects").unwrap(),
                "`{from}` belongs to a child of {kind}"
            );
            xml = xml.replacen(from, to, 1);
        }
        let path = self
            .root
            .join(fixture.folder)
            .join(format!("{}.xml", fixture.stem));
        compile_descriptor(kind, &path, xml.as_bytes(), &self.context()).unwrap()
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

/// The owner record's slots of a row, each as its serialized text.
fn owner(row: &[u8]) -> Vec<String> {
    let tree = parse_row(row).unwrap();
    tree.at(&[1])
        .and_then(Brace::as_list)
        .expect("a descriptor row has an owner record")
        .iter()
        .map(|slot| serialize(slot).replace("\r\n", ""))
        .collect()
}

/// `<Name>value</Name>`.
fn element(name: &str, value: &str) -> String {
    format!("<{name}>{value}</{name}>")
}

/// Edits one property of a fixture and returns the slots that moved against
/// the stored row: `(slot, stored, now)`.
fn moved(
    tree: &Tree,
    kind: &str,
    property: &str,
    from: &str,
    to: &str,
) -> Vec<(usize, String, String)> {
    let stored = owner(fixture(kind).row.as_bytes());
    let now = owner(&tree.compile(kind, &[(&element(property, from), &element(property, to))]));
    assert_eq!(
        stored.len(),
        now.len(),
        "{kind}.{property} changed the record length"
    );
    stored
        .into_iter()
        .zip(now)
        .enumerate()
        .filter(|(_, (stored, now))| stored != now)
        .map(|(slot, (stored, now))| (slot, stored, now))
        .collect()
}

/// `(property, stored value, new value, slot, code the slot now holds)`.
type Case = (
    &'static str,
    &'static str,
    &'static str,
    usize,
    &'static str,
);

fn check(kind: &str, cases: &[Case]) {
    let tree = Tree::new();
    for (property, from, to, slot, code) in cases {
        let moved = moved(&tree, kind, property, from, to);
        assert_eq!(
            moved.len(),
            1,
            "{kind}.{property} {from} -> {to} moved {moved:?}, expected slot {slot} only"
        );
        let (moved_slot, _, now) = &moved[0];
        assert_eq!(
            (*moved_slot, now.as_str()),
            (*slot, *code),
            "{kind}.{property} {from} -> {to}"
        );
    }
}

/// The five rows compile to the stored text, BOM and line breaks included.
#[test]
fn the_extension_registers_and_charts_compile_to_their_stored_rows() {
    let tree = Tree::new();
    for fixture in &FIXTURES {
        let row = String::from_utf8(tree.compile(fixture.kind, &[])).unwrap();
        assert!(
            row == fixture.row,
            "{} does not compile to its stored row",
            fixture.kind
        );
    }
}

/// And the stored rows read back to every scalar property the platform
/// exported. (The main export writes `<StandardTabularSections/>` where the
/// extension's charts have none, which is the extension export's business, so
/// the comparison is per property line and not per file.)
#[test]
fn the_stored_rows_export_to_the_native_properties() {
    let tree = Tree::new();
    let context = tree.context();
    let export = ExportContext {
        names: NameIndex::from_config_index(&context.index),
        version: "2.20".to_string(),
        compat: compatibility(&context),
    };
    let properties = |text: &str| -> Vec<String> {
        let start = text.find("<Properties>").unwrap();
        let end = text.find("</Properties>").unwrap();
        text[start..end]
            .lines()
            .filter(|line| line.starts_with("\t\t\t<") && !line.starts_with("\t\t\t\t"))
            .map(|line| line.trim_end().to_string())
            .filter(|line| line.ends_with("/>") || line.contains("</"))
            .collect()
    };
    for fixture in &FIXTURES {
        let read =
            properties(&export_descriptor(fixture.kind, fixture.row.as_bytes(), &export).unwrap());
        let native = properties(fixture.xml);
        assert!(
            native.len() > 10,
            "{} has few scalar properties",
            fixture.kind
        );
        for line in &native {
            assert!(
                read.contains(line),
                "{} reads back without `{}`",
                fixture.kind,
                line.trim()
            );
        }
    }
}

/// Accounting register, 30 slots (ten registers on record: DataLockControlMode
/// `0` on nine, `1` on the extension's; FullTextSearch `0` on all ten;
/// Correspondence `0` on three; PeriodAdjustmentLength `0` on eight, `1` on
/// two).
#[test]
fn accounting_register_properties_ride_their_stored_slots() {
    check(
        "AccountingRegister",
        &[
            ("UseStandardCommands", "true", "false", 16, "0"),
            ("IncludeHelpInContents", "false", "true", 17, "1"),
            ("Correspondence", "true", "false", 20, "0"),
            ("DataLockControlMode", "Managed", "Automatic", 21, "0"),
            ("FullTextSearch", "DontUse", "Use", 22, "1"),
            ("EnableTotalsSplitting", "true", "false", 23, "0"),
        ],
    );
}

/// A period adjustment writes version 22: a second `22`, every later slot one
/// further, the length last. ERP УХ `Хозрасчетный` and
/// `КорректировкиНалоговойБазы` (both `PeriodAdjustmentLength` 1) store
/// UseStandardCommands `1` at 17, IncludeHelpInContents `1`/`0` at 18,
/// Correspondence `1` at 21, DataLockControlMode `0` at 22, FullTextSearch `0`
/// at 23, EnableTotalsSplitting `1` at 24 and the length `1` at 30.
#[test]
fn a_period_adjustment_moves_the_accounting_register_to_version_22() {
    let tree = Tree::new();
    let stored = owner(fixture("AccountingRegister").row.as_bytes());
    assert_eq!(stored.len(), 30);
    assert_eq!(stored[0], "21");

    let adjusted = |edits: &[(&str, &str)]| {
        let mut all = vec![(
            "<PeriodAdjustmentLength>0</PeriodAdjustmentLength>",
            "<PeriodAdjustmentLength>1</PeriodAdjustmentLength>",
        )];
        all.extend_from_slice(edits);
        owner(&tree.compile("AccountingRegister", &all))
    };
    let now = adjusted(&[]);
    assert_eq!(now.len(), 31);
    assert_eq!(&now[..2], ["22", "22"]);
    assert_eq!(&now[2..30], &stored[1..29]);
    assert_eq!(now[30], "1");

    // The property values of `Хозрасчетный`.
    let khozrashchetny = adjusted(&[
        (
            "<IncludeHelpInContents>false</IncludeHelpInContents>",
            "<IncludeHelpInContents>true</IncludeHelpInContents>",
        ),
        (
            "<DataLockControlMode>Managed</DataLockControlMode>",
            "<DataLockControlMode>Automatic</DataLockControlMode>",
        ),
    ]);
    for (slot, code) in [
        (17, "1"),
        (18, "1"),
        (21, "1"),
        (22, "0"),
        (23, "0"),
        (24, "1"),
        (30, "1"),
    ] {
        assert_eq!(khozrashchetny[slot], code, "slot {slot}");
    }
}

/// Chart of accounts, 57 slots (six charts on record). Slot 24 is
/// AutoOrderByCode (`0` on the extension's chart, `1` on the other five) and
/// 27 the edit type: the pair the model once swapped.
#[test]
fn chart_of_accounts_properties_ride_their_stored_slots() {
    check(
        "ChartOfAccounts",
        &[
            ("UseStandardCommands", "true", "false", 16, "0"),
            ("IncludeHelpInContents", "false", "true", 17, "1"),
            ("MaxExtDimensionCount", "0", "3", 20, "3"),
            ("CodeLength", "9", "11", 22, "11"),
            ("DescriptionLength", "25", "30", 23, "30"),
            ("AutoOrderByCode", "false", "true", 24, "1"),
            ("OrderLength", "0", "7", 25, "7"),
            ("DefaultPresentation", "AsCode", "AsDescription", 26, "1"),
            ("EditType", "InDialog", "InList", 27, "0"),
            ("EditType", "InDialog", "BothWays", 27, "2"),
            ("ChoiceMode", "BothWays", "FromForm", 31, "0"),
            ("QuickChoice", "false", "true", 32, "1"),
            ("CheckUnique", "true", "false", 34, "0"),
            (
                "CodeSeries",
                "WholeChartOfAccounts",
                "WithinSubordination",
                35,
                "1",
            ),
            ("DataLockControlMode", "Managed", "Automatic", 36, "0"),
            ("FullTextSearch", "Use", "DontUse", 37, "0"),
            ("CreateOnInput", "DontUse", "Use", 49, "2"),
            ("PredefinedDataUpdate", "Auto", "DontAutoUpdate", 51, "2"),
            ("ChoiceHistoryOnInput", "Auto", "DontUse", 53, "1"),
            ("DataHistory", "DontUse", "Use", 54, "1"),
        ],
    );
}

/// Chart of calculation types, 63 slots (five charts on record). Slot 27 is
/// DependenceOnCalculationTypes (`0` on the extension's chart, `1` on the
/// other four) and 35 the edit type: the pair the model once swapped.
#[test]
fn chart_of_calculation_types_properties_ride_their_stored_slots() {
    check(
        "ChartOfCalculationTypes",
        &[
            ("UseStandardCommands", "true", "false", 24, "0"),
            ("CodeLength", "9", "5", 25, "5"),
            ("CodeType", "String", "Number", 26, "0"),
            (
                "DependenceOnCalculationTypes",
                "DontUse",
                "OnActionPeriod",
                27,
                "1",
            ),
            (
                "DependenceOnCalculationTypes",
                "DontUse",
                "OnBasePeriod",
                27,
                "2",
            ),
            ("ActionPeriodUse", "false", "true", 29, "1"),
            ("DescriptionLength", "40", "100", 30, "100"),
            ("DefaultPresentation", "AsDescription", "AsCode", 31, "0"),
            ("EditType", "InDialog", "InList", 35, "0"),
            ("IncludeHelpInContents", "false", "true", 37, "1"),
            ("ChoiceMode", "BothWays", "QuickChoice", 38, "1"),
            ("QuickChoice", "false", "true", 39, "1"),
            ("DataLockControlMode", "Managed", "Automatic", 41, "0"),
            ("FullTextSearch", "Use", "DontUse", 42, "0"),
            ("CodeAllowedLength", "Variable", "Fixed", 53, "0"),
            ("CreateOnInput", "DontUse", "Use", 55, "2"),
            ("PredefinedDataUpdate", "Auto", "AutoUpdate", 57, "1"),
            ("ChoiceHistoryOnInput", "Auto", "DontUse", 59, "1"),
            ("DataHistory", "DontUse", "Use", 60, "1"),
        ],
    );
}

/// Chart of characteristic types, 59 slots (36 charts on record).
#[test]
fn chart_of_characteristic_types_properties_ride_their_stored_slots() {
    check(
        "ChartOfCharacteristicTypes",
        &[
            ("UseStandardCommands", "true", "false", 14, "0"),
            ("IncludeHelpInContents", "false", "true", 16, "1"),
            ("Hierarchical", "false", "true", 19, "1"),
            ("FoldersOnTop", "true", "false", 20, "0"),
            ("CodeLength", "9", "10", 21, "10"),
            ("Autonumbering", "true", "false", 22, "0"),
            ("DescriptionLength", "25", "30", 23, "30"),
            ("DefaultPresentation", "AsDescription", "AsCode", 24, "0"),
            ("EditType", "InDialog", "InList", 25, "0"),
            ("ChoiceMode", "BothWays", "FromForm", 31, "0"),
            ("QuickChoice", "false", "true", 32, "1"),
            ("CheckUnique", "true", "false", 34, "0"),
            (
                "CodeSeries",
                "WholeCharacteristicKind",
                "WithinSubordination",
                35,
                "1",
            ),
            ("DataLockControlMode", "Managed", "Automatic", 36, "0"),
            ("FullTextSearch", "Use", "DontUse", 37, "0"),
            ("CodeAllowedLength", "Variable", "Fixed", 49, "0"),
            ("CreateOnInput", "DontUse", "Use", 51, "2"),
            ("PredefinedDataUpdate", "Auto", "AutoUpdate", 53, "1"),
            ("ChoiceHistoryOnInput", "Auto", "DontUse", 55, "1"),
            ("DataHistory", "DontUse", "Use", 56, "1"),
        ],
    );
}

/// Calculation register, 33 slots (five registers on record). ActionPeriod
/// (17) and BasePeriod (18) are each `1` on all but the registers named:
/// `false` on ERP УХ `Удержания` and the extension's for the first, on the
/// extension's alone for the second.
#[test]
fn calculation_register_properties_ride_their_stored_slots() {
    check(
        "CalculationRegister",
        &[
            ("ActionPeriod", "false", "true", 17, "1"),
            ("BasePeriod", "false", "true", 18, "1"),
            ("UseStandardCommands", "true", "false", 24, "0"),
            ("IncludeHelpInContents", "false", "true", 25, "1"),
            ("DataLockControlMode", "Managed", "Automatic", 26, "0"),
            ("FullTextSearch", "DontUse", "Use", 27, "1"),
        ],
    );
}

/// A characteristic can name no source: `from=""` on both groups, stored as two
/// nil uuids with sentinel fields only. This is the item the БСП 8.5 extension
/// catalog `_ДемоСегментыПартнеровРасширение` carries (row
/// `263857ac-f92c-4902-8a66-d632472a7c30`); the model used to refuse the empty
/// `from` while compiling it.
#[test]
fn a_characteristic_without_a_source_compiles_to_nil_sources_and_reads_back() {
    const BLOCK: &str = r#"<Characteristics>
				<xr:Characteristic>
					<xr:CharacteristicTypes from="">
						<xr:KeyField>0</xr:KeyField>
						<xr:TypesFilterField>0</xr:TypesFilterField>
						<xr:TypesFilterValue xsi:nil="true"/>
						<xr:DataPathField>-1</xr:DataPathField>
						<xr:MultipleValuesUseField>-1</xr:MultipleValuesUseField>
					</xr:CharacteristicTypes>
					<xr:CharacteristicValues from="">
						<xr:ObjectField>0</xr:ObjectField>
						<xr:TypeField>0</xr:TypeField>
						<xr:ValueField>0</xr:ValueField>
						<xr:MultipleValuesKeyField>-1</xr:MultipleValuesKeyField>
						<xr:MultipleValuesOrderField>-1</xr:MultipleValuesOrderField>
					</xr:CharacteristicValues>
				</xr:Characteristic>
			</Characteristics>"#;
    const STORED: &str = r##"{0,{1,{"#",fe839d42-d094-40ba-b903-75bccc21ba30,{4,{1,00000000-0000-0000-0000-000000000000},{1,00000000-0000-0000-0000-000000000000},{1,{0},0},{1,{0},0},{1,{0},0},{1,{0},0},{"U"},{1,{0},0},{1,{-1},0},{1,{-1},0},{1,{-1},0},{1,{-1},0}}}}}"##;
    let tree = Tree::new();
    let row = tree.compile(
        "ChartOfCharacteristicTypes",
        &[("<Characteristics/>", BLOCK)],
    );
    let holders = owner(&row)
        .into_iter()
        .filter(|slot| slot == STORED)
        .count();
    assert_eq!(holders, 1, "the owner record holds the stored item once");

    let context = tree.context();
    let export = ExportContext {
        names: NameIndex::from_config_index(&context.index),
        version: "2.20".to_string(),
        compat: compatibility(&context),
    };
    let xml = export_descriptor("ChartOfCharacteristicTypes", &row, &export).unwrap();
    assert_eq!(xml.matches("from=\"\"").count(), 2, "{xml}");
    assert!(
        xml.contains("<xr:TypesFilterValue xsi:nil=\"true\"/>"),
        "{xml}"
    );
}
