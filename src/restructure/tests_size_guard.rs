//! The size guard (S1-J, #406): the limit is on the sum over the stage, exactly at the limit passes, the
//! refusal names the largest table, the totals, the limit and the native apply, and the gate blocks and
//! drops the phase. No database: a source that answers with canned rows stands in for the server.
//!
//! The byte limit is on the bytes the rebuild writes: the data twice (the load into a heap, the clustered
//! index built over it) and the other indexes once, `2 x data + indexes`.

use std::collections::BTreeMap;

use anyhow::Result;

use crate::mssql_config_apply::gate::{GateVerdict, StructurePhase};
use crate::restructure::plan::Method;
use crate::restructure::reader::RowSource;
use crate::restructure::size_guard::{
    DEFAULT_LIMIT_BYTES, DEFAULT_LIMIT_ROWS, LOG_PER_REBUILD_BYTE, LimitSetting, RestructureLimit,
    TableSize, check_tables, evaluate, format_bytes, guard_phase, parse_byte_size, read_sizes,
};
use crate::sql::{SqlRow, SqlValue};

fn setting(rows: u64, bytes: u64) -> LimitSetting {
    LimitSetting {
        limit: RestructureLimit { rows, bytes },
        rows_source: "--restructure-limit-rows".to_owned(),
        bytes_source: "IBCMD_RS_RESTRUCTURE_LIMIT_BYTES".to_owned(),
    }
}

fn size(rows: u64, data: u64, index: u64) -> TableSize {
    TableSize {
        rows,
        data_bytes: data,
        index_bytes: index,
    }
}

fn tables(items: &[(&str, TableSize)]) -> BTreeMap<String, TableSize> {
    items
        .iter()
        .map(|(name, size)| (name.to_string(), *size))
        .collect()
}

/// A server that has these tables (name, rows, data bytes, index bytes) and remembers what it was asked.
struct Canned {
    tables: Vec<(String, i64, i64, i64)>,
    asked: Vec<String>,
}

impl Canned {
    fn new(tables: &[(&str, i64, i64, i64)]) -> Self {
        Self {
            tables: tables
                .iter()
                .map(|(name, rows, data, index)| (name.to_string(), *rows, *data, *index))
                .collect(),
            asked: Vec::new(),
        }
    }
}

impl RowSource for Canned {
    fn rows(&mut self, query: &str, each: &mut dyn FnMut(SqlRow) -> Result<()>) -> Result<()> {
        self.asked.push(query.to_owned());
        for (name, rows, data, index) in &self.tables {
            if query.contains(&format!("N'{name}'")) {
                each(SqlRow {
                    result_set: 0,
                    values: vec![
                        SqlValue::Text(name.clone()),
                        SqlValue::Int(*rows),
                        SqlValue::Int(*data),
                        SqlValue::Int(*index),
                    ],
                })?;
            }
        }
        Ok(())
    }
}

fn phase_of(tables: &[&str]) -> StructurePhase {
    StructurePhase {
        tables: tables.iter().map(|name| name.to_string()).collect(),
        objects: vec!["Catalog X".to_owned()],
        ..StructurePhase::default()
    }
}

#[test]
fn a_small_stage_is_within_the_limit() {
    let check = evaluate(
        tables(&[("_Reference20", size(14, 16_384, 81_920))]),
        &LimitSetting::default(),
    );
    assert!(check.within_limit());
    assert_eq!(check.refusal(), None);
    assert_eq!(check.total_rows, 14);
    assert_eq!(check.total_bytes, 98_304);
    // the data twice and the indexes once
    assert_eq!(check.total_rebuild_bytes, 2 * 16_384 + 81_920);
}

