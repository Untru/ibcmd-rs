use std::env;

use anyhow::{Result, ensure};
use ibcmd_sql::sql::{Dbms, ScriptVariables, SqlExec, SqlLogin, SqlParam, SqlTarget, SqlValue};

#[test]
#[ignore = "requires a disposable PostgreSQL infobase created by 1C"]
fn shared_postgres_backend_reads_config_and_writes_parameterized_rows() -> Result<()> {
    let exec = SqlExec::postgres(SqlTarget {
        server: env::var("IBCMD_RS_PG_SERVER")?,
        database: Some(env::var("IBCMD_RS_PG_DATABASE")?),
        login: SqlLogin::from_user(
            Some(&env::var("IBCMD_RS_PG_USER")?),
            Some(&env::var("IBCMD_RS_PG_PASSWORD")?),
        ),
        trust_server_certificate: false,
    })?;
    let client = exec.client().unwrap();
    ensure!(client.dbms() == Dbms::PostgreSql);
    let mut rows = 0;
    client.read_rows(
        "SELECT filename::text, binarydata FROM config WHERE partno = $1",
        &[SqlParam::I32(0)],
        &mut |row| {
            ensure!(matches!(
                row.values.as_slice(),
                [SqlValue::Text(_), SqlValue::Binary(_)]
            ));
            rows += 1;
            Ok(())
        },
    )?;
    ensure!(rows > 0);
    ensure!(
        client
            .query_json("SELECT jsonb_build_object('k', true)")?
            .as_deref()
            == Some("{\"k\": true}")
    );
    let table = format!("pg_client_test_{}", std::process::id());
    client.run_script(
        &format!("CREATE TABLE {table} (name text UNIQUE, small int2, number int4, big int8, bytes bytea)"),
        ScriptVariables::Literal,
    )?;
    let checked = (|| -> Result<()> {
        let values = [
            SqlParam::Text("тест'\\"),
            SqlParam::U8(255),
            SqlParam::I32(-42),
            SqlParam::I64(i64::MAX),
            SqlParam::Binary(&[0, b'\\', 255]),
        ];
        let insert = format!("INSERT INTO {table} VALUES ($1, $2, $3, $4, $5)");
        ensure!(client.execute(&insert, &values)? == 1);
        ensure!(
            client
                .execute(&insert, &values)
                .unwrap_err()
                .to_string()
                .contains("23505")
        );
        let columns = ["name", "small", "number", "big", "bytes"];
        let mut another = values.to_vec();
        another[0] = SqlParam::Text("other");
        ensure!(client.write_rows(&table, &columns, &[another])? == 1);
        let query = format!("SELECT name, small, number, big, bytes FROM {table} WHERE name = $1");
        let result = client.query_rows(&query, &values[..1])?;
        ensure!(result.len() == 1);
        ensure!(
            result[0].values
                == [
                    SqlValue::Text("тест'\\".into()),
                    SqlValue::Int(255),
                    SqlValue::Int(-42),
                    SqlValue::Int(i64::MAX),
                    SqlValue::Binary(vec![0, b'\\', 255]),
                ]
        );
        Ok(())
    })();
    let cleanup = client.execute(&format!("DROP TABLE {table}"), &[]);
    checked?;
    cleanup?;
    Ok(())
}
