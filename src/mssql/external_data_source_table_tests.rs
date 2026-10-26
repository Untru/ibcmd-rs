//! Offline regressions for staging an owned external-source Table descriptor.

use std::path::Path;

use anyhow::Result;

use crate::metadata_model::brace::{Brace, parse_row, serialize_row};
use crate::metadata_model::export::{ExportContext, NameIndex, decode_object, write_document};
use crate::metadata_model::external_data_source_children::owned_names;
use crate::metadata_model::objects::parts::Compat;
use crate::module_blob::{
    deflate_raw, inflate_raw, pack_simple_metadata_blob_from_xml_with_source,
};
use crate::source::{SourceFile, SourceKind, SourceManifest};

const NATIVE_TABLE: &[u8] =
    include_bytes!("../../tests/fixtures/eds-branch/external_data_source_table_row.txt");

fn fixture() -> Result<(Brace, ExportContext, String)> {
    let native = parse_row(NATIVE_TABLE)?;
    let owned = owned_names("Table", &native, "ExternalDataSource.CorpusSource")?;
    let mut names = NameIndex::default();
    names.add(&owned);
    let context = ExportContext {
        names,
        version: "2.21".to_owned(),
        compat: Compat(8, 5, 1),
    };
    let object = decode_object("Table", &native, &context)?;
    let xml = write_document(&object, "2.21");
    assert_eq!(native.as_list().unwrap()[1].as_list().unwrap().len(), 52);
    assert!(xml.contains("<TableDataType>NonobjectData</TableDataType>"));
    Ok((native, context, xml))
}

fn patch(base: &Brace, xml: &str) -> Result<Brace> {
    let blob = deflate_raw(&serialize_row(base))?;
    let packed = pack_simple_metadata_blob_from_xml_with_source(&blob, xml.as_bytes(), None)?;
    parse_row(&inflate_raw(&packed.blob)?)
}

#[test]
fn external_data_source_table_is_selected_as_its_own_stage_descriptor() {
    let root = Path::new("/offline-source");
    let paths = [
        "ExternalDataSources/CorpusSource.xml",
        "ExternalDataSources/CorpusSource/Tables/Stocks.xml",
        "ExternalDataSources/CorpusSource/Tables/Stocks/Ext/ObjectModule.bsl",
    ];
    let manifest = SourceManifest {
        root: root.to_path_buf(),
        generated_at_unix: 0,
        files: paths
            .iter()
            .map(|path| SourceFile {
                path: (*path).to_owned(),
                size_bytes: 0,
                sha256: String::new(),
                kind: SourceKind::MetadataXml,
                xml_root: Some("MetaDataObject".to_owned()),
                object_hint: None,
            })
            .collect(),
    };

    assert_eq!(
        super::source_metadata_xmls(&manifest, root),
        vec![root.join(paths[0]), root.join(paths[1]),]
    );
}

#[test]
fn external_data_source_table_admission_rejects_other_owned_paths() {
    for path in [
        "ExternalDataSources/CorpusSource/Stocks.xml",
        "ExternalDataSources/CorpusSource/Tables/Stocks/Field.xml",
        "ExternalDataSources/CorpusSource/Tables/Stocks/Ext/Table.xml",
        "Catalogs/CorpusSource/Tables/Stocks.xml",
    ] {
        assert!(!super::is_stage_metadata_xml(path), "admitted {path}");
    }
}

#[test]
fn external_data_source_table_patch_transfers_object_data_without_changing_other_native_values()
-> Result<()> {
    let (base, context, xml) = fixture()?;
    let changed = xml.replace(
        "<TableDataType>NonobjectData</TableDataType>",
        "<TableDataType>ObjectData</TableDataType>",
    );
    let mut expected = base.clone();
    expected.as_list_mut().unwrap()[1].as_list_mut().unwrap()[20] = Brace::num(1);

    let actual = patch(&base, &changed)?;
    assert_eq!(
        actual, expected,
        "only the TableDataType coordinate may change"
    );
    let exported = write_document(&decode_object("Table", &actual, &context)?, "2.21");
    assert_eq!(exported, changed, "the staged Table must export ObjectData");
    assert_eq!(
        patch(&actual, &changed)?,
        expected,
        "reapplying is idempotent"
    );
    assert_eq!(
        patch(&actual, &xml)?,
        base,
        "NonobjectData restores the base value"
    );
    Ok(())
}

#[test]
fn external_data_source_table_patch_preserves_an_unchanged_native_descriptor() -> Result<()> {
    let (base, _, xml) = fixture()?;
    assert_eq!(patch(&base, &xml)?, base);
    Ok(())
}

#[test]
fn external_data_source_table_patch_rejects_unsupported_or_malformed_xml() -> Result<()> {
    let (base, _, xml) = fixture()?;
    for (before, after) in [
        (
            "<TableDataType>NonobjectData</TableDataType>",
            "<TableDataType>UnknownKind</TableDataType>",
        ),
        (
            "<TableDataType>NonobjectData</TableDataType>",
            "<TableDataType>ObjectData</TableDataType><TableDataType>ObjectData</TableDataType>",
        ),
        ("<TableDataType>NonobjectData</TableDataType>", ""),
        (
            "<TableDataType>NonobjectData</TableDataType>",
            "<TableDataType extra=\"1\">ObjectData</TableDataType>",
        ),
        (
            "<ExpressionInDataSource/>",
            "<ExpressionInDataSource>SELECT 1</ExpressionInDataSource>",
        ),
        (
            "<PresentationField/>",
            "<PresentationField>UnknownField</PresentationField>",
        ),
        (
            "</Properties>",
            "<UnsupportedProperty>true</UnsupportedProperty></Properties>",
        ),
        ("<ChildObjects>", "<ChildObjects><UnsupportedChild/>"),
    ] {
        let changed = xml.replacen(before, after, 1);
        assert_ne!(changed, xml, "the fixture must contain {before}");
        assert!(
            patch(&base, &changed).is_err(),
            "accepted unsupported change {after}"
        );
    }
    Ok(())
}

#[test]
fn external_data_source_table_patch_rejects_unverified_native_profiles() -> Result<()> {
    let (base, _, xml) = fixture()?;
    let mut short = base.clone();
    short.as_list_mut().unwrap()[1].as_list_mut().unwrap().pop();
    let mut extended = base.clone();
    extended.as_list_mut().unwrap()[1]
        .as_list_mut()
        .unwrap()
        .push(Brace::num(0));
    let mut unknown_property = base.clone();
    unknown_property.as_list_mut().unwrap()[1]
        .as_list_mut()
        .unwrap()[40] = Brace::num(1);
    for native in [short, extended, unknown_property] {
        assert!(
            patch(&native, &xml).is_err(),
            "accepted an unverified native Table profile"
        );
    }
    Ok(())
}
