//! Targeted exporter for ExternalDataSource descriptors and their child files.
//!
//! The legacy descriptor path has no owner chain for EDS children, and the
//! model exporter is not part of `cf export`. This index carries only the EDS
//! names and generated types the child writers need; every other kind keeps
//! its existing path.

use std::collections::HashMap;
use std::path::PathBuf;

use ibcmd_metadata_model::metadata_model::brace::parse_row;
use ibcmd_metadata_model::metadata_model::export::{ExportContext, NameIndex, write_document};
use ibcmd_metadata_model::metadata_model::external_data_source;
use ibcmd_metadata_model::metadata_model::external_data_source_children;
use ibcmd_metadata_model::metadata_model::objects::parts::Compat;

use super::{ExtractedMetadataSourceXml, MetadataTextRow};

pub(crate) struct EdsExport {
    names: NameIndex,
    kinds: HashMap<String, &'static str>,
    version: String,
}

pub(crate) fn build(rows: &[MetadataTextRow], version: &str) -> Option<EdsExport> {
    let by_name = rows
        .iter()
        .map(|row| (row.file_name.as_str(), row))
        .collect::<HashMap<_, _>>();
    let mut names = NameIndex::default();
    let mut kinds = HashMap::new();
    let mut roots = Vec::new();
    for row in rows {
        if external_data_source::source_family(&row.text, &row.file_name).is_none() {
            continue;
        }
        let tree = parse_row(row.text.as_bytes()).ok()?;
        let root = external_data_source::names(&tree).ok()?;
        names.add(&root);
        kinds.insert(row.file_name.clone(), "ExternalDataSource");
        roots.push((row.file_name.clone(), root.full_name));
    }
    if roots.is_empty() {
        return None;
    }
    for (file_name, root_full) in &roots {
        let text = &by_name.get(file_name.as_str())?.text;
        let (cubes, tables) = external_data_source_children::root_children(text).ok()?;
        for uuid in tables {
            add_child(&mut names, &mut kinds, &by_name, &uuid, root_full)?;
        }
        for uuid in cubes {
            let (_, cube_full) = add_child(&mut names, &mut kinds, &by_name, &uuid, root_full)?;
            let cube_text = &by_name.get(uuid.as_str())?.text;
            let dimensions = external_data_source_children::dimension_table_refs(cube_text).ok()?;
            for dimension in dimensions {
                add_child(&mut names, &mut kinds, &by_name, &dimension, &cube_full)?;
            }
        }
    }
    Some(EdsExport {
        names,
        kinds,
        version: version.to_string(),
    })
}

fn add_child(
    names: &mut NameIndex,
    kinds: &mut HashMap<String, &'static str>,
    by_name: &HashMap<&str, &MetadataTextRow>,
    uuid: &str,
    owner: &str,
) -> Option<(String, String)> {
    let row = by_name.get(uuid)?;
    let (kind, _) = external_data_source_children::family(&row.text, uuid)?;
    let tree = parse_row(row.text.as_bytes()).ok()?;
    let object = external_data_source_children::owned_names(kind, &tree, owner).ok()?;
    names.add(&object);
    kinds.insert(uuid.to_string(), kind);
    Some((kind.to_string(), object.full_name))
}

impl EdsExport {
    pub(crate) fn export_row(
        &self,
        file_name: &str,
        row: &MetadataTextRow,
    ) -> Option<Result<ExtractedMetadataSourceXml, String>> {
        let kind = *self.kinds.get(file_name)?;
        let tree = match parse_row(row.text.as_bytes()) {
            Ok(tree) => tree,
            Err(error) => return Some(Err(format!("{error:#}"))),
        };
        let context = ExportContext {
            names: self.names.clone(),
            version: self.version.clone(),
            compat: Compat(8, 3, 27),
        };
        let element = if kind == "ExternalDataSource" {
            external_data_source::decode(&tree, &self.names)
        } else {
            external_data_source_children::decode_row(kind, &tree, &context)
        };
        Some(match element {
            Ok(element) => {
                let xml = write_document(&element, &self.version);
                let full = match self.names.name(file_name) {
                    Some(full) => full,
                    None => return Some(Err(format!("no EDS name for {file_name}"))),
                };
                match path_from_full(full) {
                    Some(relative_path) => Ok(ExtractedMetadataSourceXml {
                        relative_path,
                        xml: xml.into_bytes(),
                    }),
                    None => Err(format!("unexpected EDS full name {full}")),
                }
            }
            Err(error) => Err(format!("{error:#}")),
        })
    }
}

