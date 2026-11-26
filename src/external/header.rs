//! Main row of an external object (`{1,{<main>},1,{<class>,{1,{4,…},N,…}}}`)
//! and its rewrite into the `DataProcessor` (v17) / `Report` (v19) row that
//! the configuration pipeline decodes.
//!
//! Header field mapping, established on 81 .epf and 22 .erf built by
//! 8.3.27.2214 against the same objects' configuration rows:
//!
//! | external v4 | DataProcessor v17 | Report v19 |
//! |---|---|---|
//! | 1 TypeId, 2 ValueId, 3 names | 1, 2, 3 | 1, 2, 3 |
//! | 4 DefaultForm | 4 | 4 |
//! | DP 5 `""` / report 10 `""` | — | — |
//! | DP 6 AuxiliaryForm | 9 | |
//! | report 5, 6 | | 5, 6 |
//! | report 7, 8, 9 | | 8, 9, 10 |
//! | report 11, 12 | | 14, 17 |
//!
//! The internal-only fields (standard commands, help in contents, manager
//! type ids, extended presentation, explanation) are filled neutrally; the
//! properties they produce are dropped again from the external root XML.

use anyhow::{Context, Result, bail};

use super::{ExternalKind, brace, derived_uuid};

const DATA_PROCESSOR_COMMANDS: &str = "{45556acb-826a-4f73-898a-6025fc9536e1,0}";
const REPORT_COMMANDS: &str = "{e7ff38c0-ec3c-47a0-ae90-20c73ca72246,0}";
const FORMS_COLLECTION: &str = "{d5b0e5ed-256d-401c-9c36-f630cafd8a62";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalMain {
    pub kind: ExternalKind,
    /// `root` → main entry name; the XML `uuid` of the external object.
    pub main_uuid: String,
    /// `{1,0,<id>}` of the header: the ContainedObject ObjectId, owner of the
    /// object module `<id>.0` and help `<id>.1`.
    pub object_id: String,
    pub name: String,
    /// Header fields, `[0] == "4"`.
    pub header: Vec<String>,
    pub collections: Vec<String>,
}

/// `Ok(None)` when `text` is not an external object's main row.
pub fn parse_main(main_uuid: &str, text: &str) -> Result<Option<ExternalMain>> {
    let text = brace::strip_bom(text).trim_start();
    let Some((top, _)) = brace::fields(text, 0) else {
        return Ok(None);
    };
    // Decide by the wrapper's class id first: a row naming an external class
    // in any other layout is refused instead of falling back to the
    // configuration path (which would silently mis-export it).
    let Some((at, wrap, kind)) = top.iter().enumerate().find_map(|(at, field)| {
        let (wrap, _) = brace::fields(field, 0)?;
        let kind = wrap
            .first()
            .and_then(|class| ExternalKind::from_class_id(class))?;
        Some((at, wrap, kind))
    }) else {
        return Ok(None);
    };
    if at != 3 || top.len() != 4 || top[0] != "1" || top[2] != "1" {
        bail!(
            "external {} main row has an unsupported layout",
            kind.external_kind()
        );
    }
    let body = wrap.get(1).context("external object wrapper has no body")?;
    let (inner, _) = brace::fields(body, 0).context("external object body is not a brace list")?;
    if inner.len() < 3 || inner[0] != "1" {
        bail!("unexpected external object body layout");
    }
    let (header, _) = brace::fields(inner[1], 0).context("external header is not a brace list")?;
    if header.first() != Some(&"4") {
        bail!(
            "unsupported external header version `{}`",
            header.first().copied().unwrap_or_default()
        );
    }
    let expected = match kind {
        ExternalKind::DataProcessor => 7,
        ExternalKind::Report => 13,
    };
    if header.len() != expected {
        bail!(
            "external {} header has {} fields, expected {expected}",
            kind.external_kind(),
            header.len()
        );
    }
    let count: usize = inner[2].parse().context("external collection count")?;
    let collections: Vec<String> = inner[3..].iter().map(|s| (*s).to_owned()).collect();
    if collections.len() != count {
        bail!(
            "external object declares {count} collections but has {}",
            collections.len()
        );
    }
    let (names, _) = brace::fields(header[3], 0).context("external header names block")?;
    let (named, _) = brace::fields(names.get(1).context("names block body")?, 0)
        .context("external header names tuple")?;
    let (id_tuple, _) =
        brace::fields(named.get(1).context("object id tuple")?, 0).context("external object id")?;
    let object_id = id_tuple
        .get(2)
        .context("external object id value")?
        .to_ascii_lowercase();
    let name = named
        .get(2)
        .context("external object name")?
        .trim_matches('"')
        .replace("\"\"", "\"");
    // The name becomes a folder and a file under the output (and a scratch
    // directory a load clears): only a 1C identifier is taken, never `..`
    // or a path.
    if !is_object_name(&name) {
        bail!("external object name {name:?} is not a 1C identifier");
    }
    Ok(Some(ExternalMain {
        kind,
        main_uuid: main_uuid.to_ascii_lowercase(),
        object_id,
        name,
        header: header.iter().map(|s| (*s).to_owned()).collect(),
        collections,
    }))
}

