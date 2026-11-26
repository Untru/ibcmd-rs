//! The refusal matrix of S1 (#404): every case the check's probes ran
//! (`docs/apply/restructuring-check.md`, sections 6.1 and 6.2: the focused
//! runs `p1` ... `p8` and the 159 single-property probes) goes through the
//! check's own comparison and the S1 classification, and comes out as
//!
//! * `Harmless`: no reason at all,
//! * an operation of S1, or
//! * a refusal that names its code.
//!
//! Two things are asserted for every case. What the native platform acted on
//! is never harmless (nothing outside S1 gets through), and what the case is
//! expected to be is what it is (the matrix does not drift). A case the
//! native platform left alone may still be refused: the check is
//! fail-closed, and the number of such over-refusals is fixed below.

use super::descriptor::{self, ObjectRef};
use super::s1::{self, RefusalCode};
use super::{Verdict, tree_diff};
use crate::metadata_model::xml::{Element, parse_element_tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Harmless,
    /// The name of the S1 operation.
    Operation(&'static str),
    Refused(RefusalCode),
}

fn outcome_of(verdict: &Verdict) -> Outcome {
    let class = s1::classify(verdict);
    match (class.operations.as_slice(), class.refusals.as_slice()) {
        ([], []) => {
            assert!(
                !verdict.needs_restructuring,
                "a reason was taken for nothing: {:?}",
                verdict.reasons
            );
            Outcome::Harmless
        }
        ([operation], []) => Outcome::Operation(operation.name()),
        (_, [refusal, ..]) => {
            // One refused reason refuses the stage; the first names it.
            Outcome::Refused(refusal.code)
        }
        (operations, []) => panic!("several operations for one case: {operations:?}"),
    }
}

fn element(text: &str) -> Element {
    parse_element_tree(text.as_bytes()).unwrap_or_else(|error| panic!("{text}: {error:#}"))
}

/// `<Kind uuid="u"><Properties><Name>N</Name>{properties}</Properties>
/// <ChildObjects>{children}</ChildObjects></Kind>`
fn object(kind: &str, properties: &str, children: &str) -> Element {
    element(&format!(
        "<{kind} uuid=\"u\"><Properties><Name>N</Name>{properties}</Properties>\
         <ChildObjects>{children}</ChildObjects></{kind}>"
    ))
}

fn attribute(uuid: &str, name: &str, length: u32, indexing: &str, extra: &str) -> String {
    format!(
        "<Attribute uuid=\"{uuid}\"><Properties><Name>{name}</Name>\
         <Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>{length}</v8:Length>\
         <v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers></Type>\
         <Indexing>{indexing}</Indexing>{extra}</Properties></Attribute>"
    )
}

fn section(uuid: &str, name: &str, extra: &str, columns: &str) -> String {
    format!(
        "<TabularSection uuid=\"{uuid}\"><Properties><Name>{name}</Name>{extra}</Properties>\
         <ChildObjects>{columns}</ChildObjects></TabularSection>"
    )
}

const A1: &str = "aaaaaaaa-0000-4000-8000-000000000001";
const A2: &str = "aaaaaaaa-0000-4000-8000-000000000002";
const T1: &str = "bbbbbbbb-0000-4000-8000-000000000001";

/// The check's comparison of one object on two sides.
fn compare(kind: &str, old: &Element, new: &Element) -> Verdict {
    let mut verdict = Verdict::new("rows");
    let name = format!("{kind}.N");
    descriptor::compare(
        &ObjectRef {
            kind,
            name: &name,
            file_name: "row",
            id: "u",
        },
        old,
        new,
        &mut verdict,
    );
    verdict
}

struct Case {
    /// The probe run and the change: `p2 Indexing`.
    id: String,
    /// What the native platform did to it (a restructuring, or nothing).
    acted: bool,
    verdict: Verdict,
    expect: Outcome,
}

fn case(id: &str, acted: bool, verdict: Verdict, expect: Outcome) -> Case {
    Case {
        id: id.to_string(),
        acted,
        verdict,
        expect,
    }
}

const REFUSE_PROPERTY: Outcome = Outcome::Refused(RefusalCode::PropertyOutsideS1);
const REFUSE_ATTRIBUTE_PROPERTY: Outcome =
    Outcome::Refused(RefusalCode::AttributePropertyOutsideS1);

/// `p1_catalog_props`: 18 catalogs, one property each.
fn p1() -> Vec<Case> {
    [
        ("Autonumbering", "true", "false", false),
        ("Explanation", "a", "b", false),
        ("ObjectPresentation", "a", "b", false),
        ("ListPresentation", "a", "b", false),
        ("DescriptionLength", "100", "101", true),
        ("DefaultPresentation", "AsDescription", "AsCode", false),
        ("QuickChoice", "true", "false", false),
        ("ChoiceMode", "BothWays", "FromForm", false),
        ("CodeLength", "9", "12", true),
        ("IncludeHelpInContents", "true", "false", false),
        ("CheckUnique", "true", "false", false),
        ("DataHistory", "DontUse", "Use", true),
        ("PredefinedDataUpdate", "Auto", "DontAutoUpdate", false),
        ("FullTextSearch", "Use", "DontUse", false),
        ("DataLockControlMode", "Managed", "Automatic", false),
        ("UseStandardCommands", "true", "false", false),
        ("ChoiceHistoryOnInput", "Auto", "DontUse", false),
        ("EditType", "InDialog", "InList", false),
    ]
    .into_iter()
    .map(|(tag, from, to, acted)| {
        let side = |value: &str| object("Catalog", &format!("<{tag}>{value}</{tag}>"), "");
        let expect = if acted {
            REFUSE_PROPERTY
        } else {
            Outcome::Harmless
        };
        case(
            &format!("p1 Catalog {tag}"),
            acted,
            compare("Catalog", &side(from), &side(to)),
            expect,
        )
    })
    .collect()
}

/// `p2_attribute_props`: 16 catalogs, one attribute property each.
fn p2() -> Vec<Case> {
    let base = |length: u32, indexing: &str, extra: &str| {
        object("Catalog", "", &attribute(A1, "A", length, indexing, extra))
    };
    let mut cases = Vec::new();
    for (tag, from, to) in [
        ("Synonym", "a", "b"),
        ("ToolTip", "a", "b"),
        ("FillChecking", "DontCheck", "ShowError"),
        ("QuickChoice", "Auto", "DontUse"),
        ("FullTextSearch", "Use", "DontUse"),
        ("ChoiceHistoryOnInput", "Auto", "DontUse"),
        ("CreateOnInput", "Auto", "DontUse"),
        ("MultiLine", "false", "true"),
        ("PasswordMode", "false", "true"),
        ("ExtendedEdit", "false", "true"),
        ("MarkNegatives", "false", "true"),
        ("FillFromFillingValue", "false", "true"),
    ] {
        let side = |value: &str| base(50, "DontIndex", &format!("<{tag}>{value}</{tag}>"));
        cases.push(case(
            &format!("p2 attribute {tag}"),
            false,
            compare("Catalog", &side(from), &side(to)),
            Outcome::Harmless,
        ));
    }
    // The name is a property of the attribute like the others.
    cases.push(case(
        "p2 attribute Name",
        false,
        compare(
            "Catalog",
            &object("Catalog", "", &attribute(A1, "A", 50, "DontIndex", "")),
            &object("Catalog", "", &attribute(A1, "B", 50, "DontIndex", "")),
        ),
        Outcome::Harmless,
    ));
    // The one the platform left alone because the object's own history was
    // off; the check cannot see that, and refuses.
    let history = |value: &str| {
        base(
            50,
            "DontIndex",
            &format!("<DataHistory>{value}</DataHistory>"),
        )
    };
    cases.push(case(
        "p2 attribute DataHistory",
        false,
        compare("Catalog", &history("DontUse"), &history("Use")),
        REFUSE_ATTRIBUTE_PROPERTY,
    ));
    cases.push(case(
        "p2 attribute Indexing",
        true,
        compare(
            "Catalog",
            &base(50, "DontIndex", ""),
            &base(50, "Index", ""),
        ),
        Outcome::Operation("switch-index"),
    ));
    cases.push(case(
        "p2 attribute StringLength",
        true,
        compare(
            "Catalog",
            &base(50, "DontIndex", ""),
            &base(100, "DontIndex", ""),
        ),
        Outcome::Operation("widen-string"),
    ));
    cases
}

/// `p4_children`.
fn p4() -> Vec<Case> {
    let mut cases = Vec::new();
    let with = |extra: &str| object("Catalog", "", &attribute(A1, "A", 50, "DontIndex", extra));
    cases.push(case(
        "p4 attribute ChoiceFoldersAndItems",
        false,
        compare(
            "Catalog",
            &with("<ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems>"),
            &with("<ChoiceFoldersAndItems>FoldersAndItems</ChoiceFoldersAndItems>"),
        ),
        Outcome::Harmless,
    ));
    cases.push(case(
        "p4 attribute Use",
        true,
        compare(
            "Catalog",
            &with("<Use>ForItem</Use>"),
            &with("<Use>ForFolderAndItem</Use>"),
        ),
        REFUSE_ATTRIBUTE_PROPERTY,
    ));
    cases.push(case(
        "p4 attribute Indexing off",
        true,
        compare(
            "Catalog",
            &object("Catalog", "", &attribute(A1, "A", 50, "Index", "")),
            &object("Catalog", "", &attribute(A1, "A", 50, "DontIndex", "")),
        ),
        Outcome::Operation("switch-index"),
    ));
    // Five properties of standard attributes on five catalogs.
    for (tag, from, to, expect) in [
        ("FillChecking", "ShowError", "DontCheck", Outcome::Harmless),
        ("MultiLine", "false", "true", Outcome::Harmless),
        ("FullTextSearch", "Use", "DontUse", Outcome::Harmless),
        ("ExtendedEdit", "false", "true", Outcome::Harmless),
        // The platform left it alone (the object's history was off).
        ("DataHistory", "DontUse", "Use", REFUSE_PROPERTY),
    ] {
        let standard = |value: &str| {
            object(
                "Catalog",
                &format!(
                    "<StandardAttributes><xr:StandardAttribute name=\"Description\">\
                     <{tag}>{value}</{tag}></xr:StandardAttribute></StandardAttributes>"
                ),
                "",
            )
        };
        cases.push(case(
            &format!("p4 standard attribute {tag}"),
            false,
            compare("Catalog", &standard(from), &standard(to)),
            expect,
        ));
    }
    let values = |extra: &str| {
        object(
            "Enum",
            "",
            &format!(
                "<EnumValue uuid=\"{A1}\"><Properties><Name>V1</Name></Properties></EnumValue>{extra}"
            ),
        )
    };
    cases.push(case(
        "p4 enumeration value added",
        true,
        compare(
            "Enum",
            &values(""),
            &values(&format!(
                "<EnumValue uuid=\"{A2}\"><Properties><Name>V2</Name></Properties></EnumValue>"
            )),
        ),
        Outcome::Refused(RefusalCode::DataChange),
    ));
    let renamed = |name: &str| {
        object(
            "Enum",
            "",
            &format!(
                "<EnumValue uuid=\"{A1}\"><Properties><Name>{name}</Name></Properties></EnumValue>"
            ),
        )
    };
    cases.push(case(
        "p4 enumeration value renamed",
        false,
        compare("Enum", &renamed("V1"), &renamed("V9")),
        Outcome::Harmless,
    ));
    cases
}

/// `p5_children`.
fn p5() -> Vec<Case> {
    let mut cases = Vec::new();
    for kind in ["Report", "DataProcessor"] {
        cases.push(case(
            &format!("p5 attribute added to a {kind}"),
            false,
            compare(
                kind,
                &object(kind, "", ""),
                &object(kind, "", &attribute(A1, "A", 50, "DontIndex", "")),
            ),
            Outcome::Harmless,
        ));
    }
    let with_section =
        |extra: &str, column: &str| object("Document", "", &section(T1, "T", extra, column));
    let column = |extra: &str| {
        format!(
            "<Attribute uuid=\"{A1}\"><Properties><Name>C</Name>{extra}</Properties></Attribute>"
        )
    };
    cases.push(case(
        "p5 tabular section FillChecking",
        false,
        compare(
            "Document",
            &with_section("<FillChecking>DontCheck</FillChecking>", ""),
            &with_section("<FillChecking>ShowError</FillChecking>", ""),
        ),
        Outcome::Harmless,
    ));
    cases.push(case(
        "p5 tabular section attribute MultiLine",
        false,
        compare(
            "Document",
            &with_section("", &column("<MultiLine>false</MultiLine>")),
            &with_section("", &column("<MultiLine>true</MultiLine>")),
        ),
        Outcome::Harmless,
    ));
    cases.push(case(
        "p5 attribute added to a tabular section",
        true,
        compare(
            "Document",
            &with_section("", ""),
            &with_section("", &column("")),
        ),
        Outcome::Operation("add-section-attribute"),
    ));
    cases.push(case(
        "p5 tabular section added",
        true,
        compare(
            "Document",
            &object("Document", "", ""),
            &with_section("", &column("")),
        ),
        Outcome::Operation("add-tabular-section"),
    ));
    let dimension = |extra: &str| {
        format!(
            "<Dimension uuid=\"{A1}\"><Properties><Name>D</Name>{extra}</Properties></Dimension>"
        )
    };
    cases.push(case(
        "p5 dimension DenyIncompleteValues",
        false,
        compare(
            "InformationRegister",
            &object(
                "InformationRegister",
                "",
                &dimension("<DenyIncompleteValues>false</DenyIncompleteValues>"),
            ),
            &object(
                "InformationRegister",
                "",
                &dimension("<DenyIncompleteValues>true</DenyIncompleteValues>"),
            ),
        ),
        Outcome::Harmless,
    ));
    cases.push(case(
        "p5 dimension added to an accumulation register",
        true,
        compare(
            "AccumulationRegister",
            &object("AccumulationRegister", "", ""),
            &object("AccumulationRegister", "", &dimension("")),
        ),
        Outcome::Refused(RefusalCode::KindOutsideS1),
    ));
    cases.push(case(
        "p5 last attribute of a catalog dropped",
        true,
        compare(
            "Catalog",
            &object("Catalog", "", &attribute(A1, "A", 50, "DontIndex", "")),
            &object("Catalog", "", ""),
        ),
        Outcome::Operation("delete-attribute"),
    ));
    // The same for an attribute added to a catalog and a document.
    for kind in ["Catalog", "Document"] {
        cases.push(case(
            &format!("s1 attribute added to a {kind}"),
            true,
            compare(
                kind,
                &object(kind, "", ""),
                &object(kind, "", &attribute(A1, "A", 50, "DontIndex", "")),
            ),
            Outcome::Operation("add-attribute"),
        ));
    }
    cases
}

/// `p6_config`, `p6b_kinds`.
fn p6() -> Vec<Case> {
    let mut cases = Vec::new();
    for tag in [
        "Version",
        "UpdateCatalogAddress",
        "Copyright",
        "BriefInformation",
        "Vendor",
        "DetailedInformation",
        "VendorInformationAddress",
        "ConfigurationInformationAddress",
    ] {
        let side = |value: &str| object("Configuration", &format!("<{tag}>{value}</{tag}>"), "");
        cases.push(case(
            &format!("p6 Configuration {tag}"),
            false,
            compare("Configuration", &side("1"), &side("2")),
            Outcome::Harmless,
        ));
    }
    for tag in [
        "MethodName",
        "RestartCountOnFailure",
        "RestartIntervalOnFailure",
    ] {
        let side = |value: &str| object("ScheduledJob", &format!("<{tag}>{value}</{tag}>"), "");
        cases.push(case(
            &format!("p6 ScheduledJob {tag}"),
            true,
            compare("ScheduledJob", &side("1"), &side("2")),
            Outcome::Refused(RefusalCode::DataChange),
        ));
    }
    let event = |value: &str| object("EventSubscription", &format!("<Event>{value}</Event>"), "");
    cases.push(case(
        "p6 EventSubscription Event",
        false,
        compare(
            "EventSubscription",
            &event("BeforeWrite"),
            &event("OnWrite"),
        ),
        Outcome::Harmless,
    ));
    for kind in [
        "ScheduledJob",
        "EventSubscription",
        "WebService",
        "HTTPService",
        "XDTOPackage",
        "SettingsStorage",
        "Sequence",
    ] {
        let side = |value: &str| object(kind, &format!("<Comment>{value}</Comment>"), "");
        cases.push(case(
            &format!("p6 {kind} Comment"),
            false,
            compare(kind, &side("a"), &side("b")),
            Outcome::Harmless,
        ));
    }
    cases
}

/// `p7a_objects_add`, `p7b_objects_remove`: a common module, a catalog and a
/// role. The catalog is the platform's one restructuring, and it is two
/// reasons: the object and its listing in the configuration.
fn p7() -> Vec<Case> {
    let mut cases = Vec::new();
    for (run, added) in [("p7a", true), ("p7b", false)] {
        for kind in ["CommonModule", "Role"] {
            let mut verdict = Verdict::new("rows");
            descriptor::lifecycle(
                &ObjectRef {
                    kind,
                    name: &format!("{kind}.X"),
                    file_name: "row",
                    id: "u",
                },
                added,
                &mut verdict,
            );
            cases.push(case(
                &format!("{run} {kind}"),
                false,
                verdict,
                Outcome::Harmless,
            ));
        }
        let mut verdict = Verdict::new("rows");
        descriptor::lifecycle(
            &ObjectRef {
                kind: "Catalog",
                name: "Catalog.СправочникRcheck",
                file_name: "row",
                id: "u",
            },
            added,
            &mut verdict,
        );
        // A configuration lists many catalogs: the listing is a list of
        // plain values, and an item of it is named by its text.
        let listing = |names: &str| {
            object(
                "Configuration",
                "",
                &format!(
                    "<Language>Русский</Language>                     <Catalog>Первый</Catalog><Catalog>Второй</Catalog>{names}"
                ),
            )
        };
        let (old, new) = if added {
            (listing(""), listing("<Catalog>СправочникRcheck</Catalog>"))
        } else {
            (listing("<Catalog>СправочникRcheck</Catalog>"), listing(""))
        };
        descriptor::compare(
            &ObjectRef {
                kind: "Configuration",
                name: "Configuration",
                file_name: "row",
                id: "c",
            },
            &old,
            &new,
            &mut verdict,
        );
        cases.push(case(
            &format!("{run} Catalog"),
            true,
            verdict,
            if added {
                Outcome::Operation("add-object")
            } else {
                Outcome::Refused(RefusalCode::ObjectRemoved)
            },
        ));
    }
    cases
}

/// `p8_catalog_presentation`.
fn p8() -> Vec<Case> {
    let mut cases = Vec::new();
    for tag in [
        "ExtendedObjectPresentation",
        "ExtendedListPresentation",
        "FullTextSearchOnInputByString",
        "AuxiliaryObjectForm",
        "AuxiliaryListForm",
        "AuxiliaryChoiceForm",
        "AuxiliaryFolderForm",
        "AuxiliaryFolderChoiceForm",
    ] {
        let side = |value: &str| object("Catalog", &format!("<{tag}>{value}</{tag}>"), "");
        cases.push(case(
            &format!("p8 Catalog {tag}"),
            false,
            compare("Catalog", &side("a"), &side("b")),
            Outcome::Harmless,
        ));
    }
    let input = |fields: &str| {
        object(
            "Catalog",
            &format!("<InputByString>{fields}</InputByString>"),
            "",
        )
    };
    cases.push(case(
        "p8 Catalog InputByString",
        false,
        compare(
            "Catalog",
            &input("<xr:Field>Catalog.N.StandardAttribute.Description</xr:Field>"),
            &input(
                "<xr:Field>Catalog.N.StandardAttribute.Description</xr:Field>\
                 <xr:Field>Catalog.N.StandardAttribute.Code</xr:Field>",
            ),
        ),
        Outcome::Harmless,
    ));
    cases
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// `probe_auto_a`: one top-level property of one object, 159 times.
fn auto_probes() -> Vec<Case> {
    const TABLE: &str =
        include_str!("../../../../docs/apply/evidence/restructuring-check/probes-auto-a.tsv");
    TABLE
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 6, "{line}");
            let (kind, tag, from, to) = (fields[0], fields[2], fields[3], fields[4]);
            let acted = match fields[5] {
                "yes" => true,
                "no" => false,
                other => panic!("native acted: {other}"),
            };
            let side = |value: &str| object(kind, &format!("<{tag}>{}</{tag}>", escape(value)), "");
            let verdict = compare(kind, &side(from), &side(to));
            // A property change is never an operation of S1: harmless or
            // refused, the expectation is what the verdict says either way.
            let outcome = outcome_of(&verdict);
            Case {
                id: format!("auto {kind} {tag}"),
                acted,
                verdict,
                expect: outcome,
            }
        })
        .collect()
}

