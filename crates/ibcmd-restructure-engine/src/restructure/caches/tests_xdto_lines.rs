//! The property lines of the attributes in the XDTO model, made of the descriptors, against the
//! БСП corpus (every catalog and document, their tabular sections) and the native cases.

use crate::metadata_model::brace::parse_row;
use crate::restructure::caches::facts::ObjectFacts;
use crate::restructure::caches::members::{Member, Members, catalog_attribute_nullable};
use crate::restructure::caches::names_tables::NamesTables;
use crate::restructure::caches::plan::{catalog_shape, document_shape};
use crate::restructure::caches::root::{class_of_kind, collection_of, collections};
use crate::restructure::caches::tests_corpus::{ROOT_ROW, Snap, lab_snap, row_name};
use crate::restructure::caches::xdto_types::{
    Family, RefNames, XdtoModel, attribute_property, document_standard_properties,
    section_property_line, standard_properties,
};

/// Properties the platform adds to some catalogs beyond the attributes: for predefined items
/// (`ОтредактированныеПредопределенныеРеквизиты`), for data separation
/// (`ОбластьДанныхВспомогательныеДанные`) and for a multilingual description or comment (`...Язык1`,
/// `...Язык2`). They are not made of the descriptor, so a comparison leaves them out.
fn is_service_line(line: &str) -> bool {
    let Some(start) = line.find("name=\"") else {
        return false;
    };
    let rest = &line[start + 6..];
    let name = &rest[..rest.find('"').unwrap_or(rest.len())];
    let language_variant = name.rfind("Язык").is_some_and(|at| {
        name[at + "Язык".len()..]
            .chars()
            .all(|c| c.is_ascii_digit())
            && at > 0
    });
    name == "ОтредактированныеПредопределенныеРеквизиты"
        || name == "ОбластьДанныхВспомогательныеДанные"
        || language_variant
}

#[derive(Default, Debug)]
struct Report {
    objects: usize,
    lines: usize,
    /// Attributes of a defined or characteristic type (not made: refused).
    refused: usize,
    service: usize,
    problems: Vec<String>,
}

fn first_difference(ours: &[String], native: &[String]) -> String {
    let first = ours
        .iter()
        .zip(native.iter())
        .position(|(a, b)| a != b)
        .unwrap_or(ours.len().min(native.len()));
    format!(
        "line {first}\n    ours:   {:?}\n    native: {:?}",
        ours.get(first),
        native.get(first)
    )
}

/// The lines of the attributes of `members`, or `None` when one is of a type not made.
fn attribute_lines(
    members: &[Member],
    nullable: &dyn Fn(&Member) -> bool,
    refs: &RefNames,
    report: &mut Report,
) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut refused = false;
    for member in members {
        match attribute_property(&member.name, &member.pattern, nullable(member), refs) {
            Ok(line) => out.push(line),
            Err(error) => {
                assert!(
                    error.to_string().contains("neither a platform type"),
                    "{}: {error:#}",
                    member.name
                );
                report.refused += 1;
                refused = true;
            }
        }
    }
    (!refused).then_some(out)
}

