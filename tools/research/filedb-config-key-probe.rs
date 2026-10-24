// TSV bridge to the public API. Key generation stays in ibcmd-filedb.
use ibcmd_filedb::index::{Collation, config_key};
use std::io::{self, BufRead, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let line = line?;
        let mut fields = line.splitn(3, '\t');
        let locale = fields.next().ok_or("missing locale")?;
        let part: u32 = fields.next().ok_or("missing part")?.parse()?;
        let name = fields.next().ok_or("missing name")?;
        let key = Collation::for_locale(locale)
            .and_then(|collation| config_key(collation, name, part).ok());
        match key {
            Some(key) => {
                write!(out, "OK\t")?;
                for byte in key {
                    write!(out, "{byte:02x}")?;
                }
                writeln!(out)?;
            }
            None => writeln!(out, "REFUSED")?,
        }
    }
    Ok(())
}
