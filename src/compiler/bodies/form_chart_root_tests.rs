//! The test of `form_chart` (in `ibcmd-form-writer`) that renders the
//! written bodies through the root's exporter.

#[cfg(test)]
mod tests {
    /// The whole form, not just its chart. Each ERP УХ form that holds a chart
    /// attribute, as the writer compiled it (`audit-native-form-writer` with
    /// `IBCMD_RS_WRITE_BODIES_DIR`; `IBCMD_CHART_BODIES` names that
    /// directory), and its stored body are both rendered by the exporter
    /// against the offline context of a saved dump (`IBCMD_CHART_RUN_ROOT`, a
    /// directory whose `candidate_dump` is that dump). A compiled body that
    /// renders what the stored body renders exports back to the native
    /// Form.xml wherever the stored body does.
    #[test]
    #[ignore = "reads the ERP УХ dump and compiled bodies under F:\\ibcmd\\lab"]
    fn chart_forms_export_as_their_stored_bodies_do() {
        let corpus = std::env::var("IBCMD_CHART_CORPUS")
            .unwrap_or_else(|_| "F:/ibcmd/lab/tools/agent-chart/corpus".to_string());
        let bodies = std::env::var("IBCMD_CHART_BODIES")
            .unwrap_or_else(|_| "F:/ibcmd/lab/tools/agent-chart/audit2/bodies".to_string());
        let run_root = std::env::var("IBCMD_CHART_RUN_ROOT")
            .unwrap_or_else(|_| "F:/ibcmd/lab/tools/agent-chart/offline_root".to_string());
        let context = crate::mssql_dump::offline_context::OfflineFormContextFactory::from_run_root(
            std::path::Path::new(&run_root),
            None,
        )
        .expect("offline context");
        let render = |path: &str, uuid: &str| -> Option<String> {
            let text = std::fs::read_to_string(path).ok()?;
            let body = ibcmd_form_body::parse_form_body_plain(text.trim_start_matches('\u{feff}'))
                .expect("form body parses");
            crate::mssql_dump::render_form_body_xml_offline(&body, &context, uuid)
        };
        let index = std::fs::read_to_string(format!("{corpus}/index.tsv")).expect("index.tsv");
        let mut seen = Vec::new();
        let mut report = String::new();
        let mut compiled = 0;
        let mut same = 0;
        let mut different = Vec::new();
        for line in index.lines().filter(|line| !line.trim().is_empty()) {
            let fields = line.split('\t').collect::<Vec<_>>();
            let (path, uuid, stored_path) = (fields[2], fields[4], fields[5]);
            if seen.contains(&uuid) {
                continue;
            }
            seen.push(uuid);
            let stored = render(stored_path, uuid).expect("the stored body exports");
            let Some(written) = render(&format!("{bodies}/{uuid}.0.txt"), uuid) else {
                report.push_str(&format!("{path}: not compiled\n"));
                continue;
            };
            compiled += 1;
            if written == stored {
                same += 1;
                report.push_str(&format!("{path}: exports as the stored body does\n"));
            } else {
                let at = written
                    .lines()
                    .zip(stored.lines())
                    .position(|(left, right)| left != right)
                    .unwrap_or(0);
                report.push_str(&format!(
                    "{path}: differs at line {}: {:?} / stored {:?}\n",
                    at + 1,
                    written.lines().nth(at),
                    stored.lines().nth(at)
                ));
                different.push(path.to_string());
            }
        }
        report.push_str(&format!(
            "forms: {}; compiled: {compiled}; export as the stored body: {same}\n",
            seen.len()
        ));
        println!("{report}");
        if let Ok(path) = std::env::var("IBCMD_CHART_REPORT") {
            std::fs::write(path, &report).expect("report");
        }
        assert!(different.is_empty(), "differ: {different:#?}");
    }
}