fn all_focused() -> Vec<Case> {
    let mut cases = Vec::new();
    for group in [p1(), p2(), p4(), p5(), p6(), p7(), p8()] {
        cases.extend(group);
    }
    cases
}

#[test]
fn every_focused_case_comes_out_as_expected_and_nothing_native_acted_on_is_harmless() {
    let cases = all_focused();
    assert_eq!(cases.len(), 89, "the focused cases");
    let mut operations = 0;
    // run -> (cases, native acted, harmless, operations, refused)
    let mut by_run = std::collections::BTreeMap::<String, [usize; 5]>::new();
    for case in &cases {
        let outcome = outcome_of(&case.verdict);
        assert_eq!(outcome, case.expect, "{}", case.id);
        let row = by_run
            .entry(case.id.split(' ').next().unwrap().to_string())
            .or_default();
        row[0] += 1;
        row[1] += usize::from(case.acted);
        match outcome {
            Outcome::Harmless => row[2] += 1,
            Outcome::Operation(_) => row[3] += 1,
            Outcome::Refused(_) => row[4] += 1,
        }
        if case.acted {
            assert_ne!(
                outcome,
                Outcome::Harmless,
                "{}: the native platform restructured and the check lets it through",
                case.id
            );
        }
        if matches!(outcome, Outcome::Operation(_)) {
            operations += 1;
            assert!(
                case.acted,
                "{}: an operation for a harmless change",
                case.id
            );
        }
    }
    for (run, [total, acted, harmless, ops, refused]) in &by_run {
        println!(
            "{run}: {total} cases, native acted on {acted}, harmless {harmless},              S1 operations {ops}, refused {refused}"
        );
    }
    // add attribute x2, delete, widen, index x2, tabular section, attribute of a tabular section, new object.
    assert_eq!(operations, 9, "operations in the focused runs");
}

