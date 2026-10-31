//! Clean-room source admission must fail before SQL is contacted.
//! Transaction/publication and real SQL acceptance are separate gates.
use ibcmd_rs::{
    file_import::Selection,
    mssql_source_change::SourceTraversalLimits,
    server::sql_import::{SourceAdmission, SourceLimits},
};
use std::{fs, path::Path};
#[path = "support/owned_scratch.rs"]
mod owned_scratch;
use owned_scratch::OwnedScratch;

const MODULE: &str = "CommonModules/Synthetic/Ext/Module.bsl";
fn fixture() -> OwnedScratch {
    let root = OwnedScratch::new("editor-sql-source");
    fs::create_dir_all(root.join("CommonModules/Synthetic/Ext")).unwrap();
    fs::write(
        root.join("CommonModules/Synthetic.xml"),
        b"<MetaDataObject/>",
    )
    .unwrap();
    fs::write(root.join(MODULE), b"// before\n").unwrap();
    root
}
fn selected() -> Selection {
    Selection {
        files: vec![MODULE.into()],
        ..Selection::default()
    }
}

#[test]
fn selection_and_verification_refuse_before_source_or_sql_io() {
    let absent = Path::new("nonexistent-editor-sql-admission-root");
    for selection in [
        Selection::default(),
        Selection {
            files: vec![MODULE.into()],
            changed: true,
            ..Selection::default()
        },
    ] {
        let error = SourceAdmission::prepare(absent, &selection, None, SourceLimits::default())
            .err()
            .unwrap();
        assert!(error.to_string().contains("exactly one"), "{error:#}");
    }
    for object in ["Configuration", "configuration", "Configuration.Synthetic"] {
        let selection = Selection {
            objects: vec![object.into()],
            ..Selection::default()
        };
        let error = SourceAdmission::prepare(absent, &selection, None, SourceLimits::default())
            .err()
            .unwrap();
        assert!(error.to_string().contains("Configuration"), "{error:#}");
    }
    let error = SourceAdmission::prepare(absent, &selected(), Some(false), SourceLimits::default())
        .err()
        .unwrap();
    assert!(error.to_string().contains("verification"), "{error:#}");
}

#[test]
fn admitted_bytes_are_immutable_and_changed_source_is_refused() {
    let root = fixture();
    let admitted =
        SourceAdmission::prepare(&root, &selected(), Some(true), SourceLimits::default()).unwrap();
    admitted.revalidate().unwrap();
    assert_eq!(admitted.bytes(MODULE).unwrap(), b"// before\n");
    fs::write(root.join(MODULE), b"// changed\n").unwrap();
    assert_eq!(admitted.bytes(MODULE).unwrap(), b"// before\n");
    assert!(admitted.revalidate().is_err());
}

#[test]
fn traversal_and_missing_or_aliased_files_refuse_without_sql() {
    let root = fixture();
    let limits = SourceLimits {
        traversal: SourceTraversalLimits {
            max_entries: 2,
            max_depth: 64,
        },
        ..SourceLimits::default()
    };
    assert!(SourceAdmission::prepare(&root, &selected(), None, limits).is_err());
    for file in [
        "../outside.bsl",
        "CommonModules/Synthetic/Ext/Missing.bsl",
        "Configuration.xml",
        "commonmodules/Synthetic/Ext/Module.bsl",
    ] {
        let selection = Selection {
            files: vec![file.into()],
            ..Selection::default()
        };
        assert!(
            SourceAdmission::prepare(&root, &selection, None, SourceLimits::default()).is_err(),
            "{file}"
        );
    }
}

#[test]
fn changed_mode_requires_synchronization_inventory_before_sql() {
    let root = fixture();
    let selection = Selection {
        changed: true,
        ..Selection::default()
    };
    let error = SourceAdmission::prepare(&root, &selection, None, SourceLimits::default())
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("ConfigDumpInfo.xml"));
}

#[cfg(unix)]
#[test]
fn source_links_are_refused_before_sql() {
    let root = fixture();
    std::os::unix::fs::symlink(root.join(MODULE), root.join("alias.bsl")).unwrap();
    assert!(SourceAdmission::prepare(&root, &selected(), None, SourceLimits::default()).is_err());
}

