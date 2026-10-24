use ibcmd_filedb::{CONFIG_TABLES, FileDb};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("missing database path")?;
    let mut db = FileDb::open(Path::new(&path))?;
    println!(
        "FILE\t{}\t{}\t{}",
        db.locale(),
        db.page_size(),
        db.page_count()
    );
    let tables: Vec<_> = db.tables().cloned().collect();
    for table in tables {
        println!(
            "TABLE\t{}\t{}\t{}\t{}",
            table.name, table.records_page, table.blob_page, table.index_page
        );
        if CONFIG_TABLES.contains(&table.name.as_str()) {
            let rows = db.config_rows(&table.name)?;
            let stored: usize = rows.iter().map(|row| row.binary.len()).sum();
            let maximum = rows.iter().map(|row| row.binary.len()).max().unwrap_or(0);
            println!(
                "ROWS\t{}\t{}\t{}\t{}",
                table.name,
                rows.len(),
                stored,
                maximum
            );
        }
    }
    Ok(())
}