#[test]
fn the_159_probes_are_harmless_or_refused_and_the_20_that_acted_are_all_refused() {
    let cases = auto_probes();
    assert_eq!(cases.len(), 159);
    let acted = cases.iter().filter(|case| case.acted).count();
    assert_eq!(acted, 20, "the probes the platform acted on");
    let mut harmless = 0;
    let mut over_refused = 0;
    for case in &cases {
        let outcome = outcome_of(&case.verdict);
        assert!(
            !matches!(outcome, Outcome::Operation(_)),
            "{}: a property change became an operation of S1",
            case.id
        );
        if case.acted {
            assert!(
                matches!(outcome, Outcome::Refused(_)),
                "{}: acted on natively, {outcome:?} here",
                case.id
            );
        } else if outcome == Outcome::Harmless {
            harmless += 1;
        } else {
            println!("over-refused: {} -> {outcome:?}", case.id);
            over_refused += 1;
        }
    }
    println!("{harmless} harmless, {over_refused} over-refused, {acted} refused as they should");
    assert_eq!(harmless + over_refused + acted, 159);
    // The price of failing closed: what the native platform left alone and
    // the rules do not list. It only goes down when a property is probed
    // and listed.
    assert_eq!(
        over_refused, OVER_REFUSED,
        "over-refusals moved: update the doc"
    );
}