/// A 1C object name: a letter or `_`, then letters, digits and `_`.
pub(crate) fn is_object_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_alphanumeric() || ch == '_')
}

/// The configuration row (BOM-prefixed) the pipeline decodes as the same
/// object; stored under `main.object_id`.
pub fn internal_row_text(main: &ExternalMain) -> Result<String> {
    let h = &main.header;
    let m1 = derived_uuid(&format!("onecdec-external/manager-type/{}", h[1]));
    let m2 = derived_uuid(&format!("onecdec-external/manager-value/{}", h[2]));
    let mut collections = main.collections.clone();
    let header = match main.kind {
        ExternalKind::DataProcessor => {
            if h[5] != "\"\"" {
                bail!(
                    "external data processor header field 5 is `{}`; only \"\" is evidenced",
                    h[5]
                );
            }
            let at = collections
                .iter()
                .position(|c| c.starts_with(FORMS_COLLECTION))
                .unwrap_or(collections.len());
            collections.insert(at, DATA_PROCESSOR_COMMANDS.to_owned());
            format!(
                "{{17,{},{},\r\n{},{},1,0,{m1},{m2},{},\r\n{{0}},\r\n{{0}}\r\n}}",
                h[1], h[2], h[3], h[4], h[6]
            )
        }
        ExternalKind::Report => {
            if h[10] != "\"\"" {
                bail!(
                    "external report header field 10 is `{}`; only \"\" is evidenced",
                    h[10]
                );
            }
            collections.push(REPORT_COMMANDS.to_owned());
            format!(
                "{{19,{},{},\r\n{},{},{},{},1,{},{},{},0,{m1},{m2},{},\r\n{{0}},\r\n{{0}},{}}}",
                h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[11], h[12]
            )
        }
    };
    Ok(format!(
        "\u{feff}{{1,\r\n{header},{},\r\n{}\r\n}}",
        collections.len(),
        collections.join(",\r\n")
    ))
}

/// Byte offset of `inner` (a slice of `outer`) within `outer`.
fn offset(outer: &str, inner: &str) -> usize {
    inner.as_ptr() as usize - outer.as_ptr() as usize
}

