//! UI rollup of the shared canonical native owners; comparison remains owned by #362.
use super::{DatabaseVersionEvidence, ObjectStatus, StoredObject};
use crate::native_export_status::{ConfigDumpInfoSnapshot, NativeExportStatus};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn attach(
    objects: &[StoredObject],
    current: &ConfigDumpInfoSnapshot,
    native: &NativeExportStatus,
    baseline_identity: &str,
    database_snapshot: &str,
    statuses: &mut [ObjectStatus],
) {
    let changed = native
        .changes
        .iter()
        .map(|c| c.id.as_str())
        .collect::<BTreeSet<_>>();
    let owners = current.owners();
    let known_roots = objects
        .iter()
        .map(|object| object.full_name.as_str())
        .collect::<BTreeSet<_>>();
    let targets = statuses
        .iter()
        .map(|row| row.object.as_str())
        .collect::<BTreeSet<_>>();
    // Only names validated by the shared helper can be ancestor targets.
    let canonical = owners
        .by_id
        .values()
        .map(|owner| (owner.object.as_str(), owner.root.as_str()))
        .collect::<BTreeSet<_>>();
    let mut flags = BTreeMap::<String, bool>::new();
    let mut complete = !native.full_required && owners.unmapped_ids.is_empty();
    for entry in &current.entries {
        if let Some(owner) = owners.by_id.get(&entry.id) {
            if !known_roots.contains(owner.root.as_str()) {
                complete = false;
            }
            // A selected nested subsystem exports its descendants too. Roll up through
            // verified canonical ancestors, never a guessed closest string prefix.
            let is_changed = changed.contains(entry.id.as_str());
            let ancestors = owner.object.match_indices('.').filter_map(|(index, _)| {
                let prefix = &owner.object[..index];
                canonical
                    .contains(&(prefix, owner.root.as_str()))
                    .then_some(prefix)
            });
            for target in std::iter::once(owner.object.as_str())
                .chain(std::iter::once(owner.root.as_str()))
                .chain(ancestors)
            {
                if targets.contains(target) {
                    *flags.entry(target.to_owned()).or_default() |= is_changed;
                }
            }
        } else {
            complete = false;
        }
    }
    for row in statuses {
        let flag = flags.get(&row.object).copied();
        let conclusive = complete && flag.is_some() && row.version.is_some();
        row.database_version_status = Some(DatabaseVersionEvidence {
            basis: "native-config-version",
            object: row.object.clone(),
            object_version: row.version.clone(),
            baseline_identity: baseline_identity.into(),
            database_snapshot: database_snapshot.into(),
            conclusive,
            changed: conclusive.then_some(flag.unwrap_or(false)),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_export_status::ConfigVersionEntry;
    fn object(name: &str) -> StoredObject {
        StoredObject {
            kind: name.split('.').next().unwrap().into(),
            name: name.into(),
            full_name: name.into(),
            uuid: String::new(),
            path: String::new(),
            folder: String::new(),
        }
    }
    fn row(name: &str) -> ObjectStatus {
        ObjectStatus {
            object: name.into(),
            kind: String::new(),
            status: super::super::StatusKind::Same,
            version: Some("cache-token".into()),
            changed: vec![],
            base_only: vec![],
            local_only: vec![],
            database_version_status: None,
        }
    }
    fn snapshot() -> ConfigDumpInfoSnapshot {
        let mut entries = Vec::new();
        for (index, name) in ["Catalog.C", "Document.D", "CommonModule.Unrelated"]
            .iter()
            .enumerate()
        {
            let id = uuid::Uuid::from_u128(index as u128 + 1).to_string();
            entries.push(ConfigVersionEntry {
                name: (*name).into(),
                id: id.clone(),
                config_version: "a".repeat(40),
            });
            entries.push(ConfigVersionEntry {
                name: format!(
                    "{name}.{}",
                    if index == 2 { "Module" } else { "ObjectModule" }
                ),
                id: format!("{id}.0"),
                config_version: "a".repeat(40),
            });
        }
        entries.push(ConfigVersionEntry {
            name: "Catalog.C.Form.F.Form".into(),
            id: format!("{}.0", uuid::Uuid::from_u128(99)),
            config_version: "a".repeat(40),
        });
        ConfigDumpInfoSnapshot {
            entries,
            parts: BTreeMap::new(),
        }
    }
    #[test]
    fn large_owner_rollup_keeps_unrelated_roots_conclusive() {
        let mut current = ConfigDumpInfoSnapshot {
            entries: vec![],
            parts: BTreeMap::new(),
        };
        let mut objects = vec![];
        let mut rows = vec![];
        for i in 0..1000u128 {
            let name = format!("CommonModule.M{i}");
            objects.push(object(&name));
            rows.push(row(&name));
            let id = uuid::Uuid::from_u128(i + 1).to_string();
            for (name, id) in [
                (name.clone(), id.clone()),
                (format!("{name}.Module"), format!("{id}.0")),
            ] {
                current.entries.push(ConfigVersionEntry {
                    name,
                    id,
                    config_version: "a".repeat(40),
                });
            }
        }
        let baseline = current.clone();
        current.entries[777].config_version = "b".repeat(40);
        let native = current.compare(&baseline);
        let started = std::time::Instant::now();
        attach(&objects, &current, &native, "base", "snapshot", &mut rows);
        eprintln!(
            "rollup 2000 entries / 1000 targets: {:?}",
            started.elapsed()
        );
        for (i, row) in rows.iter().enumerate() {
            let evidence = row.database_version_status.as_ref().unwrap();
            assert!(evidence.conclusive);
            assert_eq!(evidence.changed, Some(i == 388));
        }
    }

    #[test]
    fn ordinary_modules_and_body_only_forms_do_not_poison_unrelated_clean_objects() {
        let baseline = snapshot();
        let objects = vec![
            object("Catalog.C"),
            object("Document.D"),
            object("CommonModule.Unrelated"),
        ];
        let mut rows = vec![
            row("Catalog.C"),
            row("Document.D"),
            row("CommonModule.Unrelated"),
            row("Catalog.C.Form.F"),
        ];
        attach(
            &objects,
            &baseline,
            &baseline.compare(&baseline),
            "base",
            "snapshot",
            &mut rows,
        );
        assert!(rows.iter().all(|r| {
            let e = r.database_version_status.as_ref().unwrap();
            e.conclusive && e.changed == Some(false)
        }));
        let form = baseline
            .owners()
            .by_id
            .values()
            .find(|o| o.object == "Catalog.C.Form.F")
            .unwrap()
            .clone();
        assert_eq!(form.root, "Catalog.C");
        assert_eq!(form.native_object, "Catalog.C.Form.F");
        let mut current = baseline.clone();
        for entry in &mut current.entries {
            if entry.name.contains("ObjectModule") || entry.name == "Catalog.C.Form.F.Form" {
                entry.config_version = "b".repeat(40);
            }
        }
        attach(
            &objects,
            &current,
            &current.compare(&baseline),
            "base",
            "snapshot",
            &mut rows,
        );
        for row in rows {
            assert_eq!(
                row.database_version_status.unwrap().changed,
                Some(row.object != "CommonModule.Unrelated")
            );
        }
    }
    #[test]
    fn future_unmapped_roles_stay_inconclusive_without_parent_fallback() {
        let mut current = snapshot();
        current.entries.push(ConfigVersionEntry {
            name: "Catalog.C.UnknownNested.X.Module".into(),
            id: uuid::Uuid::from_u128(100).to_string(),
            config_version: "c".repeat(40),
        });
        let mut rows = vec![row("Catalog.C"), row("CommonModule.Unrelated")];
        assert_eq!(current.owners().unmapped_ids.len(), 1);
        attach(
            &[
                object("Catalog.C"),
                object("Document.D"),
                object("CommonModule.Unrelated"),
            ],
            &current,
            &current.compare(&current),
            "base",
            "snapshot",
            &mut rows,
        );
        assert!(
            rows.iter()
                .all(|r| !r.database_version_status.as_ref().unwrap().conclusive)
        );
    }
    #[test]
    fn explicit_intermediate_subsystem_includes_changed_descendants() {
        let baseline = ConfigDumpInfoSnapshot {
            entries: [
                "Subsystem.A",
                "Subsystem.A.Subsystem.B",
                "Subsystem.A.Subsystem.B.Subsystem.C",
                "CommonModule.Unrelated",
            ]
            .iter()
            .enumerate()
            .map(|(i, name)| ConfigVersionEntry {
                name: (*name).into(),
                id: uuid::Uuid::from_u128(i as u128 + 1).to_string(),
                config_version: "a".repeat(40),
            })
            .collect(),
            parts: BTreeMap::new(),
        };
        let mut current = baseline.clone();
        current.entries[2].config_version = "b".repeat(40);
        let mut rows = baseline
            .entries
            .iter()
            .map(|entry| row(&entry.name))
            .collect::<Vec<_>>();
        attach(
            &[object("Subsystem.A"), object("CommonModule.Unrelated")],
            &current,
            &current.compare(&baseline),
            "base",
            "snapshot",
            &mut rows,
        );
        for row in rows {
            let evidence = row.database_version_status.unwrap();
            assert!(evidence.conclusive, "{}", row.object);
            assert_eq!(
                evidence.changed,
                Some(row.object != "CommonModule.Unrelated"),
                "{}",
                row.object
            );
        }
    }

    struct ReadOnlySql(std::sync::Arc<std::sync::Mutex<Vec<String>>>);
    impl crate::sql::SqlClient for ReadOnlySql {
        fn dbms(&self) -> crate::sql::Dbms {
            crate::sql::Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn run_script(&self, _: &str, _: crate::sql::ScriptVariables) -> anyhow::Result<()> {
            panic!("read adapter attempted script execution")
        }
        fn execute(&self, _: &str, _: &[crate::sql::SqlParam<'_>]) -> anyhow::Result<u64> {
            panic!("read adapter attempted SQL execution")
        }
        fn read_rows(
            &self,
            query: &str,
            _: &[crate::sql::SqlParam<'_>],
            _: &mut dyn FnMut(crate::sql::SqlRow) -> anyhow::Result<()>,
        ) -> anyhow::Result<()> {
            let select = query
                .trim_start()
                .strip_prefix("SET NOCOUNT ON;")
                .unwrap_or(query)
                .trim_start();
            assert!(select.starts_with("SELECT"), "{query}");
            self.0.lock().unwrap().push(query.into());
            Ok(())
        }
        fn query_json(&self, _: &str) -> anyhow::Result<Option<String>> {
            panic!("unexpected JSON read")
        }
        fn write_rows(
            &self,
            _: &str,
            _: &[&str],
            _: &[Vec<crate::sql::SqlParam<'_>>],
        ) -> anyhow::Result<u64> {
            panic!("read adapter attempted bulk write")
        }
    }
    #[test]
    fn sql_main_view_reads_configsave_and_uses_unfiltered_main_export() {
        let queries = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sql = crate::sql::SqlExec::with_client(
            crate::sql::SqlExec::detached("no network").target().clone(),
            Box::new(ReadOnlySql(queries.clone())),
        );
        let legacy = super::super::StoredConfiguration::new(
            super::super::ConfigSource::Database {
                sql,
                database: "db".into(),
            },
            crate::platform::parse("8.5.1").unwrap(),
        );
        let main = legacy.main_view();
        assert!(!legacy.main_configuration);
        assert!(main.main_configuration);
        assert!(main.inventory.is_none());
        let selected = BTreeSet::from(["some-row".into()]);
        let active_args = legacy.view_export_args(selected.clone());
        assert!(!active_args.main_configuration);
        assert_eq!(active_args.file_names, vec!["some-row"]);
        let main_args = main.view_export_args(selected);
        assert!(main_args.main_configuration);
        assert!(main_args.file_names.is_empty());
        let sql = match main.source() {
            super::super::ConfigSource::Database { sql, .. } => sql,
            _ => unreachable!(),
        };
        let _engine = super::super::engine();
        crate::mssql_dump::with_database_table(sql, "db", |table| table.names()).unwrap();
        assert!(
            !queries
                .lock()
                .unwrap()
                .iter()
                .any(|q| q.contains("[ConfigSave]"))
        );
        queries.lock().unwrap().clear();
        crate::mssql_dump::with_main_database_table(sql, "db", |table| table.names()).unwrap();
        let reads = queries.lock().unwrap();
        assert!(reads.iter().any(|q| q.contains("[Config]")), "{reads:?}");
        assert!(
            reads.iter().any(|q| q.contains("[ConfigSave]")),
            "{reads:?}"
        );
    }
}
