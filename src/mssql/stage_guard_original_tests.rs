use super::*;
use crate::mssql::source_original_consumers_tests::{Fixture, OBJECT, metadata};

fn comparer(f: &Fixture, source: &MetadataSourceContext, path: &str) -> TreeComparer {
    let manifest = source.scan_sources(&[]).unwrap();
    let member = manifest
        .files
        .iter()
        .find(|file| file.path == path)
        .unwrap();
    let mut comparer = TreeComparer::new(
        &f.top.join("export"),
        &f.root,
        vec![(path.into(), Some(member.sha256.clone()))],
        &[],
    );
    comparer.original_source = Some(source.clone());
    comparer
}

#[test]
fn original_guard_compares_exact_hash_semantic_xml_and_actual_title_change() {
    for version in ["2.20", "2.21"] {
        let f = Fixture::new();
        let xml = metadata(
            "Catalog",
            "A",
            OBJECT,
            version,
            "<Comment>original</Comment>",
        );
        f.write("Catalogs/A.xml", &xml);
        let source = f.context();
        let exact = comparer(&f, &source, "Catalogs/A.xml");
        exact
            .accept(&f.top.join("export/Catalogs/A.xml"), xml.as_bytes())
            .unwrap();
        let result = exact.finish().unwrap();
        assert_eq!(
            (result.compared, result.identical, result.differences.len()),
            (1, 1, 0)
        );
        let formatted = xml.replace("><", ">\r\n  <");
        let semantic = comparer(&f, &source, "Catalogs/A.xml");
        semantic
            .accept(&f.top.join("export/Catalogs/A.xml"), formatted.as_bytes())
            .unwrap();
        assert_eq!(semantic.finish().unwrap().identical, 1);
        let edited = comparer(&f, &source, "Catalogs/A.xml");
        edited
            .accept(
                &f.top.join("export/Catalogs/A.xml"),
                xml.replace("original", "edited").as_bytes(),
            )
            .unwrap();
        let result = edited.finish().unwrap();
        assert_eq!(result.identical, 0);
        assert_eq!(result.differences.len(), 1);
        assert!(
            matches!(&result.differences[0].difference, Difference::Changed { leaves } if !leaves.is_empty())
        );
        source.require_original_unchanged().unwrap();
    }
}

#[test]
fn original_guard_never_falls_back_to_foreign_context_path() {
    let f = Fixture::new();
    let xml = metadata("Catalog", "A", OBJECT, "2.20", "");
    f.write("Catalogs/A.xml", &xml);
    let source = f.context();
    let foreign = Fixture::new();
    foreign.write("Catalogs/A.xml", &xml);
    let mut guard = TreeComparer::new(
        &f.top.join("export"),
        &f.root,
        vec![("Catalogs/A.xml".into(), None)],
        &[],
    );
    guard.original_source = Some(foreign.context());
    assert!(
        guard
            .accept(&f.top.join("export/Catalogs/A.xml"), xml.as_bytes())
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn original_guard_uses_same_immutable_xml_then_publication_refuses_path_drift() {
    let f = Fixture::new();
    let xml = metadata("Catalog", "A", OBJECT, "2.20", "");
    let path = f.write("Catalogs/A.xml", &xml);
    let source = f.context();
    let original = source.read_source(&path).unwrap();
    let guard = comparer(&f, &source, "Catalogs/A.xml");
    fs::write(&path, xml.replace("<Name>A", "<Name>B")).unwrap();
    guard
        .accept(
            &f.top.join("export/Catalogs/A.xml"),
            xml.replace("><", ">\n<").as_bytes(),
        )
        .unwrap();
    assert_eq!(guard.finish().unwrap().identical, 1);
    assert_eq!(&*source.read_source(&path).unwrap(), &*original);
    assert!(source.require_original_unchanged().is_err());
}
