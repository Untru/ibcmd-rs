use super::*;
use crate::mssql_dump::fetch::fetch_row_headers;
use crate::sql::{SqlBackend, SqlClient, SqlExec, SqlOptions};
use std::collections::{BTreeMap, BTreeSet};

/// A quoted T-SQL string, as the expression quotes it.
fn quote(value: &str) -> String {
    format!("N'{}'", value.replace('\'', "''"))
}

type Rows = BTreeMap<(String, i64), (i64, Vec<u8>)>;

fn stored_rows(client: &dyn SqlClient, table: &str) -> anyhow::Result<Rows> {
    let mut rows = Rows::new();
    for row in client.query_rows(
        &format!(
            "SELECT FileName, PartNo, DataSize, HASHBYTES('SHA2_256', BinaryData) FROM {table}"
        ),
        &[],
    )? {
        let key = (row.text(0)?.to_owned(), row.i64(1)?);
        assert!(
            rows.insert(key.clone(), (row.i64(2)?, row.binary(3)?.to_vec()))
                .is_none(),
            "{key:?} is stored twice"
        );
    }
    Ok(rows)
}

#[test]
#[ignore = "reads a lab database: set IBCMD_RS_DYNGEN_DB"]
fn the_expression_publishes_the_rows_the_overlay_describes() -> anyhow::Result<()> {
    let Some(database) = std::env::var_os("IBCMD_RS_DYNGEN_DB") else {
        return Ok(());
    };
    let database = database.to_string_lossy().into_owned();
    let main = std::env::var_os("IBCMD_RS_DYNGEN_MAIN").is_some();
    let sql = SqlExec::from_options(SqlOptions::integrated("localhost", None))?;
    let SqlBackend::Client(client) = sql.backend() else {
        anyhow::bail!("the built-in client is expected");
    };
    let config = format!("[{}].dbo.[Config]", database.replace(']', "]]"));
    let saved = format!("[{}].dbo.[ConfigSave]", database.replace(']', "]]"));

    let stored = stored_rows(client, &config)?;
    let staged = stored_rows(client, &saved)?;

    // What the export installs, exactly as `dump_table_rows_streamed` asks for it.
    let _view = StorageViewScope::begin(&database);
    let headers = fetch_row_headers(&sql, &database, "Config", &BTreeSet::new())?;
    let listed = crate::mssql_dump::install_storage_overlay(
        &sql,
        &database,
        "Config",
        &BTreeSet::new(),
        headers,
        main,
    )?;
    let overlay =
        storage_generation_overlay_for(&database, "Config").expect("an overlay is installed");

    // What the overlay says is published.
    let mut expected = Rows::new();
    for ((name, part), value) in &stored {
        if let Some(published) = overlay.published_name(name) {
            expected.insert((published.to_owned(), *part), value.clone());
        } else if !is_dynamic_generation_alias(name) && !overlay.hides(name) {
            expected.insert((name.clone(), *part), value.clone());
        }
    }
    if let Some(names) = overlay.staged_names() {
        for ((name, part), value) in &staged {
            assert!(names.contains(name), "{name} is staged but not listed");
            expected.insert((name.clone(), *part), value.clone());
        }
    }

    // What the server publishes.
    let expression = crate::mssql_dump::qualified_storage_table(&database, "Config");
    let published = stored_rows(client, &expression)?;
    eprintln!(
        "{database}: {} stored rows, {} staged rows, {} aliases published, {} published rows, {} listed, expression of {} characters",
        stored.len(),
        staged.len(),
        overlay.renames().len(),
        published.len(),
        listed.len(),
        expression.len()
    );
    assert_eq!(published.len(), expected.len());
    assert!(published == expected, "the server and the overlay disagree");

    // The same rows through a selection (#409 F-15): a few aliased names, plain
    // names, and a name that does not exist.
    if !main {
        let mut picked = BTreeSet::new();
        for published in overlay.renames().values().take(6) {
            picked.insert(published.clone());
        }
        for (name, _) in stored
            .keys()
            .filter(|(name, _)| !is_dynamic_generation_alias(name))
            .take(6)
        {
            picked.insert(name.clone());
        }
        picked.insert("00000000-0000-0000-0000-000000000000.0".to_owned());
        let read = |selection: Selection<'_>, filter: &str| -> anyhow::Result<Rows> {
            let expression =
                crate::mssql_dump::qualified_storage_table_for(&database, "Config", selection);
            stored_rows(
                client,
                &format!("(SELECT * FROM {expression} WHERE {filter}) sel"),
            )
        };
        let values = picked
            .iter()
            .map(|name| quote(name))
            .collect::<Vec<_>>()
            .join(", ");
        let filter = format!("FileName IN ({values})");
        let by_names = read(Selection::Names(&picked), &filter)?;
        let unbounded = read(Selection::All, &filter)?;
        assert!(
            by_names == unbounded,
            "a selection changed the rows it keeps"
        );
        assert!(!by_names.is_empty());
        let owners = picked
            .iter()
            .filter(|name| !name.contains('.'))
            .cloned()
            .collect::<BTreeSet<_>>();
        if !owners.is_empty() {
            let filter = owners
                .iter()
                .map(|owner| {
                    format!(
                        "FileName = {} OR FileName LIKE N'{}.%'",
                        quote(owner),
                        owner.replace('\'', "''")
                    )
                })
                .collect::<Vec<_>>()
                .join(" OR ");
            let by_owners = read(Selection::Owners(&owners), &filter)?;
            let all = read(Selection::All, &filter)?;
            assert!(
                by_owners == all,
                "an owner selection changed the rows it keeps"
            );
        }
        eprintln!(
            "{database}: {} rows through a selection of {} names and {} owners",
            by_names.len(),
            picked.len(),
            owners.len()
        );
    }

    // What the export lists: the same names, parts and sizes.
    let listed = listed
        .iter()
        .map(|header| {
            (
                (header.file_name.clone(), i64::from(header.part_no)),
                header.data_size,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let sizes = expected
        .iter()
        .map(|(key, (size, _))| (key.clone(), *size))
        .collect::<BTreeMap<_, _>>();
    assert!(
        listed == sizes,
        "the headers and the published rows disagree"
    );
    Ok(())
}