#[test]
fn exactly_at_the_limit_passes_and_one_above_refuses() {
    // bytes to write = 2 x data + indexes = 2 x 3000 + 4000 = 10000
    let limit = setting(1000, 10_000);
    let at = evaluate(tables(&[("_A", size(1000, 3_000, 4_000))]), &limit);
    assert!(at.within_limit(), "rows and bytes exactly at the limit");
    let one_row = evaluate(tables(&[("_A", size(1001, 3_000, 4_000))]), &limit);
    assert!(one_row.over_rows && !one_row.over_bytes);
    assert!(one_row.refusal().is_some());
    let one_byte = evaluate(tables(&[("_A", size(1000, 3_000, 4_001))]), &limit);
    assert!(one_byte.over_bytes && !one_byte.over_rows);
    // a byte of data is two bytes to write
    let one_data_byte = evaluate(tables(&[("_A", size(1000, 3_001, 4_000))]), &limit);
    assert!(one_data_byte.over_bytes);
    let both = evaluate(tables(&[("_A", size(2000, 6_000, 6_000))]), &limit);
    assert!(both.over_rows && both.over_bytes);
    let text = both.refusal().unwrap();
    assert!(
        text.contains("rows: 2000 above the limit of 1000"),
        "{text}"
    );
    assert!(
        text.contains(
            "bytes to write (the data twice and the indexes once): 17.6 KiB above the limit of 9.8 KiB"
        ),
        "{text}"
    );
}

#[test]
fn the_limit_is_on_the_sum_over_the_stage() {
    // each table alone is under the limit; the transaction rebuilds them together
    let limit = setting(1_000_000, 1_000);
    let stage = tables(&[
        ("_A", size(10, 200, 100)),
        ("_A_VT1", size(10, 150, 100)),
        ("_B", size(10, 150, 100)),
    ]);
    for table in stage.values() {
        assert!(table.rebuild_bytes() <= 1_000);
    }
    let check = evaluate(stage, &limit);
    assert_eq!(check.total_rebuild_bytes, 1_300);
    assert!(check.over_bytes);
}

#[test]
fn the_refusal_names_the_largest_table_the_totals_the_limit_and_the_native_apply() {
    let check = evaluate(
        tables(&[
            (
                "_Reference20",
                size(3_000_000, 1_660_000_000, 3_228_000_000),
            ),
            ("_Reference20_VT155", size(29, 16_384, 0)),
        ]),
        &setting(2_000_000, 3 << 30),
    );
    let text = check.refusal().expect("above the limit");
    assert!(text.contains("2 tables"), "{text}");
    assert!(text.contains("3000029 rows"), "{text}");
    assert!(
        text.contains("1.5 GiB of data and 3.0 GiB of indexes"),
        "{text}"
    );
    assert!(
        text.contains("The largest is _Reference20: 3000000 rows"),
        "{text}"
    );
    assert!(text.contains("--restructure-limit-rows"), "{text}");
    assert!(text.contains("IBCMD_RS_RESTRUCTURE_LIMIT_BYTES"), "{text}");
    assert!(text.contains("ibcmd infobase config apply"), "{text}");
    assert!(text.contains("log would grow by about"), "{text}");
    // the estimate is the measured factor of the bytes to write
    assert_eq!(
        check.estimated_log_bytes(),
        (check.total_rebuild_bytes as f64 * LOG_PER_REBUILD_BYTE) as u64
    );
}

#[test]
fn a_one_table_stage_says_table_not_tables() {
    let check = evaluate(tables(&[("_A", size(5, 10, 10))]), &setting(1, 1_000_000));
    let text = check.refusal().unwrap();
    assert!(text.contains("rebuilds 1 table with"), "{text}");
}

#[test]
fn an_empty_stage_and_a_missing_table_count_as_nothing() {
    let none = evaluate(BTreeMap::new(), &setting(1, 1));
    assert!(none.within_limit());
    assert!(none.largest.is_none());
    let mut server = Canned::new(&[]);
    let sizes = read_sizes(&mut server, &["_NewObject".to_owned()]).unwrap();
    assert_eq!(sizes["_NewObject"], TableSize::default());
    assert!(
        server.asked[0].contains("N'_NewObject'"),
        "the query names the table: {}",
        server.asked[0]
    );
    // no tables: no query at all
    let mut server = Canned::new(&[]);
    assert!(read_sizes(&mut server, &[]).unwrap().is_empty());
    assert!(server.asked.is_empty());
}