mod physical_readset_contracts {
    use ibcmd_rs::{
        mssql_live_continue::DatabaseIdentity,
        server::sql_import::readset::{Builder, Check, Input, Key, Limits, Part, Table},
    };
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    fn identity() -> DatabaseIdentity {
        DatabaseIdentity {
            server: "synthetic-server".into(),
            database: "synthetic-database".into(),
            database_guid: uuid::Uuid::from_u128(1).to_string(),
            family_guid: uuid::Uuid::from_u128(2).to_string(),
            recovery_fork: uuid::Uuid::from_u128(3).to_string(),
        }
    }
    fn row(name: &str, part: u32, size: u64, bytes: &[u8], header: &[u8]) -> Input {
        Input {
            key: Key {
                file_name: name.into(),
                part_no: part,
            },
            part: Part {
                data_size: size,
                header: header.to_vec(),
                binary: Arc::new(bytes.to_vec()),
            },
        }
    }
    fn builder() -> Builder {
        Builder::new(identity(), [1; 32], Limits::default(), Arc::new(|| Ok(()))).unwrap()
    }
    fn baseline() -> ibcmd_rs::server::sql_import::readset::Readset {
        let mut b = builder();
        b.push(
            Table::Config,
            row("selected", 0, 3, b"old", b"config-header"),
        )
        .unwrap();
        b.push(
            Table::Config,
            row("unrelated", 0, 3, b"old", b"config-header"),
        )
        .unwrap();
        b.push(
            Table::ConfigSave,
            row("neighbor", 0, 4, b"aa", b"stage-header-0"),
        )
        .unwrap();
        b.push(
            Table::ConfigSave,
            row("neighbor", 1, 4, b"bb", b"stage-header-1"),
        )
        .unwrap();
        b.finish().unwrap()
    }
    #[test]
    fn every_unselected_staged_part_and_header_is_preserved() {
        let original = baseline();
        let merged = original
            .with_selected_stage([
                row("selected", 0, 4, b"ab", b"new-0"),
                row("selected", 1, 4, b"cd", b"new-1"),
            ])
            .unwrap();
        assert!(
            original
                .parts(Table::Config)
                .eq(merged.parts(Table::Config))
        );
        assert!(
            original.parts(Table::ConfigSave).eq(merged
                .parts(Table::ConfigSave)
                .filter(|(key, _)| key.file_name == "neighbor"))
        );
        assert_ne!(original.digest().unwrap(), merged.digest().unwrap());
        assert!(
            original
                .with_selected_stage([row("new", 0, 1, b"x", b"header")])
                .is_err()
        );
    }
    #[test]
    fn shrinking_selected_stage_removes_old_tail_and_preserves_neighbor() {
        let original = baseline()
            .with_selected_stage([
                row("selected", 0, 4, b"ab", b"old-0"),
                row("selected", 1, 4, b"cd", b"old-1"),
            ])
            .unwrap();
        let replacement = row("selected", 0, 1, b"x", b"new-header");
        let expected = replacement.part.clone();
        let merged = original.with_selected_stage([replacement]).unwrap();
        let selected = merged
            .parts(Table::ConfigSave)
            .filter(|(key, _)| key.file_name == "selected")
            .collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].0.part_no, 0);
        assert_eq!(selected[0].1, &expected);
        assert!(
            original
                .parts(Table::ConfigSave)
                .filter(|(key, _)| key.file_name == "neighbor")
                .eq(merged
                    .parts(Table::ConfigSave)
                    .filter(|(key, _)| key.file_name == "neighbor"))
        );
        assert!(
            original
                .parts(Table::Config)
                .eq(merged.parts(Table::Config))
        );
        assert!(original.with_selected_stage([]).is_err());
    }
    #[test]
    fn individually_admissible_replacements_cannot_exceed_merged_budget() {
        let limits = Limits {
            max_payload_bytes: 4,
            ..Limits::default()
        };
        let mut b = Builder::new(identity(), [1; 32], limits, Arc::new(|| Ok(()))).unwrap();
        b.push(Table::Config, row("selected", 0, 2, b"ab", b"h"))
            .unwrap();
        b.push(Table::ConfigSave, row("neighbor", 0, 2, b"cd", b"h"))
            .unwrap();
        let original = b.finish().unwrap();
        assert!(
            original
                .with_selected_stage([row("selected", 0, 1, b"x", b"h")])
                .is_err()
        );
        assert_eq!(original.parts(Table::ConfigSave).count(), 1);
        assert_eq!(
            original
                .parts(Table::ConfigSave)
                .next()
                .unwrap()
                .1
                .binary
                .as_slice(),
            b"cd"
        );
    }
    #[test]
    fn identity_schema_unrelated_rows_headers_additions_and_deletions_change_binding() {
        let original = baseline().digest().unwrap();
        for variant in 0..6 {
            let mut id = identity();
            if variant == 0 {
                id.recovery_fork = uuid::Uuid::from_u128(4).to_string();
            }
            let mut b = Builder::new(
                id,
                if variant == 1 { [2; 32] } else { [1; 32] },
                Limits::default(),
                Arc::new(|| Ok(())),
            )
            .unwrap();
            b.push(
                Table::Config,
                row("selected", 0, 3, b"old", b"config-header"),
            )
            .unwrap();
            if variant != 5 {
                b.push(
                    Table::Config,
                    row(
                        "unrelated",
                        0,
                        3,
                        if variant == 2 { b"new" } else { b"old" },
                        b"config-header",
                    ),
                )
                .unwrap();
            }
            b.push(
                Table::ConfigSave,
                row(
                    "neighbor",
                    0,
                    4,
                    b"aa",
                    if variant == 3 {
                        b"changed-header"
                    } else {
                        b"stage-header-0"
                    },
                ),
            )
            .unwrap();
            b.push(
                Table::ConfigSave,
                row("neighbor", 1, 4, b"bb", b"stage-header-1"),
            )
            .unwrap();
            if variant == 4 {
                b.push(Table::ConfigSave, row("additional", 0, 1, b"x", b"header"))
                    .unwrap();
            }
            assert_ne!(
                original,
                b.finish().unwrap().digest().unwrap(),
                "variant {variant}"
            );
        }
    }
    #[test]
    fn duplicates_and_budget_errors_poison_the_whole_readset() {
        let mut b = builder();
        b.push(Table::Config, row("selected", 0, 1, b"a", b"h"))
            .unwrap();
        assert!(
            b.push(Table::Config, row("selected", 0, 1, b"a", b"h"))
                .is_err()
        );
        assert!(b.finish().is_err());
        let mut b = Builder::new(
            identity(),
            [1; 32],
            Limits {
                max_payload_bytes: 1,
                ..Limits::default()
            },
            Arc::new(|| Ok(())),
        )
        .unwrap();
        assert!(
            b.push(Table::Config, row("selected", 0, 2, b"ab", b"h"))
                .is_err()
        );
        assert!(b.finish().is_err());
    }
    #[test]
    fn part_count_part_size_name_and_identity_metadata_budgets_are_enforced() {
        for limits in [
            Limits {
                max_parts: 1,
                ..Limits::default()
            },
            Limits {
                max_part_bytes: 1,
                ..Limits::default()
            },
            Limits {
                max_name_units: 1,
                ..Limits::default()
            },
        ] {
            let mut b = Builder::new(identity(), [1; 32], limits, Arc::new(|| Ok(()))).unwrap();
            if limits.max_parts == 1 {
                b.push(Table::Config, row("a", 0, 1, b"a", b"h")).unwrap();
            }
            assert!(b.push(Table::Config, row("xx", 0, 2, b"ab", b"h")).is_err());
            assert!(b.finish().is_err());
        }
        assert!(
            Builder::new(
                identity(),
                [1; 32],
                Limits {
                    max_metadata_bytes: 1,
                    ..Limits::default()
                },
                Arc::new(|| Ok(()))
            )
            .is_err()
        );
    }

    #[test]
    fn part_gaps_inconsistent_sizes_and_truncated_payloads_are_refused() {
        for variant in 0..3 {
            let mut b = builder();
            b.push(Table::Config, row("selected", 0, 4, b"aa", b"h"))
                .unwrap();
            if variant != 2 {
                b.push(
                    Table::Config,
                    row(
                        "selected",
                        if variant == 0 { 2 } else { 1 },
                        if variant == 1 { 5 } else { 4 },
                        b"bb",
                        b"h",
                    ),
                )
                .unwrap();
            }
            assert!(b.finish().is_err());
        }
    }
    #[test]
    fn cancellation_remains_typed_after_capture() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let observed = cancelled.clone();
        let check: Check = Arc::new(move || {
            if observed.load(Ordering::Relaxed) {
                Err(ibcmd_rs::cancel::Cancelled.into())
            } else {
                Ok(())
            }
        });
        let mut b = Builder::new(identity(), [1; 32], Limits::default(), check).unwrap();
        b.push(Table::Config, row("selected", 0, 3, b"old", b"header"))
            .unwrap();
        let snapshot = b.finish().unwrap();
        cancelled.store(true, Ordering::Relaxed);
        assert!(ibcmd_rs::cancel::is_cancelled(
            &snapshot.digest().unwrap_err()
        ));
    }
}

