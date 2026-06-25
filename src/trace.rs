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

    let entry = groups
        .entry(normalized)
        .or_insert_with(|| GroupAccumulator {
            sample_sql: sql.trim().to_string(),
            ..GroupAccumulator::default()
        });
    entry.count += 1;
    entry.total_duration_us += duration;
    entry.max_duration_us = entry.max_duration_us.max(duration);
    if let Some(name) = event.event_name {
        entry.event_names.insert(name);
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use super::{GroupAccumulator, analyze_one_file, normalize_sql};

    #[test]
    fn normalizes_literals_and_numbers() {
        assert_eq!(
            normalize_sql("SELECT * FROM T WHERE ID = 42 AND NAME = 'ABC'"),
            "select * from t where id = ? and name = ?"
        );
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
  <event name="sql_batch_completed">
    <data name="duration"><value>10</value></data>
    <data name="batch_text"><value>SELECT * FROM T WHERE ID = 1</value></data>
  </event>
  <event name="sql_batch_completed">
    <data name="duration"><value>20</value></data>
    <data name="batch_text"><value>SELECT * FROM T WHERE ID = 2</value></data>
  </event>
</RingBufferTarget>
"#,
        )
        .unwrap();

        let mut events_seen = 0;
        let mut groups = BTreeMap::<String, GroupAccumulator>::new();
        analyze_one_file(&path, &mut events_seen, &mut groups).unwrap();

        assert_eq!(events_seen, 2);
        assert_eq!(groups.len(), 1);
        let group = groups.values().next().unwrap();
        assert_eq!(group.count, 2);
        assert_eq!(group.total_duration_us, 30);

        let _ = fs::remove_file(path);
        let _ = fs::remove_dir(dir);
    }
}