#[test]
fn the_sizes_come_from_the_partition_stats_of_the_named_tables_only() {
    let mut server = Canned::new(&[("_Reference20", 14, 16_384, 81_920), ("_Other", 9, 9, 9)]);
    let names = vec!["_Reference20".to_owned(), "_Reference20_VT155".to_owned()];
    let check = check_tables(&mut server, &names, &setting(100, 1 << 20)).unwrap();
    assert_eq!(check.tables["_Reference20"], size(14, 16_384, 81_920));
    assert_eq!(check.tables["_Reference20_VT155"], TableSize::default());
    assert!(!check.tables.contains_key("_Other"));
    let query = &server.asked[0];
    assert!(query.contains("sys.dm_db_partition_stats"), "{query}");
    assert!(query.contains("index_id IN (0, 1)"), "{query}");
    assert!(query.contains("index_id > 1"), "{query}");
    assert!(
        query.contains("N'_Reference20', N'_Reference20_VT155'"),
        "{query}"
    );
}

#[test]
fn a_name_that_is_not_a_plain_identifier_is_not_put_into_a_query() {
    let mut server = Canned::new(&[]);
    for bad in ["", "_A'; DROP TABLE Config;--", "dbo._A", "_A B"] {
        assert!(
            read_sizes(&mut server, &[bad.to_owned()]).is_err(),
            "{bad:?}"
        );
    }
    assert!(server.asked.is_empty());
}

#[test]
fn the_name_the_server_returns_is_matched_without_regard_to_case() {
    struct Upper;
    impl RowSource for Upper {
        fn rows(&mut self, _: &str, each: &mut dyn FnMut(SqlRow) -> Result<()>) -> Result<()> {
            each(SqlRow {
                result_set: 0,
                values: vec![
                    SqlValue::Text("_REFERENCE20".to_owned()),
                    SqlValue::Int(3),
                    SqlValue::Int(8192),
                    SqlValue::Int(0),
                ],
            })
        }
    }
    let sizes = read_sizes(&mut Upper, &["_Reference20".to_owned()]).unwrap();
    assert_eq!(sizes.len(), 1, "{sizes:?}");
    assert_eq!(sizes["_Reference20"].rows, 3);
}

#[test]
fn the_gate_blocks_a_stage_above_the_limit_and_drops_its_phase() {
    // 5000 rows; 2 x 1 000 000 + 2 000 000 = 4 000 000 bytes to write
    let mut server = Canned::new(&[("_Reference20", 5_000, 1_000_000, 2_000_000)]);
    let names = ["_Reference20", "_Reference20_VT159"];

    // within (exactly at the byte limit): the phase stays and carries the verdict
    let mut verdict = GateVerdict::default();
    let mut kept = Some(phase_of(&names));
    guard_phase(
        &mut server,
        Method::Rebuild,
        &setting(10_000, 4_000_000),
        &mut verdict,
        &mut kept,
    )
    .unwrap();
    assert!(verdict.blockers.is_empty() && !verdict.restructuring_required);
    let report = kept
        .expect("the phase stays")
        .size_check
        .expect("the verdict is reported");
    assert_eq!(report["within_limit"], true);
    assert_eq!(report["limit"]["rows"], 10_000);
    assert_eq!(
        report["limit"]["bytes_from"],
        "IBCMD_RS_RESTRUCTURE_LIMIT_BYTES"
    );
    assert_eq!(report["rebuilt"]["rows"], 5_000);
    assert_eq!(report["rebuilt"]["rebuild_bytes"], 4_000_000);
    assert_eq!(report["rebuilt"]["bytes"], 3_000_000);
    assert_eq!(report["largest"]["table"], "_Reference20");

    // above: one blocker that starts like every reason of the S1 gate, no phase
    let mut verdict = GateVerdict::default();
    let mut dropped = Some(phase_of(&names));
    guard_phase(
        &mut server,
        Method::Rebuild,
        &setting(4_999, 10_000_000),
        &mut verdict,
        &mut dropped,
    )
    .unwrap();
    assert!(dropped.is_none());
    assert!(verdict.restructuring_required);
    assert_eq!(verdict.blockers.len(), 1);
    let reason = &verdict.blockers[0].reason;
    assert!(
        reason.starts_with("S1: the stage rebuilds 2 tables"),
        "{reason}"
    );
    assert!(reason.contains("ibcmd infobase config apply"), "{reason}");
}