/// Probes the platform applied without acting that the rules still refuse.
const OVER_REFUSED: usize = 11;

#[test]
fn a_refused_case_carries_a_code_and_the_rule_that_made_it() {
    for case in all_focused().into_iter().chain(auto_probes()) {
        let class = s1::classify(&case.verdict);
        for refusal in &class.refusals {
            assert!(!refusal.code.id().is_empty(), "{}", case.id);
            assert_ne!(
                refusal.rule,
                super::RuleId::Unspecified,
                "{}: {}",
                case.id,
                refusal.message()
            );
        }
        for reason in &case.verdict.reasons {
            assert_ne!(reason.rule, super::RuleId::Unspecified, "{}", case.id);
            if reason.class != super::ReasonClass::Unknown {
                assert!(!reason.kind.is_empty(), "{}: {reason:?}", case.id);
            }
        }
    }
}

#[test]
fn the_paths_of_a_reason_are_the_ones_the_summary_prints() {
    // The typed path and the text a person reads are one thing.
    let verdict = compare(
        "Catalog",
        &object("Catalog", "", &attribute(A1, "A", 50, "DontIndex", "")),
        &object("Catalog", "", &attribute(A1, "A", 100, "DontIndex", "")),
    );
    let reason = &verdict.reasons[0];
    let change = tree_diff::Change {
        path: reason.path.clone(),
        op: reason.op.clone().unwrap(),
    };
    assert_eq!(reason.property, change.display_path());
    assert_eq!(
        reason.path_names(),
        [
            "ChildObjects",
            "Attribute",
            "Properties",
            "Type",
            "StringQualifiers",
            "Length"
        ]
    );
}