fn path_from_full(full: &str) -> Option<PathBuf> {
    let parts = full.split('.').collect::<Vec<_>>();
    if parts.len() < 2 || parts.len() % 2 != 0 {
        return None;
    }
    let mut path = PathBuf::new();
    for pair in parts.chunks(2) {
        let folder = match pair[0] {
            "ExternalDataSource" => "ExternalDataSources",
            "Cube" => "Cubes",
            "Table" => "Tables",
            "DimensionTable" => "DimensionTables",
            _ => return None,
        };
        path.push(folder);
        path.push(pair[1]);
    }
    Some(path.with_extension("xml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const ROOT: &str = include_str!("../../tests/fixtures/eds-matrix/root.txt");
    const CUBE: &str = include_str!("../../tests/fixtures/eds-matrix/cube.txt");
    const TABLE: &str = include_str!("../../tests/fixtures/eds-matrix/table.txt");
    const DIMENSION: &str = include_str!("../../tests/fixtures/eds-matrix/dimension.txt");

    const CORPUS_ROOT: &str = include_str!("../../tests/fixtures/eds-corpus2/root.txt");
    const CORPUS_CUBE: &str = include_str!("../../tests/fixtures/eds-corpus2/cube.txt");
    const CORPUS_TABLE: &str = include_str!("../../tests/fixtures/eds-corpus2/table.txt");
    const CORPUS_DIMENSION: &str = include_str!("../../tests/fixtures/eds-corpus2/dimension.txt");

    fn row(file_name: &str, text: &str) -> MetadataTextRow {
        super::super::metadata::metadata_text_row_from_text(file_name, text.to_string())
            .unwrap_or_else(|| panic!("row {file_name} was not classified"))
    }

    fn assert_case(directory: &str, rows: Vec<MetadataTextRow>, fixtures: &[(&str, &str, &str)]) {
        let export = build(&rows, "2.20").expect("EDS index");
        for (uuid, fixture, expected_path) in fixtures {
            let row = rows.iter().find(|row| row.file_name == *uuid).unwrap();
            let out = export
                .export_row(uuid, row)
                .unwrap_or_else(|| panic!("{uuid} was not exported"))
                .unwrap_or_else(|error| panic!("{uuid}: {error}"));
            let expected = std::fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures")
                    .join(directory)
                    .join(fixture),
            )
            .unwrap();
            assert_eq!(out.xml, expected, "{uuid} differs from the platform");
            assert_eq!(out.relative_path, PathBuf::from(expected_path), "{uuid}");
        }
    }

    #[test]
    fn matrix_eds_tree_is_byte_exact_and_nested() {
        assert_case(
            "eds-matrix",
            vec![
                row("35f0f3eb-6bc6-455e-8200-2a8c1664122b", ROOT),
                row("959bb0a7-39af-49f7-97cd-b9702bcf6cf0", CUBE),
                row("87f7ae12-3120-4257-92a1-11beacd7b790", TABLE),
                row("79581bb0-b72d-43bb-b988-8f45c602139a", DIMENSION),
            ],
            &[
                (
                    "35f0f3eb-6bc6-455e-8200-2a8c1664122b",
                    "root.xml",
                    "ExternalDataSources/ВнешнийИсточникДанных1.xml",
                ),
                (
                    "959bb0a7-39af-49f7-97cd-b9702bcf6cf0",
                    "cube.xml",
                    "ExternalDataSources/ВнешнийИсточникДанных1/Cubes/Куб1.xml",
                ),
                (
                    "87f7ae12-3120-4257-92a1-11beacd7b790",
                    "table.xml",
                    "ExternalDataSources/ВнешнийИсточникДанных1/Tables/Таблица1.xml",
                ),
                (
                    "79581bb0-b72d-43bb-b988-8f45c602139a",
                    "dimension.xml",
                    "ExternalDataSources/ВнешнийИсточникДанных1/Cubes/Куб1/DimensionTables/ТаблицаИзмерения1.xml",
                ),
            ],
        );
    }

    #[test]
    fn stepwise_corpus_is_byte_exact_and_nested() {
        assert_case(
            "eds-corpus2",
            vec![
                row("7a9885f0-eba3-4020-b10e-4f2784b179cb", CORPUS_ROOT),
                row("2a643a64-766d-4f18-a3a6-718bee907cef", CORPUS_CUBE),
                row("ef3f4e83-a764-4ae6-9877-d962e074dcf0", CORPUS_TABLE),
                row("59945b31-4421-4845-b868-8eefd291dc8c", CORPUS_DIMENSION),
            ],
            &[
                (
                    "7a9885f0-eba3-4020-b10e-4f2784b179cb",
                    "root.xml",
                    "ExternalDataSources/ВнешнийИсточникДанных1.xml",
                ),
                (
                    "2a643a64-766d-4f18-a3a6-718bee907cef",
                    "cube.xml",
                    "ExternalDataSources/ВнешнийИсточникДанных1/Cubes/Куб1.xml",
                ),
                (
                    "ef3f4e83-a764-4ae6-9877-d962e074dcf0",
                    "table.xml",
                    "ExternalDataSources/ВнешнийИсточникДанных1/Tables/Таблица1.xml",
                ),
                (
                    "59945b31-4421-4845-b868-8eefd291dc8c",
                    "dimension.xml",
                    "ExternalDataSources/ВнешнийИсточникДанных1/Cubes/Куб1/DimensionTables/ТаблицаИзмерения1.xml",
                ),
            ],
        );
    }
}
