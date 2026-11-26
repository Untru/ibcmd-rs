//! Research probe: `verify <db>` checks every configuration table's index
//! against its records; `copy-table <from-db> <TABLE> <to-db> <TABLE>`
//! replaces a table of a closed infobase with the rows of another.

use std::path::Path;

use ibcmd_filedb::{CONFIG_TABLES, FileDb, replace_config_table};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("verify") if args.len() == 2 => verify(Path::new(&args[1])),
        Some("copy-table") if args.len() == 5 => copy(&args[1], &args[2], &args[3], &args[4]),
        Some("reindex") if args.len() == 3 => {
            ibcmd_filedb::rebuild_config_index_in_place(Path::new(&args[1]), &args[2])
                .map_err(Into::into)
        }
        _ => Err("usage: verify <db> | copy-table <from-db> <TABLE> <to-db> <TABLE>".into()),
    };
    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn verify(db: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut db = FileDb::open(db)?;
    println!(
        "locale {} page {} pages {}",
        db.locale(),
        db.page_size(),
        db.page_count()
    );
    for table in CONFIG_TABLES {
        let rows = db.config_rows(table)?;
        let bytes: u64 = rows.iter().map(|r| r.binary.len() as u64).sum();
        let keys = db.verify_index(table)?;
        println!(
            "{table}: rows {} bytes {bytes} index keys {keys} OK",
            rows.len()
        );
    }
    Ok(())
}

fn copy(
    from: &str,
    from_table: &str,
    to: &str,
    to_table: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let rows = FileDb::open(Path::new(from))?.config_rows(from_table)?;
    let report = replace_config_table(Path::new(to), to_table, &rows)?;
    println!("{report:?}");
    Ok(())
}