fn describe(outcome: Outcome) -> String {
    match outcome {
        Outcome::Harmless => "harmless".to_string(),
        Outcome::Operation(name) => format!("operation {name}"),
        Outcome::Refused(code) => format!("refused {}", code.id()),
    }
}

/// Writes the whole matrix as a Markdown table to the file named by
/// `IBCMD_RS_S1_MATRIX_OUT` (the evidence file `s1-matrix.md`); does nothing
/// without it.
#[test]
fn the_matrix_can_be_written_out() {
    let Ok(target) = std::env::var("IBCMD_RS_S1_MATRIX_OUT") else {
        return;
    };
    let mut out = String::from(concat!(
        "# The refusal matrix of S1 (#404)\n\n",
        "Every case of the probe runs of `restructuring-check.md` (6.1, 6.2) through the check and ",
        "`apply_check::s1::classify`. Written by `IBCMD_RS_S1_MATRIX_OUT=<file> cargo test -p ibcmd-apply-check ",
        "apply_check::s1_matrix_tests::the_matrix_can_be_written_out`.\n\n",
        "| case | platform acted | S1 | reasons (rule) |\n|---|---|---|---|\n",
    ));
    for case in all_focused().into_iter().chain(auto_probes()) {
        let rules = case
            .verdict
            .reasons
            .iter()
            .map(|reason| reason.rule.id())
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            case.id,
            if case.acted { "yes" } else { "no" },
            describe(outcome_of(&case.verdict)),
            if rules.is_empty() { "-" } else { &rules },
        ));
    }
    std::fs::write(target, out).unwrap();
}
