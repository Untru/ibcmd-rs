use anyhow::{Result, ensure};
use ibcmd_postgres::{PgConnection, PgTarget};
use std::env;

#[test]
#[ignore = "requires an explicit disposable PostgreSQL database and verified TLS"]
fn streams_rows_and_recovers_from_aborted_binary_copy() -> Result<()> {
    let target = PgTarget::new(
        &env::var("IBCMD_RS_PG_SERVER")?,
        &env::var("IBCMD_RS_PG_DATABASE")?,
        Some(&env::var("IBCMD_RS_PG_USER")?),
        env::var("IBCMD_RS_PG_PASSWORD").ok().as_deref(),
    )?;
    ensure!(target.verify_full);
    let mut connection = PgConnection::connect(&target)?;
    let mut tls = false;
    connection.query(
        "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()",
        &mut |row| {
            tls = row[0].as_deref() == Some(b"t");
            Ok(())
        },
    )?;
    ensure!(tls);
    let mut seen = 0;
    connection.query("SELECT generate_series(1, 10000)", &mut |row| {
        seen += 1;
        ensure!(row[0].as_deref() == Some(seen.to_string().as_bytes()));
        Ok(())
    })?;
    ensure!(seen == 10000);
    connection.query(
        "CREATE TEMP TABLE bulk (name text UNIQUE, value bytea)",
        &mut |_| Ok(()),
    )?;
    let command = "COPY bulk FROM STDIN BINARY";
    let name = "тест'\\";
    connection.copy_binary(command, |copy| {
        copy.row(&[Some(name.as_bytes()), Some(&[0, 255])])?;
        copy.row(&[Some(b""), None])
    })?;
    let failure = connection.copy_binary(command, |copy| {
        copy.row(&[Some(b"discarded"), Some(b"")])?;
        anyhow::bail!("producer cancelled")
    });
    ensure!(failure.unwrap_err().to_string() == "producer cancelled");
    let duplicate =
        connection.copy_binary(command, |copy| copy.row(&[Some(name.as_bytes()), None]));
    ensure!(duplicate.unwrap_err().to_string().contains("23505"));
    let unknown = connection.copy_binary("COPY missing_copy_table FROM STDIN BINARY", |_| {
        anyhow::bail!("server should reject COPY before asking for rows")
    });
    ensure!(unknown.unwrap_err().to_string().contains("42P01"));
    let mut contents = Vec::new();
    connection.query(
        "SELECT name, encode(value, 'hex') FROM bulk ORDER BY name",
        &mut |row| {
            contents.push(row);
            Ok(())
        },
    )?;
    ensure!(
        contents
            == [
                vec![Some(Vec::new()), None],
                vec![Some(name.as_bytes().to_vec()), Some(b"00ff".to_vec())]
            ]
    );
    Ok(())
}