mod snapshot_contracts {
    use anyhow::{Result, bail};
    use ibcmd_rs::{
        mssql_live_continue::DatabaseIdentity,
        server::sql_import::{
            readset::{Check, Limits, Table},
            snapshot::{self, Profile},
        },
        sql::{Dbms, ScriptVariables, SqlClient, SqlParam, SqlRow, SqlValue},
    };
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    #[derive(Clone, Copy)]
    enum Fault {
        None,
        Identity,
        Version,
        Schema,
        MissingSeal,
        WrongCount,
        WrongNonce,
        Truncated,
        DuplicateIdentity,
        OutOfOrder,
        BadType,
        TransportAfterSeal,
        SwallowedCallback,
        EmptySaved,
        EmptyBoth,
        ExtraAfterSeal,
        MissingIdentity,
        CancelAfterSeal,
    }
    struct Recording {
        calls: AtomicUsize,
        fault: Fault,
        cancel: Option<Arc<AtomicBool>>,
    }
    fn profile() -> Profile {
        Profile {
            identity: DatabaseIdentity {
                server: "synthetic-server".into(),
                database: "synthetic]db".into(),
                database_guid: uuid::Uuid::from_u128(1).to_string(),
                family_guid: uuid::Uuid::from_u128(2).to_string(),
                recovery_fork: uuid::Uuid::from_u128(3).to_string(),
            },
            sql_version: "synthetic-unmeasured".into(),
            schema: [7; 32],
        }
    }
    fn text(s: &str) -> SqlValue {
        SqlValue::Text(s.into())
    }
    fn row(set: usize, values: Vec<SqlValue>) -> SqlRow {
        SqlRow {
            result_set: set,
            values,
        }
    }
    fn part(set: usize, name: &str, bytes: &[u8]) -> SqlRow {
        row(
            set,
            vec![
                text(name),
                SqlValue::Int(0),
                SqlValue::Int(bytes.len() as i64),
                SqlValue::Binary(vec![1]),
                SqlValue::Binary(vec![2]),
                SqlValue::Binary(vec![3]),
                SqlValue::Binary(bytes.to_vec()),
            ],
        )
    }
    impl Recording {
        fn new(fault: Fault) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                fault,
                cancel: None,
            }
        }
    }
    impl SqlClient for Recording {
        fn dbms(&self) -> Dbms {
            Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            8
        }
        fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
            panic!("snapshot must not run a script")
        }
        fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
            panic!("snapshot must not execute DML")
        }
        fn query_json(&self, _: &str) -> Result<Option<String>> {
            panic!("snapshot must not issue a second query")
        }
        fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
            panic!("snapshot must not write rows")
        }
        fn read_rows(
            &self,
            query: &str,
            params: &[SqlParam<'_>],
            each: &mut dyn FnMut(SqlRow) -> Result<()>,
        ) -> Result<()> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            assert_eq!(query.matches("BEGIN TRANSACTION").count(), 1);
            assert_eq!(query.matches("WITH (TABLOCK,HOLDLOCK)").count(), 3);
            assert!(
                query
                    .find("FROM dbo.ConfigSave WITH (TABLOCK,HOLDLOCK)")
                    .unwrap()
                    < query
                        .find("FROM dbo.Params WITH (TABLOCK,HOLDLOCK)")
                        .unwrap()
            );
            assert!(query.contains("snapshot dynamic generation is unsupported"));
            assert!(query.contains("USE [synthetic]]db];"));
            assert!(
                query.find("snapshot aggregate budget exceeded").unwrap()
                    < query.find("SELECT FileName,PartNo,DataSize").unwrap()
            );
            let (success, failure) = query
                .split_once("END TRY\nBEGIN CATCH")
                .expect("snapshot has explicit success and failure paths");
            assert!(
                success.rfind("ROLLBACK TRANSACTION;").unwrap()
                    < success.rfind("SELECT @P7").unwrap()
            );
            assert!(failure.contains("IF @@TRANCOUNT>0 ROLLBACK TRANSACTION;"));
            assert!(!failure.contains("SELECT @P7"));
            assert!(query.contains("snapshot profile is not measured"));
            for dml in [
                "INSERT ",
                "DELETE ",
                "UPDATE ",
                "MERGE ",
                "COMMIT TRANSACTION",
            ] {
                assert!(!query.contains(dml));
            }
            let nonce = match params[6] {
                SqlParam::Text(s) => s,
                _ => panic!("missing nonce"),
            };
            assert!(matches!(params[0],SqlParam::Binary(s) if s==[7;32]));
            let p = profile();
            assert!(
                query.find("snapshot target identity changed").unwrap()
                    < query.find("SELECT FileName,PartNo,DataSize").unwrap()
            );
            let mut metadata = row(
                0,
                vec![
                    text(&p.identity.server),
                    text(&p.identity.database),
                    text(&p.identity.database_guid),
                    text(&p.identity.family_guid),
                    text(&p.identity.recovery_fork),
                    text(&p.sql_version),
                    SqlValue::Binary(p.schema.to_vec()),
                ],
            );
            match self.fault {
                Fault::Identity => metadata.values[0] = text("other-server"),
                Fault::Version => metadata.values[5] = text("other-version"),
                Fault::Schema => metadata.values[6] = SqlValue::Binary(vec![9; 32]),
                Fault::BadType => {
                    metadata.values[0] = SqlValue::Binary(b"private-user-path-sentinel".to_vec())
                }
                _ => {}
            }
            let mut rows = vec![metadata];
            if matches!(self.fault, Fault::DuplicateIdentity) {
                rows.push(rows[0].clone());
            }
            rows.push(part(1, "selected", b"old"));
            if !matches!(self.fault, Fault::EmptySaved | Fault::EmptyBoth) {
                rows.push(part(2, "neighbor", b"aa"));
            }
            if matches!(self.fault, Fault::OutOfOrder) {
                rows.swap(1, 2);
            }
            if matches!(self.fault, Fault::Truncated) {
                rows.remove(1);
            }
            if !matches!(self.fault, Fault::MissingSeal) {
                rows.push(row(
                    3,
                    vec![
                        text(if matches!(self.fault, Fault::WrongNonce) {
                            "wrong"
                        } else {
                            nonce
                        }),
                        SqlValue::Int(if matches!(self.fault, Fault::WrongCount) {
                            2
                        } else if matches!(self.fault, Fault::EmptyBoth) {
                            0
                        } else {
                            1
                        }),
                        SqlValue::Int(
                            if matches!(self.fault, Fault::EmptySaved | Fault::EmptyBoth) {
                                0
                            } else {
                                1
                            },
                        ),
                        SqlValue::Int(if matches!(self.fault, Fault::EmptyBoth) {
                            0
                        } else {
                            3
                        }),
                        SqlValue::Int(
                            if matches!(self.fault, Fault::EmptySaved | Fault::EmptyBoth) {
                                0
                            } else {
                                2
                            },
                        ),
                    ],
                ));
            }
            if matches!(self.fault, Fault::EmptyBoth) {
                rows.retain(|r| r.result_set == 0 || r.result_set == 3);
            }
            if matches!(self.fault, Fault::MissingIdentity) {
                rows.remove(0);
            }
            if matches!(self.fault, Fault::ExtraAfterSeal) {
                rows.push(part(2, "extra", b"x"));
            }
            for (index, row) in rows.into_iter().enumerate() {
                if index == 1
                    && !matches!(self.fault, Fault::CancelAfterSeal)
                    && let Some(cancel) = &self.cancel
                {
                    cancel.store(true, Ordering::Relaxed);
                }
                let result = each(row);
                if !matches!(self.fault, Fault::SwallowedCallback) {
                    result?;
                }
            }
            if matches!(self.fault, Fault::CancelAfterSeal)
                && let Some(cancel) = &self.cancel
            {
                cancel.store(true, Ordering::Relaxed);
            }
            if matches!(self.fault, Fault::TransportAfterSeal) {
                bail!("synthetic transport lost after seal");
            }
            Ok(())
        }
    }
    fn capture(
        client: &Recording,
        limits: Limits,
    ) -> Result<ibcmd_rs::server::sql_import::readset::Readset> {
        snapshot::capture(client, &profile(), limits, Arc::new(|| Ok(())))
    }
    #[test]
    fn one_request_captures_both_tables_and_complete_headers() {
        let client = Recording::new(Fault::None);
        let captured = capture(&client, Limits::default()).unwrap();
        assert_eq!(client.calls.load(Ordering::Relaxed), 1);
        let (key, part) = captured.parts(Table::ConfigSave).next().unwrap();
        assert_eq!(key.file_name, "neighbor");
        assert_eq!(part.binary.as_slice(), b"aa");
        let expected = [
            vec![1u64.to_le_bytes().to_vec(), vec![1]].concat(),
            vec![1u64.to_le_bytes().to_vec(), vec![2]].concat(),
            vec![1u64.to_le_bytes().to_vec(), vec![3]].concat(),
        ]
        .concat();
        assert_eq!(part.header, expected);
        assert!(
            capture(&Recording::new(Fault::EmptySaved), Limits::default())
                .unwrap()
                .parts(Table::ConfigSave)
                .next()
                .is_none()
        );
    }
    #[test]
    fn empty_tables_still_require_metadata_and_a_complete_seal() {
        let client = Recording::new(Fault::EmptyBoth);
        let captured = capture(&client, Limits::default()).unwrap();
        assert_eq!(captured.parts(Table::Config).count(), 0);
        assert_eq!(captured.parts(Table::ConfigSave).count(), 0);
        assert_eq!(client.calls.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn cancellation_after_valid_seal_still_discards_the_read_only_snapshot() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let seen = cancelled.clone();
        let check: Check = Arc::new(move || {
            if seen.load(Ordering::Relaxed) {
                Err(ibcmd_rs::cancel::Cancelled.into())
            } else {
                Ok(())
            }
        });
        let mut client = Recording::new(Fault::CancelAfterSeal);
        client.cancel = Some(cancelled);
        let error = snapshot::capture(&client, &profile(), Limits::default(), check)
            .err()
            .unwrap();
        assert!(ibcmd_rs::cancel::is_cancelled(&error));
        assert_eq!(client.calls.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn identity_schema_version_shape_and_frame_order_fail_closed() {
        for fault in [
            Fault::Identity,
            Fault::Version,
            Fault::Schema,
            Fault::BadType,
            Fault::DuplicateIdentity,
            Fault::OutOfOrder,
            Fault::MissingIdentity,
            Fault::ExtraAfterSeal,
        ] {
            let client = Recording::new(fault);
            assert!(capture(&client, Limits::default()).is_err());
            assert_eq!(client.calls.load(Ordering::Relaxed), 1);
        }
        let message = capture(&Recording::new(Fault::BadType), Limits::default())
            .err()
            .unwrap()
            .to_string();
        assert!(!message.contains("private-user-path-sentinel"));
    }
    #[test]
    fn missing_wrong_or_transport_failed_seal_never_returns_snapshot() {
        for fault in [
            Fault::MissingSeal,
            Fault::WrongCount,
            Fault::WrongNonce,
            Fault::Truncated,
            Fault::TransportAfterSeal,
        ] {
            let client = Recording::new(fault);
            assert!(capture(&client, Limits::default()).is_err());
            assert_eq!(client.calls.load(Ordering::Relaxed), 1);
        }
    }
    #[test]
    fn identity_metadata_is_reserved_before_server_row_admission() {
        struct BudgetProbe(AtomicUsize);
        impl SqlClient for BudgetProbe {
            fn dbms(&self) -> Dbms {
                Dbms::SqlServer
            }
            fn max_connections(&self) -> usize {
                1
            }
            fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
                panic!("unexpected")
            }
            fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
                panic!("unexpected")
            }
            fn query_json(&self, _: &str) -> Result<Option<String>> {
                panic!("unexpected")
            }
            fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
                panic!("unexpected")
            }
            fn read_rows(
                &self,
                _: &str,
                p: &[SqlParam<'_>],
                _: &mut dyn FnMut(SqlRow) -> Result<()>,
            ) -> Result<()> {
                self.0.fetch_add(1, Ordering::Relaxed);
                assert!(matches!(p[4], SqlParam::I64(0)));
                bail!("synthetic server refuses row metadata before streaming")
            }
        }
        let p = profile();
        let reserved = [
            &p.identity.server,
            &p.identity.database,
            &p.identity.database_guid,
            &p.identity.family_guid,
            &p.identity.recovery_fork,
        ]
        .into_iter()
        .map(|s| s.len() as u64)
        .sum();
        let probe = BudgetProbe(AtomicUsize::new(0));
        let result = snapshot::capture(
            &probe,
            &p,
            Limits {
                max_metadata_bytes: reserved,
                ..Limits::default()
            },
            Arc::new(|| Ok(())),
        );
        assert!(result.is_err());
        assert_eq!(probe.0.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn decoder_enforces_cumulative_and_part_limits_even_for_bad_transport() {
        for limits in [
            Limits {
                max_parts: 1,
                ..Limits::default()
            },
            Limits {
                max_part_bytes: 2,
                ..Limits::default()
            },
            Limits {
                max_payload_bytes: 4,
                ..Limits::default()
            },
            Limits {
                max_metadata_bytes: 200,
                ..Limits::default()
            },
        ] {
            assert!(capture(&Recording::new(Fault::None), limits).is_err());
        }
        let client = Recording::new(Fault::SwallowedCallback);
        assert!(
            capture(
                &client,
                Limits {
                    max_payload_bytes: 1,
                    ..Limits::default()
                }
            )
            .is_err()
        );
        assert_eq!(client.calls.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn invalid_bounds_or_pre_cancelled_request_never_touches_sql() {
        let client = Recording::new(Fault::None);
        assert!(
            capture(
                &client,
                Limits {
                    max_parts: 0,
                    ..Limits::default()
                }
            )
            .is_err()
        );
        let error = snapshot::capture(
            &client,
            &profile(),
            Limits::default(),
            Arc::new(|| Err(ibcmd_rs::cancel::Cancelled.into())),
        )
        .err()
        .unwrap();
        assert!(ibcmd_rs::cancel::is_cancelled(&error));
        assert_eq!(client.calls.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn in_stream_cancellation_remains_typed_and_is_not_retried() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let seen = cancelled.clone();
        let check: Check = Arc::new(move || {
            if seen.load(Ordering::Relaxed) {
                Err(ibcmd_rs::cancel::Cancelled.into())
            } else {
                Ok(())
            }
        });
        let mut client = Recording::new(Fault::None);
        client.cancel = Some(cancelled);
        let error = snapshot::capture(&client, &profile(), Limits::default(), check)
            .err()
            .unwrap();
        assert!(ibcmd_rs::cancel::is_cancelled(&error));
        assert_eq!(client.calls.load(Ordering::Relaxed), 1);
    }
}

mod coverage_contracts {
    use ibcmd_rs::server::sql_import::{
        coverage::{self, Limits},
        readset::Check,
    };
    use std::{
        collections::BTreeSet,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    fn keys(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|v| v.to_string()).collect()
    }
    fn check() -> Check {
        Arc::new(|| Ok(()))
    }
    fn limits() -> Limits {
        Limits {
            max_witness_keys: 32,
            max_key_bytes: 256,
        }
    }
    #[test]
    fn incoming_a_only_inventory_cannot_hide_configuration_neighbor_b() {
        let baseline = keys(&["A", "B"]);
        let stage = keys(&["A"]);
        let pruned = keys(&["A"]);
        assert!(
            coverage::require_preserved(&baseline, &stage, &pruned, &pruned, limits(), &check())
                .is_err()
        );
        assert!(
            coverage::require_preserved(&baseline, &stage, &baseline, &pruned, limits(), &check())
                .is_err()
        );
        assert!(
            coverage::require_preserved(
                &baseline,
                &stage,
                &baseline,
                &baseline,
                limits(),
                &check()
            )
            .is_ok()
        );
    }
    #[test]
    fn original_stage_neighbor_and_unexpected_compiled_keys_cannot_disappear_or_expand() {
        let baseline = keys(&["A", "B"]);
        let original_stage = keys(&["C"]);
        let complete = keys(&["A", "B", "C"]);
        assert!(
            coverage::require_preserved(
                &baseline,
                &original_stage,
                &baseline,
                &baseline,
                limits(),
                &check()
            )
            .is_err()
        );
        assert!(
            coverage::require_preserved(
                &baseline,
                &original_stage,
                &complete,
                &baseline,
                limits(),
                &check()
            )
            .is_err()
        );
        assert!(
            coverage::require_preserved(
                &baseline,
                &original_stage,
                &complete,
                &keys(&["A", "B", "C", "D"]),
                limits(),
                &check()
            )
            .is_err()
        );
        assert!(
            coverage::require_preserved(
                &baseline,
                &original_stage,
                &complete,
                &complete,
                limits(),
                &check()
            )
            .is_ok()
        );
    }
    #[test]
    fn structural_historical_projection_is_separate_from_partial_omission() {
        // Synthetic already-projected body witnesses only. This is NOT evidence
        // of actual engine alias resolution: raw physical history is not input.
        let projected = keys(&["A", "B"]);
        let empty = keys(&[]);
        assert!(
            coverage::require_preserved(
                &projected,
                &empty,
                &projected,
                &projected,
                limits(),
                &check()
            )
            .is_ok()
        );
        let new_partial = keys(&["A"]);
        assert!(
            coverage::require_preserved(
                &projected,
                &new_partial,
                &new_partial,
                &new_partial,
                limits(),
                &check()
            )
            .is_err()
        );
    }
    #[test]
    fn witness_cardinality_bytes_and_typed_cancellation_are_bounded() {
        let baseline = keys(&["A", "B"]);
        let empty = keys(&[]);
        for limit in [
            Limits {
                max_witness_keys: 5,
                ..limits()
            },
            Limits {
                max_key_bytes: 5,
                ..limits()
            },
        ] {
            assert!(
                coverage::require_preserved(
                    &baseline,
                    &empty,
                    &baseline,
                    &baseline,
                    limit,
                    &check()
                )
                .is_err()
            );
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let cancelled: Check = Arc::new(move || {
            if seen.fetch_add(1, Ordering::Relaxed) >= 2 {
                Err(ibcmd_rs::cancel::Cancelled.into())
            } else {
                Ok(())
            }
        });
        let error = coverage::require_preserved(
            &baseline,
            &empty,
            &baseline,
            &baseline,
            limits(),
            &cancelled,
        )
        .unwrap_err();
        assert!(ibcmd_rs::cancel::is_cancelled(&error));
    }
}

mod compiler_diagnostic_contracts {
    use super::*;
    use ibcmd_rs::server::sql_import::compiler_diagnostic::{self, Failure, Limits, ProvidedView};
    use std::{collections::BTreeMap, sync::Arc};
    fn view() -> ProvidedView {
        ProvidedView {
            rows: BTreeMap::new(),
            snapshot_label: [1; 32],
            schema_label: [2; 32],
            profile_label: [3; 32],
            epoch_label: 4,
            export_profile: "synthetic-profile".into(),
            export_schema: 1,
        }
    }
    #[test]
    fn library_diagnostic_guard_errors_are_redacted_and_cancellation_stays_typed() {
        let error = compiler_diagnostic::inspect_provided_view(
            view(),
            Path::new("private-path-user-sentinel"),
            &selected(),
            ibcmd_rs::platform::parse_flag("8.5.1").unwrap(),
            Limits::default(),
            Arc::new(|| Err(anyhow::anyhow!("private-path-user-sentinel"))),
        )
        .err()
        .unwrap();
        assert_eq!(error.downcast_ref::<Failure>(), Some(&Failure::Interrupted));
        assert!(!format!("{error:#}").contains("private-path-user-sentinel"));
        let error = compiler_diagnostic::inspect_provided_view(
            view(),
            Path::new("private-path-user-sentinel"),
            &selected(),
            ibcmd_rs::platform::parse_flag("8.5.1").unwrap(),
            Limits::default(),
            Arc::new(|| {
                Err(anyhow::Error::from(ibcmd_rs::cancel::Cancelled)
                    .context("private-path-user-sentinel"))
            }),
        )
        .err()
        .unwrap();
        assert!(ibcmd_rs::cancel::is_cancelled(&error));
        assert!(!format!("{error:#}").contains("private-path-user-sentinel"));
    }
    #[test]
    fn invalid_diagnostic_admission_has_no_private_error_chain() {
        let error = compiler_diagnostic::inspect_provided_view(
            view(),
            Path::new("private-path-user-sentinel"),
            &Selection::default(),
            ibcmd_rs::platform::parse_flag("8.5.1").unwrap(),
            Limits::default(),
            Arc::new(|| Ok(())),
        )
        .err()
        .unwrap();
        assert_eq!(error.downcast_ref::<Failure>(), Some(&Failure::Admission));
        assert_eq!(
            format!("{error:#}"),
            "compiler diagnostic source admission refused"
        );
    }
}

// Runs only on the disposable public SQL fixture, never in ordinary CI.
// The operator supplies an existing certificate-valid TCP endpoint. This uses
// the product's integrated TDS/TLS client, without certificate bypass or secrets.
#[test]
#[ignore = "sole operator: Windows, explicit public SQL fixture and existing valid TLS endpoint required"]
fn public_sql_snapshot_uses_real_product_tds_reader() -> anyhow::Result<()> {
    use anyhow::{Context, ensure};
    use ibcmd_rs::{
        mssql_live_continue::DatabaseIdentity,
        server::sql_import::{
            readset::{Limits, Table},
            snapshot::{self, Profile},
        },
        sql::{SqlLogin, SqlTarget, mssql::MssqlClient},
    };
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    ensure!(
        cfg!(windows),
        "public integrated-auth acceptance requires Windows"
    );
    const DATABASE: &str = "IBCMDLAB_PUBLIC_SQL_151c08962516426e96ccf4e48d240ff9";
    const GUID: &str = "9000716B-B12F-45BE-BB5B-27FB88534F95";
    ensure!(
        std::env::var("IBCMD_SQL_IMPORT_PUBLIC_FIXTURE").as_deref() == Ok(GUID),
        "explicit disposable public fixture opt-in required"
    );
    let endpoint = std::env::var("IBCMD_SQL_IMPORT_PUBLIC_ENDPOINT")
        .context("operator-measured certificate-valid TCP endpoint required")?;
    ensure!(
        !endpoint.trim().is_empty() && !endpoint.contains(['\0', '\r', '\n']),
        "invalid public SQL endpoint"
    );
    let profile = Profile {
        identity: DatabaseIdentity {
            server: "WIN-LHDQGRLB1L1".into(),
            database: DATABASE.into(),
            database_guid: GUID.into(),
            family_guid: "DF1E79FC-77D9-4DF6-AF59-C753F3AF6EFC".into(),
            recovery_fork: "DF1E79FC-77D9-4DF6-AF59-C753F3AF6EFC".into(),
        },
        sql_version: "16.0.1000.6".into(),
        schema: [
            0xad, 0xa2, 0x1b, 0x51, 0x3d, 0xe5, 0x5c, 0x54, 0x3d, 0xed, 0x7d, 0xe7, 0x24, 0xd1,
            0x34, 0x41, 0x52, 0x31, 0x78, 0xd0, 0x78, 0xd5, 0xf8, 0x58, 0xc9, 0x1d, 0xce, 0x0c,
            0xac, 0x10, 0xb6, 0x26,
        ],
    };
    let client = MssqlClient::new(
        SqlTarget {
            server: endpoint,
            database: Some(DATABASE.into()),
            login: SqlLogin::Integrated,
            trust_server_certificate: false,
        },
        1,
    )?;
    let started = Instant::now();
    let rows = snapshot::capture(
        &client,
        &profile,
        Limits {
            max_parts: 1024,
            max_part_bytes: 1024 * 1024,
            max_payload_bytes: 8 * 1024 * 1024,
            max_metadata_bytes: 262144,
            max_name_units: 128,
        },
        Arc::new(move || {
            ensure!(
                started.elapsed() < Duration::from_secs(30),
                "public snapshot deadline exceeded"
            );
            Ok(())
        }),
    )?;
    let parts = [
        rows.parts(Table::Config).count(),
        rows.parts(Table::ConfigSave).count(),
    ];
    let bytes: [usize; 2] = [Table::Config, Table::ConfigSave]
        .map(|table| rows.parts(table).map(|(_, part)| part.binary.len()).sum());
    ensure!(
        parts == [8, 0] && bytes == [3313, 0],
        "public fixture changed from measured snapshot"
    );
    // No names, headers or BinaryData leave the process. This is read acceptance,
    // not compiler, native export parity, CAS or publication authority.
    let digest = rows.digest()?;
    drop(rows);
    drop(client);
    println!("PUBLIC_SQL_SNAPSHOT_READ_PASS parts=8,0 bytes=3313,0 digest={digest:x?}");
    Ok(())
}

#[test]
fn common_module_plan_scope_refuses_before_source_or_sql() {
    use ibcmd_rs::{
        server::sql_import::{CommonModulePlan, RequestRefused},
        sql::SqlExec,
    };
    let sql = SqlExec::detached("SQL must not be contacted");
    let client = sql.client().unwrap();
    let absent = Path::new("nonexistent-common-module-plan-source");
    let check: ibcmd_rs::server::sql_import::readset::Check = std::sync::Arc::new(|| Ok(()));
    let verification = CommonModulePlan::prepare(
        client,
        "Synthetic",
        absent,
        &selected(),
        Some(false),
        check.clone(),
    )
    .err()
    .unwrap();
    assert!(verification.downcast_ref::<RequestRefused>().is_some());
    assert_eq!(
        verification.to_string(),
        "SQL editor import requires mandatory verification"
    );
    let mut cases = vec![
        Selection::default(),
        Selection {
            objects: vec!["CommonModule.Synthetic".into()],
            ..Selection::default()
        },
        Selection {
            changed: true,
            ..Selection::default()
        },
        Selection {
            files: vec![MODULE.into(), MODULE.into()],
            ..Selection::default()
        },
    ];
    for path in [
        "Configuration.xml",
        "Documents/Synthetic/Forms/Main/Ext/Form/Module.bsl",
        "CommonModules/Synthetic/Ext/Other.bsl",
        "commonmodules/Synthetic/Ext/Module.bsl",
        "CommonModules/../Ext/Module.bsl",
        "CommonModules/A/B/Ext/Module.bsl",
        "CommonModules//Ext/Module.bsl",
    ] {
        cases.push(Selection {
            files: vec![path.into()],
            ..Selection::default()
        });
    }
    for selection in cases {
        let error =
            CommonModulePlan::prepare(client, "Synthetic", absent, &selection, None, check.clone())
                .err()
                .unwrap();
        assert!(
            error.downcast_ref::<RequestRefused>().is_some(),
            "{error:#}"
        );
        assert_eq!(
            error.to_string(),
            "SQL preflight currently supports one existing CommonModule file"
        );
    }
}