/// The main row of `main` with its header and collections taken from
/// `internal` -- the `DataProcessor`/`Report` row compiled from the tree --
/// laid out as `original`, the base's main row: the inverse of
/// [`internal_row_text`], field for field (a round trip gives `original`
/// back byte for byte).
pub fn external_main_text(main: &ExternalMain, internal: &str, original: &str) -> Result<String> {
    let internal = brace::strip_bom(internal).trim_start();
    let (top, _) = brace::fields(internal, 0).context("internal row is not a brace list")?;
    if top.len() < 3 || top[0] != "1" {
        bail!("unexpected internal row layout");
    }
    let (h, _) = brace::fields(top[1], 0).context("internal header is not a brace list")?;
    let (header, commands) = match main.kind {
        ExternalKind::DataProcessor => {
            if h.len() < 10 || h[0] != "17" {
                bail!("internal data processor header is not v17");
            }
            (
                vec![h[1], h[2], h[3], h[4], "\"\"", h[9]],
                "{45556acb-826a-4f73-898a-6025fc9536e1,",
            )
        }
        ExternalKind::Report => {
            if h.len() < 18 || h[0] != "19" {
                bail!("internal report header is not v19");
            }
            (
                vec![
                    h[1], h[2], h[3], h[4], h[5], h[6], h[8], h[9], h[10], "\"\"", h[14], h[17],
                ],
                "{e7ff38c0-ec3c-47a0-ae90-20c73ca72246,",
            )
        }
    };
    let mut collections = Vec::new();
    for collection in &top[3..] {
        if collection.starts_with(commands) {
            let (fields, _) = brace::fields(collection, 0).context("commands collection")?;
            if fields.get(1) != Some(&"0") {
                bail!("an external {} has no commands", main.kind.external_kind());
            }
            continue;
        }
        collections.push(*collection);
    }

    // The original main row, navigated as `parse_main` does.
    let text = brace::strip_bom(original);
    let base = original.len() - text.len();
    let text_trimmed = text.trim_start();
    let base = base + (text.len() - text_trimmed.len());
    let (outer, _) = brace::fields(text_trimmed, 0).context("main row is not a brace list")?;
    let (wrap, _) = brace::fields(outer.get(3).context("main row wrapper")?, 0)
        .context("main row wrapper is not a brace list")?;
    let (inner, _) = brace::fields(wrap.get(1).context("main row body")?, 0)
        .context("main row body is not a brace list")?;
    let (old_header, _) = brace::fields(inner[1], 0).context("main row header")?;
    if old_header.len() != header.len() + 1 {
        bail!("main row header has {} fields", old_header.len());
    }
    let at = |slice: &str| base + offset(text_trimmed, slice);
    let mut edits = Vec::<(usize, usize, String)>::new();
    for (old, new) in old_header[1..].iter().zip(&header) {
        if old != new {
            edits.push((at(old), old.len(), (*new).to_owned()));
        }
    }
    edits.push((at(inner[2]), inner[2].len(), collections.len().to_string()));
    let old_collections = &inner[3..];
    if let (Some(first), Some(last)) = (old_collections.first(), old_collections.last()) {
        let separator = match old_collections.get(1) {
            Some(second) => &original[at(first) + first.len()..at(second)],
            None => ",\r\n",
        };
        edits.push((
            at(first),
            at(last) + last.len() - at(first),
            collections.join(separator),
        ));
    } else if !collections.is_empty() {
        bail!("main row has no collections to lay new ones out like");
    }
    edits.sort_by(|left, right| right.0.cmp(&left.0));
    let mut out = original.to_owned();
    for (start, len, replacement) in edits {
        out.replace_range(start..start + len, &replacement);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_row_round_trips_through_the_internal_row() {
        for (uuid, text) in [
            ("aaaaaaaa-0000-0000-0000-000000000001", MAIN_DP),
            ("b0509899-141e-4cda-ba24-de8ed9711646", MAIN_REPORT),
        ] {
            let main = parse_main(uuid, text).unwrap().unwrap();
            let internal = internal_row_text(&main).unwrap();
            assert_eq!(external_main_text(&main, &internal, text).unwrap(), text);
        }
    }

    #[test]
    fn a_name_that_is_not_an_identifier_is_refused() {
        for name in ["..", "a\\\\..\\\\..", "x/y", "", "1abc", "a.b"] {
            let text = MAIN_DP.replace("},\"Тест\",", &format!("}},\"{name}\","));
            assert!(
                parse_main("aaaaaaaa-0000-0000-0000-000000000001", &text).is_err(),
                "{name:?}"
            );
        }
        for name in ["Тест", "_x1", "ОбработкаА2"] {
            assert!(is_object_name(name), "{name}");
        }
    }

    const MAIN_DP: &str = "{1,\r\n{aaaaaaaa-0000-0000-0000-000000000001},1,\r\n{c3831ec8-d8d5-4f93-8a22-f9bfae07327f,\r\n{1,\r\n{4,11111111-1111-1111-1111-111111111111,22222222-2222-2222-2222-222222222222,\r\n{0,\r\n{3,\r\n{1,0,bbbbbbbb-0000-0000-0000-000000000002},\"Тест\",\r\n{1,\"ru\",\"Тест\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0}\r\n},cccccccc-0000-0000-0000-000000000003,\"\",00000000-0000-0000-0000-000000000000},4,\r\n{2bcef0d1-0981-11d6-b9b8-0050bae0a95d,0},\r\n{3daea016-69b7-4ed4-9453-127911372fe6,0},\r\n{d5b0e5ed-256d-401c-9c36-f630cafd8a62,1,cccccccc-0000-0000-0000-000000000003},\r\n{ec6bb5e5-b7a8-4d75-bec9-658107a699cf,0}\r\n}\r\n}\r\n}";

    // main row of tests/fixtures/external/test_report/input.erf
    const MAIN_REPORT: &str = "{1,\r\n{b0509899-141e-4cda-ba24-de8ed9711646},1,\r\n{e41aff26-25cf-4bb6-b6c1-3f478a75f374,\r\n{1,\r\n{4,1809b0db-0610-4885-86d0-4fcab351518e,c5c17663-e8dc-4c06-9f35-d58aa343b6d2,\r\n{0,\r\n{3,\r\n{1,0,3ae658fc-eb5f-4e2d-a3b9-8f021f275ff7},\"ТестОтчет\",\r\n{1,\"ru\",\"Тестовый отчет\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0}\r\n},bea0311d-c8f5-4532-a1f4-c4ec5a771802,4e0dde81-2c8d-494f-932d-f06edf1e037d,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,\"\",00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000},4,\r\n{3daea016-69b7-4ed4-9453-127911372fe6,1,4e0dde81-2c8d-494f-932d-f06edf1e037d},\r\n{7e7123e0-29e2-11d6-a3c7-0050bae0a776,0},\r\n{a3b368c0-29e2-11d6-a3c7-0050bae0a776,1,bea0311d-c8f5-4532-a1f4-c4ec5a771802},\r\n{b077d780-29e2-11d6-a3c7-0050bae0a776,0}\r\n}\r\n}\r\n}";

    fn internal_fields(row: &str) -> (Vec<String>, Vec<String>) {
        let (top, _) = brace::fields(brace::strip_bom(row), 0).unwrap();
        let (h, _) = brace::fields(top[1], 0).unwrap();
        (
            h.iter().map(|s| (*s).to_owned()).collect(),
            top.iter().map(|s| (*s).to_owned()).collect(),
        )
    }

    #[test]
    fn parses_external_data_processor_main_row() {
        let m = parse_main("AAAAAAAA-0000-0000-0000-000000000001", MAIN_DP)
            .unwrap()
            .unwrap();
        assert_eq!(m.kind, ExternalKind::DataProcessor);
        assert_eq!(m.main_uuid, "aaaaaaaa-0000-0000-0000-000000000001");
        assert_eq!(m.object_id, "bbbbbbbb-0000-0000-0000-000000000002");
        assert_eq!(m.name, "Тест");
        assert_eq!(m.header.len(), 7);
        assert_eq!(m.collections.len(), 4);
    }

    #[test]
    fn rewrites_into_data_processor_row_the_pipeline_reads() {
        let m = parse_main("aaaaaaaa-0000-0000-0000-000000000001", MAIN_DP)
            .unwrap()
            .unwrap();
        let row = internal_row_text(&m).unwrap();
        assert!(row.starts_with("\u{feff}{1,\r\n{17,11111111-1111-1111-1111-111111111111,22222222-2222-2222-2222-222222222222,"));
        let (h, top) = internal_fields(&row);
        assert_eq!(top[2], "5");
        assert_eq!(h.len(), 12);
        assert_eq!(
            (h[4].as_str(), h[5].as_str(), h[6].as_str()),
            ("cccccccc-0000-0000-0000-000000000003", "1", "0")
        );
        assert_eq!(h[9], "00000000-0000-0000-0000-000000000000");
        assert_eq!((h[10].as_str(), h[11].as_str()), ("{0}", "{0}"));
        assert_eq!(top[5], "{45556acb-826a-4f73-898a-6025fc9536e1,0}");
        assert!(top[6].starts_with("{d5b0e5ed-"));
    }

    #[test]
    fn rewrites_into_report_row_the_pipeline_reads() {
        let m = parse_main("b0509899-141e-4cda-ba24-de8ed9711646", MAIN_REPORT)
            .unwrap()
            .unwrap();
        assert_eq!(m.kind, ExternalKind::Report);
        assert_eq!(m.object_id, "3ae658fc-eb5f-4e2d-a3b9-8f021f275ff7");
        let row = internal_row_text(&m).unwrap();
        let (h, top) = internal_fields(&row);
        assert_eq!(h[0], "19");
        assert_eq!(h.len(), 18);
        assert_eq!(h[4], "bea0311d-c8f5-4532-a1f4-c4ec5a771802");
        assert_eq!(h[5], "4e0dde81-2c8d-494f-932d-f06edf1e037d");
        assert_eq!((h[7].as_str(), h[11].as_str()), ("1", "0"));
        assert_eq!((h[15].as_str(), h[16].as_str()), ("{0}", "{0}"));
        assert_eq!(top[2], "5");
        assert_eq!(top[7], "{e7ff38c0-ec3c-47a0-ae90-20c73ca72246,0}");
    }

    #[test]
    fn configuration_root_object_is_not_external() {
        let text = "{2,\r\n{30ffe4cc-eef2-4371-8b26-046597e37e22},6,\r\n{9cd510cd-abfc-11d4-9434-004095e12fc7,{1}}}";
        assert!(
            parse_main("30ffe4cc-eef2-4371-8b26-046597e37e22", text)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn external_class_in_an_unexpected_layout_fails_closed() {
        let text = MAIN_DP.replacen("},1,\r\n{c3831ec8", "},2,\r\n{c3831ec8", 1);
        assert_ne!(text, MAIN_DP);
        assert!(parse_main("aaaaaaaa-0000-0000-0000-000000000001", &text).is_err());
    }

    #[test]
    fn unknown_filled_blank_field_fails_closed() {
        let text = MAIN_DP.replace(
            ",\"\",00000000-0000-0000-0000-000000000000}",
            ",\"x\",00000000-0000-0000-0000-000000000000}",
        );
        let m = parse_main("aaaaaaaa-0000-0000-0000-000000000001", &text)
            .unwrap()
            .unwrap();
        assert!(internal_row_text(&m).is_err());
    }
}