fn check_snapshot(snap: &Snap) -> Report {
    let model = XdtoModel::parse(&snap.params(row_name("ea13a2c9")).unwrap()).unwrap();
    let refs =
        RefNames::build(&NamesTables::parse(&snap.params(row_name("a07b62f0")).unwrap()).unwrap());
    let root = parse_row(&snap.config(ROOT_ROW).unwrap()).unwrap();
    let all = collections(&root);
    let mut report = Report::default();
    for kind in ["Catalog", "Document"] {
        let class = class_of_kind(kind).unwrap();
        let family = if kind == "Catalog" {
            Family::Catalog
        } else {
            Family::Document
        };
        let (object_type, row_type) = if kind == "Catalog" {
            ("CatalogObject", "CatalogTabularSectionRow")
        } else {
            ("DocumentObject", "DocumentTabularSectionRow")
        };
        for uuid in &collection_of(&all, class).unwrap().objects {
            let row = parse_row(&snap.config(uuid).unwrap()).unwrap();
            let facts = ObjectFacts::parse(kind, &row).unwrap();
            let members = Members::parse(kind, &row).unwrap();
            let shape = (kind == "Catalog").then(|| catalog_shape(&facts).unwrap());
            let mut expected = match shape {
                Some(shape) => standard_properties(&facts.name, shape),
                None => document_standard_properties(&facts.name, document_shape(&facts).unwrap()),
            };
            let nullable = |member: &Member| {
                shape.is_some_and(|shape| {
                    catalog_attribute_nullable(
                        shape.hierarchical,
                        shape.hierarchy_type,
                        member.usage,
                    )
                })
            };
            let attributes = attribute_lines(&members.attributes, &nullable, &refs, &mut report);
            if let Some(lines) = attributes {
                expected.extend(lines);
                for section in &members.sections {
                    expected.push(section_property_line(family, &facts.name, &section.name));
                }
                let native: Vec<String> = model
                    .property_lines(&format!("{object_type}.{}", facts.name))
                    .unwrap_or_default();
                let (service, native): (Vec<String>, Vec<String>) = native
                    .into_iter()
                    .partition(|line| is_service_line(line) && !expected.contains(line));
                report.service += service.len();
                if native != expected {
                    report.problems.push(format!(
                        "{kind} {}: {}",
                        facts.name,
                        first_difference(&expected, &native)
                    ));
                }
                report.lines += expected.len();
            }
            for section in &members.sections {
                let lines = attribute_lines(&section.attributes, &|_| false, &refs, &mut report);
                let Some(expected) = lines else { continue };
                let native = model
                    .property_lines(&format!("{row_type}.{}.{}", facts.name, section.name))
                    .unwrap_or_default();
                if native != expected {
                    report.problems.push(format!(
                        "{kind} {}.{}: {}",
                        facts.name,
                        section.name,
                        first_difference(&expected, &native)
                    ));
                }
                report.lines += expected.len();
            }
            report.objects += 1;
        }
    }
    report
}

#[test]
fn the_xdto_lines_of_the_attributes_are_made_of_the_descriptors() {
    for name in ["pristine", "c2", "m", "t1_nat", "d_after"] {
        let snap = lab_snap!(name);
        let report = check_snapshot(&snap);
        eprintln!(
            "{name}: {} objects, {} property lines equal, {} attributes of a defined type left out, {} service lines left out, {} differences",
            report.objects,
            report.lines,
            report.refused,
            report.service,
            report.problems.len()
        );
        for problem in report.problems.iter().take(20) {
            eprintln!("  {problem}");
        }
        assert!(report.problems.is_empty(), "{name}");
        assert!(report.objects >= 139);
    }
}

#[test]
fn an_attribute_of_a_defined_type_or_an_unknown_pattern_is_refused() {
    use crate::metadata_model::brace::Brace;
    let refs = RefNames::default();
    let pattern = |item: Brace| Brace::List(vec![Brace::str("Pattern"), item]);
    let defined = pattern(Brace::List(vec![
        Brace::str("#"),
        Brace::atom("09e14b81-e36d-4ff8-b21b-c41172266147"),
    ]));
    let error = attribute_property("Цена", &defined, false, &refs).unwrap_err();
    assert!(
        error.to_string().contains("neither a platform type"),
        "{error:#}"
    );
    let unknown = pattern(Brace::List(vec![Brace::str("X")]));
    assert!(attribute_property("Икс", &unknown, false, &refs).is_err());
    let empty = Brace::List(vec![Brace::str("Pattern")]);
    assert!(attribute_property("Пусто", &empty, false, &refs).is_err());
}

#[test]
fn every_primitive_type_has_its_xdto_line() {
    use crate::metadata_model::brace::Brace;
    let refs = RefNames::default();
    let pattern = |tag: &str| {
        Brace::List(vec![
            Brace::str("Pattern"),
            Brace::List(vec![Brace::str(tag)]),
        ])
    };
    let line = |tag: &str, nullable: bool| {
        attribute_property("А", &pattern(tag), nullable, &refs).unwrap()
    };
    assert_eq!(
        line("B", false),
        "\t\t\t<property name=\"А\" type=\"xs:boolean\"/>\r\n"
    );
    assert_eq!(
        line("S", true),
        "\t\t\t<property name=\"А\" type=\"xs:string\" lowerBound=\"0\"/>\r\n"
    );
    assert_eq!(
        line("N", false),
        "\t\t\t<property name=\"А\" type=\"xs:decimal\"/>\r\n"
    );
    assert_eq!(
        line("D", false),
        "\t\t\t<property name=\"А\" type=\"xs:dateTime\"/>\r\n"
    );
}
