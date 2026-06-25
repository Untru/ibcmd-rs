use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceAnalysis {
    pub files: Vec<PathBuf>,
    pub events_seen: usize,
    pub groups: Vec<QueryGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryGroup {
    pub normalized_sql: String,
    pub sample_sql: String,
    pub count: usize,
    pub total_duration_us: u64,
    pub max_duration_us: u64,
    pub average_duration_us: u64,
    pub event_names: Vec<String>,
    pub total_row_count: u64,
    pub max_row_count: u64,
    pub session_ids: Vec<String>,
    pub database_names: Vec<String>,
    pub client_hostnames: Vec<String>,
    pub client_app_names: Vec<String>,
    pub usernames: Vec<String>,
    pub transaction_ids: Vec<String>,
    pub attach_activity_ids: Vec<String>,
    pub attach_activity_id_xfers: Vec<String>,
    pub object_names: Vec<String>,
    pub table_names: Vec<String>,
    pub begin_transaction_count: usize,
    pub commit_transaction_count: usize,
    pub rollback_transaction_count: usize,
}

#[derive(Debug, Default)]
struct EventState {
    event_name: Option<String>,
    fields: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
struct GroupAccumulator {
    sample_sql: String,
    count: usize,
    total_duration_us: u64,
    max_duration_us: u64,
    event_names: BTreeSet<String>,
    total_row_count: u64,
    max_row_count: u64,
    session_ids: BTreeSet<String>,
    database_names: BTreeSet<String>,
    client_hostnames: BTreeSet<String>,
    client_app_names: BTreeSet<String>,
    usernames: BTreeSet<String>,
    transaction_ids: BTreeSet<String>,
    attach_activity_ids: BTreeSet<String>,
    attach_activity_id_xfers: BTreeSet<String>,
    object_names: BTreeSet<String>,
    table_names: BTreeSet<String>,
    begin_transaction_count: usize,
    commit_transaction_count: usize,
    rollback_transaction_count: usize,
}

pub fn analyze_trace_files(inputs: &[PathBuf]) -> Result<TraceAnalysis> {
    let mut events_seen = 0;
    let mut groups = BTreeMap::<String, GroupAccumulator>::new();

    for input in inputs {
        analyze_one_file(input, &mut events_seen, &mut groups)
            .with_context(|| format!("failed to analyze {}", input.display()))?;
    }

    let mut groups = groups
        .into_iter()
        .map(|(normalized_sql, group)| {
            let average_duration_us = if group.count == 0 {
                0
            } else {
                group.total_duration_us / group.count as u64
            };
            QueryGroup {
                normalized_sql,
                sample_sql: group.sample_sql,
                count: group.count,
                total_duration_us: group.total_duration_us,
                max_duration_us: group.max_duration_us,
                average_duration_us,
                event_names: group.event_names.into_iter().collect(),
                total_row_count: group.total_row_count,
                max_row_count: group.max_row_count,
                session_ids: group.session_ids.into_iter().collect(),
                database_names: group.database_names.into_iter().collect(),
                client_hostnames: group.client_hostnames.into_iter().collect(),
                client_app_names: group.client_app_names.into_iter().collect(),
                usernames: group.usernames.into_iter().collect(),
                transaction_ids: group.transaction_ids.into_iter().collect(),
                attach_activity_ids: group.attach_activity_ids.into_iter().collect(),
                attach_activity_id_xfers: group.attach_activity_id_xfers.into_iter().collect(),
                object_names: group.object_names.into_iter().collect(),
                table_names: group.table_names.into_iter().collect(),
                begin_transaction_count: group.begin_transaction_count,
                commit_transaction_count: group.commit_transaction_count,
                rollback_transaction_count: group.rollback_transaction_count,
            }
        })
        .collect::<Vec<_>>();

    groups.sort_by(|left, right| {
        right
            .total_duration_us
            .cmp(&left.total_duration_us)
            .then_with(|| right.count.cmp(&left.count))
            .then_with(|| left.normalized_sql.cmp(&right.normalized_sql))
    });

    Ok(TraceAnalysis {
        files: inputs.to_vec(),
        events_seen,
        groups,
    })
}

pub fn write_trace_analysis(analysis: &TraceAnalysis, output: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(analysis)?;
    fs::write(output, json).with_context(|| format!("failed to write {}", output.display()))
}

fn analyze_one_file(
    path: &Path,
    events_seen: &mut usize,
    groups: &mut BTreeMap<String, GroupAccumulator>,
) -> Result<()> {
    let text = fs::read_to_string(path)?;
    let mut reader = Reader::from_str(&text);
    reader.config_mut().trim_text(true);

    let mut current_event = None::<EventState>;
    let mut current_field = None::<String>;
    let mut in_value = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = element_name(event.name().as_ref());
                match name.as_str() {
                    "event" => {
                        current_event = Some(EventState {
                            event_name: attr_value(&event, "name"),
                            fields: BTreeMap::new(),
                        });
                    }
                    "data" | "action" => {
                        current_field = attr_value(&event, "name");
                    }
                    "value" => {
                        in_value = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(text)) => {
                if in_value {
                    if let (Some(event), Some(field)) = (&mut current_event, &current_field) {
                        event.fields.insert(
                            field.clone(),
                            decode_xml_text(&String::from_utf8_lossy(text.as_ref())),
                        );
                    }
                }
            }
            Ok(Event::End(event)) => {
                let name = element_name(event.name().as_ref());
                match name.as_str() {
                    "event" => {
                        if let Some(event) = current_event.take() {
                            *events_seen += 1;
                            record_event(event, groups);
                        }
                    }
                    "data" | "action" => {
                        current_field = None;
                    }
                    "value" => {
                        in_value = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
    }

    Ok(())
}

fn record_event(event: EventState, groups: &mut BTreeMap<String, GroupAccumulator>) {
    let Some(sql) = event
        .fields
        .get("sql_text")
        .or_else(|| event.fields.get("statement"))
        .or_else(|| event.fields.get("batch_text"))
        .filter(|value| !value.trim().is_empty())
    else {
        return;
    };

    let normalized = normalize_sql(sql);
    let duration = event
        .fields
        .get("duration")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_default();
    let row_count = event
        .fields
        .get("row_count")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_default();

    let entry = groups
        .entry(normalized)
        .or_insert_with(|| GroupAccumulator {
            sample_sql: sql.trim().to_string(),
            ..GroupAccumulator::default()
        });
    entry.count += 1;
    entry.total_duration_us += duration;
    entry.max_duration_us = entry.max_duration_us.max(duration);
    entry.total_row_count += row_count;
    entry.max_row_count = entry.max_row_count.max(row_count);
    if let Some(name) = event.event_name {
        entry.event_names.insert(name);
    }
    collect_field(&event.fields, "session_id", &mut entry.session_ids);
    collect_field(&event.fields, "database_name", &mut entry.database_names);
    collect_field(
        &event.fields,
        "client_hostname",
        &mut entry.client_hostnames,
    );
    collect_field(
        &event.fields,
        "client_app_name",
        &mut entry.client_app_names,
    );
    collect_field(&event.fields, "username", &mut entry.usernames);
    collect_field(&event.fields, "transaction_id", &mut entry.transaction_ids);
    collect_field(
        &event.fields,
        "attach_activity_id",
        &mut entry.attach_activity_ids,
    );
    collect_field(
        &event.fields,
        "attach_activity_id_xfer",
        &mut entry.attach_activity_id_xfers,
    );
    collect_field(&event.fields, "object_name", &mut entry.object_names);
    for table_name in extract_table_names(sql) {
        entry.table_names.insert(table_name);
    }
    if let Some(boundary) = transaction_boundary_kind(sql) {
        match boundary {
            TransactionBoundaryKind::Begin => entry.begin_transaction_count += 1,
            TransactionBoundaryKind::Commit => entry.commit_transaction_count += 1,
            TransactionBoundaryKind::Rollback => entry.rollback_transaction_count += 1,
        }
    }
}

fn normalize_sql(sql: &str) -> String {
    let without_literals = replace_literals(sql);
    let without_numbers = replace_numbers(&without_literals);
    without_numbers
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn replace_literals(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    let mut in_string = false;

    while let Some(ch) = chars.next() {
        if ch == '\'' {
            if in_string && chars.peek() == Some(&'\'') {
                let _ = chars.next();
                continue;
            }
            in_string = !in_string;
            if in_string {
                result.push('?');
            }
            continue;
        }

        if !in_string {
            result.push(ch);
        }
    }

    result
}

fn replace_numbers(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch.is_ascii_digit() {
            result.push('?');
            while chars
                .peek()
                .is_some_and(|next| next.is_ascii_digit() || *next == '.')
            {
                let _ = chars.next();
            }
        } else {
            result.push(ch);
        }
    }

    result
}

fn attr_value(event: &quick_xml::events::BytesStart<'_>, name: &str) -> Option<String> {
    event
        .attributes()
        .filter_map(Result::ok)
        .find(|attr| attr.key.as_ref() == name.as_bytes())
        .map(|attr| String::from_utf8_lossy(attr.value.as_ref()).to_string())
}

fn element_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name).to_string()
}

fn decode_xml_text(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn extract_table_names(sql: &str) -> BTreeSet<String> {
    let tokens = sql
        .split_whitespace()
        .map(normalize_sql_token)
        .collect::<Vec<_>>();
    let mut tables = BTreeSet::new();

    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index].to_ascii_lowercase();
        let next = tokens
            .get(index + 1)
            .map(|value| value.to_ascii_lowercase());
        let candidate = match token.as_str() {
            "from" | "join" | "into" | "update" => tokens.get(index + 1),
            "delete" if next.as_deref() == Some("from") => tokens.get(index + 2),
            _ => None,
        };

        if let Some(table) = candidate {
            if let Some(table) = table
                .split_whitespace()
                .next()
                .map(strip_sql_table_token)
                .map(normalize_sql_table_name)
                .filter(|value| !value.is_empty())
            {
                tables.insert(table);
            }
        }

        index += 1;
    }

    tables
}

fn normalize_sql_token(token: &str) -> String {
    token
        .trim_matches(|ch: char| matches!(ch, ',' | ';' | '(' | ')' | '\''))
        .replace(['[', ']'], "")
}

fn strip_sql_table_token(token: &str) -> String {
    token
        .trim_matches(|ch: char| matches!(ch, ',' | ';' | '(' | ')'))
        .replace(['[', ']'], "")
}

fn normalize_sql_table_name(token: String) -> String {
    if token.contains("..") {
        token
    } else if let Some((_, last)) = token.rsplit_once('.') {
        last.to_string()
    } else {
        token
    }
}

fn collect_field(
    fields: &BTreeMap<String, String>,
    field_name: &str,
    target: &mut BTreeSet<String>,
) {
    if let Some(value) = fields
        .get(field_name)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        target.insert(value.to_string());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionBoundaryKind {
    Begin,
    Commit,
    Rollback,
}

fn transaction_boundary_kind(sql: &str) -> Option<TransactionBoundaryKind> {
    let normalized = normalize_sql(sql);
    let first_two = normalized
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");
    match first_two.as_str() {
        "begin transaction" => Some(TransactionBoundaryKind::Begin),
        "commit transaction" => Some(TransactionBoundaryKind::Commit),
        "rollback transaction" => Some(TransactionBoundaryKind::Rollback),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

    use super::{
        GroupAccumulator, analyze_one_file, analyze_trace_files, extract_table_names,
        normalize_sql, transaction_boundary_kind,
    };

    #[test]
    fn normalizes_literals_and_numbers() {
        assert_eq!(
            normalize_sql("SELECT * FROM T WHERE ID = 42 AND NAME = 'ABC'"),
            "select * from t where id = ? and name = ?"
        );
    }

    #[test]
    fn extracts_table_names_from_common_statements() {
        let tables = extract_table_names(
            "SELECT * FROM dbo.Table1 JOIN [dbo].[Table2] ON 1=1; INSERT INTO ConfigSave (...) VALUES (1); DELETE FROM master..sysdatabases WHERE name = N'x'; UPDATE [dbo].[ConfigSave] SET A = 1",
        );
        assert_eq!(
            tables,
            [
                "ConfigSave".to_string(),
                "Table1".to_string(),
                "Table2".to_string(),
                "master..sysdatabases".to_string(),
            ]
            .into_iter()
            .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn detects_transaction_boundaries() {
        assert!(matches!(
            transaction_boundary_kind("BEGIN TRANSACTION"),
            Some(super::TransactionBoundaryKind::Begin)
        ));
        assert!(matches!(
            transaction_boundary_kind("COMMIT TRANSACTION"),
            Some(super::TransactionBoundaryKind::Commit)
        ));
        assert!(matches!(
            transaction_boundary_kind("ROLLBACK TRANSACTION"),
            Some(super::TransactionBoundaryKind::Rollback)
        ));
        assert!(transaction_boundary_kind("SELECT 1").is_none());
    }

    #[test]
    fn groups_extended_event_xml() {
        let dir = std::env::temp_dir().join(format!("ibcmd-rs-trace-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("events.xml");
        fs::write(
            &path,
            r#"
<RingBufferTarget>
  <event name="sql_statement_completed">
    <data name="duration"><value>10</value></data>
    <data name="row_count"><value>3</value></data>
    <data name="statement"><value>SELECT * FROM T WHERE ID = 1</value></data>
    <action name="session_id"><value>56</value></action>
    <action name="database_name"><value>DemoDb</value></action>
    <action name="client_hostname"><value>WS01</value></action>
    <action name="client_app_name"><value>1C</value></action>
    <action name="username"><value>Pavel</value></action>
    <action name="transaction_id"><value>12345</value></action>
    <action name="attach_activity_id"><value>ABC</value></action>
    <action name="attach_activity_id_xfer"><value>XYZ</value></action>
    <action name="object_name"><value>sp_executesql</value></action>
  </event>
  <event name="sql_batch_completed">
    <data name="duration"><value>20</value></data>
    <data name="row_count"><value>7</value></data>
    <data name="batch_text"><value>SELECT * FROM T WHERE ID = 2</value></data>
    <action name="session_id"><value>56</value></action>
    <action name="database_name"><value>DemoDb</value></action>
    <action name="client_hostname"><value>WS01</value></action>
  </event>
  <event name="sql_statement_completed">
    <data name="duration"><value>5</value></data>
    <data name="statement"><value>BEGIN TRANSACTION</value></data>
  </event>
</RingBufferTarget>
"#,
        )
        .unwrap();

        let mut events_seen = 0;
        let mut groups = BTreeMap::<String, GroupAccumulator>::new();
        analyze_one_file(&path, &mut events_seen, &mut groups).unwrap();

        assert_eq!(events_seen, 3);
        assert_eq!(groups.len(), 2);
        let begin_group = groups.get("begin transaction").unwrap();
        assert_eq!(begin_group.count, 1);
        assert_eq!(begin_group.total_duration_us, 5);
        assert_eq!(begin_group.begin_transaction_count, 1);
        assert_eq!(begin_group.commit_transaction_count, 0);
        assert_eq!(begin_group.rollback_transaction_count, 0);

        let query_group = groups.get("select * from t where id = ?").unwrap();
        assert_eq!(query_group.count, 2);
        assert_eq!(query_group.total_duration_us, 30);
        assert_eq!(query_group.total_row_count, 10);
        assert_eq!(query_group.max_row_count, 7);
        assert_eq!(
            query_group.session_ids,
            ["56".to_string()].into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(
            query_group.database_names,
            ["DemoDb".to_string()].into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(
            query_group.client_hostnames,
            ["WS01".to_string()].into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(
            query_group.client_app_names,
            ["1C".to_string()].into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(
            query_group.usernames,
            ["Pavel".to_string()].into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(
            query_group.transaction_ids,
            ["12345".to_string()].into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(
            query_group.attach_activity_ids,
            ["ABC".to_string()].into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(
            query_group.attach_activity_id_xfers,
            ["XYZ".to_string()].into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(
            query_group.object_names,
            ["sp_executesql".to_string()]
                .into_iter()
                .collect::<BTreeSet<_>>()
        );
        assert_eq!(
            query_group.table_names,
            ["T".to_string()].into_iter().collect::<BTreeSet<_>>()
        );

        let _ = fs::remove_file(path);
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn groups_transaction_boundaries_and_repeated_sql_fields() {
        let dir = std::env::temp_dir().join(format!(
            "ibcmd-rs-trace-boundary-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("events.xml");
        fs::write(
            &path,
            r#"
<RingBufferTarget>
  <event name="sql_statement_completed">
    <data name="duration"><value>11</value></data>
    <data name="row_count"><value>2</value></data>
    <data name="statement"><value>SELECT * FROM dbo.TableX WHERE ID = 10</value></data>
    <action name="session_id"><value>77</value></action>
    <action name="transaction_id"><value>9001</value></action>
    <action name="object_name"><value>sp_executesql</value></action>
  </event>
  <event name="sql_statement_completed">
    <data name="duration"><value>13</value></data>
    <data name="row_count"><value>4</value></data>
    <data name="statement"><value>SELECT * FROM dbo.TableX WHERE ID = 11</value></data>
    <action name="session_id"><value>77</value></action>
    <action name="transaction_id"><value>9001</value></action>
    <action name="object_name"><value>sp_executesql</value></action>
  </event>
  <event name="sql_statement_completed">
    <data name="duration"><value>5</value></data>
    <data name="statement"><value>BEGIN TRANSACTION</value></data>
  </event>
  <event name="sql_statement_completed">
    <data name="duration"><value>7</value></data>
    <data name="statement"><value>ROLLBACK TRANSACTION</value></data>
  </event>
</RingBufferTarget>
"#,
        )
        .unwrap();

        let mut events_seen = 0;
        let mut groups = BTreeMap::<String, GroupAccumulator>::new();
        analyze_one_file(&path, &mut events_seen, &mut groups).unwrap();

        assert_eq!(events_seen, 4);
        assert_eq!(groups.len(), 3);

        let query_group = groups.get("select * from dbo.tablex where id = ?").unwrap();
        assert_eq!(query_group.count, 2);
        assert_eq!(query_group.total_duration_us, 24);
        assert_eq!(query_group.total_row_count, 6);
        assert_eq!(query_group.max_row_count, 4);
        assert_eq!(query_group.begin_transaction_count, 0);
        assert_eq!(query_group.commit_transaction_count, 0);
        assert_eq!(query_group.rollback_transaction_count, 0);
        assert_eq!(
            query_group.session_ids,
            ["77".to_string()].into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(
            query_group.transaction_ids,
            ["9001".to_string()].into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(
            query_group.object_names,
            ["sp_executesql".to_string()]
                .into_iter()
                .collect::<BTreeSet<_>>()
        );

        let begin_group = groups.get("begin transaction").unwrap();
        assert_eq!(begin_group.count, 1);
        assert_eq!(begin_group.begin_transaction_count, 1);
        assert_eq!(begin_group.commit_transaction_count, 0);
        assert_eq!(begin_group.rollback_transaction_count, 0);

        let rollback_group = groups.get("rollback transaction").unwrap();
        assert_eq!(rollback_group.count, 1);
        assert_eq!(rollback_group.begin_transaction_count, 0);
        assert_eq!(rollback_group.commit_transaction_count, 0);
        assert_eq!(rollback_group.rollback_transaction_count, 1);

        let _ = fs::remove_file(path);
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn analyze_trace_files_merges_multiple_inputs() {
        let dir =
            std::env::temp_dir().join(format!("ibcmd-rs-trace-merge-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let first = dir.join("first.xml");
        let second = dir.join("second.xml");
        fs::write(
            &first,
            r#"
<RingBufferTarget>
  <event name="sql_statement_completed">
    <data name="duration"><value>4</value></data>
    <data name="row_count"><value>1</value></data>
    <data name="statement"><value>SELECT * FROM dbo.TableY WHERE ID = 1</value></data>
    <action name="session_id"><value>101</value></action>
  </event>
</RingBufferTarget>
"#,
        )
        .unwrap();
        fs::write(
            &second,
            r#"
<RingBufferTarget>
  <event name="sql_statement_completed">
    <data name="duration"><value>6</value></data>
    <data name="row_count"><value>3</value></data>
    <data name="statement"><value>SELECT * FROM dbo.TableY WHERE ID = 2</value></data>
    <action name="session_id"><value>101</value></action>
  </event>
  <event name="sql_statement_completed">
    <data name="duration"><value>1</value></data>
    <data name="statement"><value>COMMIT TRANSACTION</value></data>
  </event>
</RingBufferTarget>
"#,
        )
        .unwrap();

        let analysis = analyze_trace_files(&[first.clone(), second.clone()]).unwrap();
        assert_eq!(analysis.files, vec![first.clone(), second.clone()]);
        assert_eq!(analysis.events_seen, 3);
        assert_eq!(analysis.groups.len(), 2);

        let query_group = analysis
            .groups
            .iter()
            .find(|group| group.normalized_sql == "select * from dbo.tabley where id = ?")
            .unwrap();
        assert_eq!(query_group.count, 2);
        assert_eq!(query_group.total_duration_us, 10);
        assert_eq!(query_group.total_row_count, 4);
        assert_eq!(query_group.max_row_count, 3);
        assert_eq!(query_group.session_ids, vec!["101".to_string()]);

        let commit_group = analysis
            .groups
            .iter()
            .find(|group| group.normalized_sql == "commit transaction")
            .unwrap();
        assert_eq!(commit_group.count, 1);
        assert_eq!(commit_group.commit_transaction_count, 1);
        assert_eq!(commit_group.begin_transaction_count, 0);
        assert_eq!(commit_group.rollback_transaction_count, 0);

        let _ = fs::remove_file(first);
        let _ = fs::remove_file(second);
        let _ = fs::remove_dir(dir);
    }
}