#[test]
fn only_a_rebuild_is_guarded_and_no_phase_needs_no_guard() {
    let mut server = Canned::new(&[("_Reference20", 9_999_999, 1 << 40, 1 << 40)]);
    let mut verdict = GateVerdict::default();
    let mut kept = Some(phase_of(&["_Reference20"]));
    // the research AlterAdd adds a column in place: nothing is copied, nothing is asked
    guard_phase(
        &mut server,
        Method::AlterAdd,
        &setting(1, 1),
        &mut verdict,
        &mut kept,
    )
    .unwrap();
    assert!(kept.is_some() && verdict.blockers.is_empty() && server.asked.is_empty());
    // a stage the gate already refused has no phase
    let mut none: Option<StructurePhase> = None;
    guard_phase(
        &mut server,
        Method::Rebuild,
        &setting(1, 1),
        &mut verdict,
        &mut none,
    )
    .unwrap();
    assert!(none.is_none() && verdict.blockers.is_empty() && server.asked.is_empty());
}

#[test]
fn a_server_that_cannot_answer_fails_the_gate_closed() {
    struct Broken;
    impl RowSource for Broken {
        fn rows(&mut self, _: &str, _: &mut dyn FnMut(SqlRow) -> Result<()>) -> Result<()> {
            anyhow::bail!("VIEW DATABASE STATE permission was denied")
        }
    }
    let mut verdict = GateVerdict::default();
    let mut phase = Some(phase_of(&["_A"]));
    let error = guard_phase(
        &mut Broken,
        Method::Rebuild,
        &LimitSetting::default(),
        &mut verdict,
        &mut phase,
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("the sizes of the tables to rebuild"),
        "{error:#}"
    );
}

#[test]
fn byte_sizes_read_the_units_every_one_a_power_of_1024() {
    assert_eq!(parse_byte_size("4294967296").unwrap(), 4 << 30);
    assert_eq!(parse_byte_size("4GB").unwrap(), 4 << 30);
    assert_eq!(parse_byte_size("4 GiB").unwrap(), 4 << 30);
    assert_eq!(parse_byte_size(" 512mb ").unwrap(), 512 << 20);
    assert_eq!(parse_byte_size("1.5g").unwrap(), 3 << 29);
    assert_eq!(parse_byte_size("64k").unwrap(), 64 << 10);
    assert_eq!(parse_byte_size("2_000_000").unwrap(), 2_000_000);
    assert_eq!(parse_byte_size("1TB").unwrap(), 1 << 40);
    for bad in ["", "GB", "4 PB", "x4", "1.2.3", "-1", "99999999999TB"] {
        assert!(parse_byte_size(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn bytes_are_told_in_binary_units() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1023), "1023 B");
    assert_eq!(format_bytes(1024), "1.0 KiB");
    assert_eq!(format_bytes(3 << 30), "3.0 GiB");
    assert_eq!(format_bytes(1536 << 20), "1.5 GiB");
}

#[test]
fn the_default_is_the_measured_constants() {
    let default = LimitSetting::default();
    assert_eq!(default.limit.rows, DEFAULT_LIMIT_ROWS);
    assert_eq!(default.limit.bytes, DEFAULT_LIMIT_BYTES);
    assert_eq!(default.rows_source, "the default");
    assert_eq!(DEFAULT_LIMIT_BYTES, 2 << 30);
    assert_eq!(DEFAULT_LIMIT_ROWS, 10_000_000);
}
