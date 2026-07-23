use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use clap::Parser;
use ibcmd_rs::{
    cli::{Cli, Commands},
    plan::{
        SourceDiffReport, build_parity_matrix, classify_parity_path, merge_parity_matrices,
        parity_exact_percent, read_parity_matrix, render_parity_matrix_markdown,
        validate_parity_matrix, write_parity_artifacts, write_parity_matrix,
    },
};

fn fixture(name: &str) -> SourceDiffReport {
    serde_json::from_str(
        &fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/source-parity")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn synthetic_fixtures_preserve_every_raw_status_and_classify_special_paths() {
    let matrix = build_parity_matrix(
        &fixture("ut-diff.json"),
        "ut".into(),
        "run-ut".into(),
        "deadbeef".into(),
        true,
    )
    .unwrap();
    assert_eq!(matrix.rows.len(), 4);
    assert_eq!(matrix.runs[0].raw_summary.unchanged, 1);
    assert_eq!(
        matrix
            .rows
            .iter()
            .filter(|row| format!("{:?}", row.raw_status) == "Different")
            .count(),
        1
    );
    assert!(
        matrix
            .rows
            .iter()
            .any(|row| row.artifact_class == "form_body" && row.family == "CommonForms")
    );
    assert!(
        matrix
            .rows
            .iter()
            .any(|row| row.artifact_class == "template_body" && row.family == "CommonTemplates")
    );
    assert_eq!(parity_exact_percent(&matrix.runs[0]), Some(25.0));
    assert_eq!(
        classify_parity_path(r"CommonForms\\Entry\\Ext\\Form.xml", None),
        ("CommonForms".into(), "form_body".into())
    );
    assert_eq!(
        classify_parity_path("Ext/Form.xml", None),
        ("Configuration".into(), "form_body".into())
    );
    assert_eq!(
        classify_parity_path("Ext/Template.xml", None),
        ("Configuration".into(), "template_body".into())
    );
    assert_eq!(
        classify_parity_path("Ext/Logo.bin", None),
        ("Configuration".into(), "ext_asset".into())
    );
    assert!(matrix.rows.iter().any(|row| {
        row.path == "Ext/Logo.bin"
            && row.family == "Configuration"
            && row.artifact_class == "ext_asset"
    }));
}

#[test]
fn scoped_runs_cannot_report_readiness_and_merged_runs_stay_separate() {
    let ut = build_parity_matrix(
        &fixture("ut-diff.json"),
        "ut".into(),
        "run".into(),
        "a".into(),
        false,
    )
    .unwrap();
    let bsp = build_parity_matrix(
        &fixture("bsp-diff.json"),
        "bsp".into(),
        "run".into(),
        "b".into(),
        true,
    )
    .unwrap();
    assert_eq!(parity_exact_percent(&ut.runs[0]), None);
    let merged = merge_parity_matrices(&[ut, bsp]).unwrap();
    assert_eq!(merged.runs.len(), 2);
    assert_eq!(merged.rows.len(), 8);
    let markdown = render_parity_matrix_markdown(&merged).unwrap();
    assert!(markdown.contains("н/д (выборка)"));
    assert!(markdown.contains("| bsp |"));
    assert!(markdown.contains("## Итоги по семействам"));
}

#[test]
fn matrix_roundtrip_is_strict_and_deterministic() {
    let matrix = build_parity_matrix(
        &fixture("ut-diff.json"),
        "ut".into(),
        "roundtrip".into(),
        "a".into(),
        true,
    )
    .unwrap();
    let output = std::env::temp_dir().join(format!("ibcmd-rs-parity-{}.json", std::process::id()));
    let _ = fs::remove_file(&output);
    write_parity_matrix(&matrix, &output, false).unwrap();
    assert_eq!(read_parity_matrix(&output).unwrap(), matrix);
    assert!(write_parity_matrix(&matrix, &output, false).is_err());
    assert_eq!(
        render_parity_matrix_markdown(&matrix).unwrap(),
        render_parity_matrix_markdown(&matrix).unwrap()
    );
    fs::remove_file(output).unwrap();
}

#[test]
fn aggregate_keys_keep_runs_of_the_same_database_separate() {
    let first = build_parity_matrix(
        &fixture("ut-diff.json"),
        "ut".into(),
        "first".into(),
        "a".into(),
        true,
    )
    .unwrap();
    let second = build_parity_matrix(
        &fixture("ut-diff.json"),
        "ut".into(),
        "second".into(),
        "b".into(),
        true,
    )
    .unwrap();
    let merged = merge_parity_matrices(&[first, second]).unwrap();
    assert!(merged.aggregates.iter().any(|item| item.run_id == "first"));
    assert!(merged.aggregates.iter().any(|item| item.run_id == "second"));
    assert!(merged.aggregates.iter().all(|item| item.files == 1));
}

#[test]
fn duplicate_paths_are_rejected() {
    let mut matrix = build_parity_matrix(
        &fixture("ut-diff.json"),
        "ut".into(),
        "duplicate".into(),
        "a".into(),
        true,
    )
    .unwrap();
    matrix.rows.push(matrix.rows[0].clone());
    assert!(validate_parity_matrix(&matrix).is_err());
}

#[test]
fn paired_publication_preflights_conflicts_and_preserves_old_files() {
    let matrix = build_parity_matrix(
        &fixture("ut-diff.json"),
        "ut".into(),
        "publish".into(),
        "a".into(),
        true,
    )
    .unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ibcmd-rs-parity-publish-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let json = root.join("matrix.json");
    let markdown = root.join("matrix.md");

    assert!(write_parity_artifacts(&matrix, &json, Some(&json), false).is_err());
    assert!(!json.exists());

    fs::write(&markdown, "old markdown").unwrap();
    assert!(write_parity_artifacts(&matrix, &json, Some(&markdown), false).is_err());
    assert!(!json.exists());
    assert_eq!(fs::read_to_string(&markdown).unwrap(), "old markdown");

    fs::write(&json, "old json").unwrap();
    let mut invalid = matrix.clone();
    invalid.aggregates.clear();
    assert!(write_parity_artifacts(&invalid, &json, Some(&markdown), true).is_err());
    assert_eq!(fs::read_to_string(&json).unwrap(), "old json");
    assert_eq!(fs::read_to_string(&markdown).unwrap(), "old markdown");

    write_parity_artifacts(&matrix, &json, Some(&markdown), true).unwrap();
    assert_eq!(read_parity_matrix(&json).unwrap(), matrix);
    assert!(
        fs::read_to_string(&markdown)
            .unwrap()
            .contains("Итоги по семействам")
    );
    assert!(fs::read_dir(&root).unwrap().all(|entry| {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        !name.ends_with(".tmp") && !name.ends_with(".backup")
    }));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cli_parses_matrix_and_merge_commands() {
    let cli = Cli::parse_from([
        "ibcmd-rs",
        "source-diff-matrix",
        "diff.json",
        "--database",
        "ut",
        "--run-id",
        "r1",
        "--git-sha",
        "abc",
        "--full",
        "-o",
        "matrix.json",
        "--markdown",
        "matrix.md",
    ]);
    match cli.command {
        Commands::SourceDiffMatrix(args) => {
            assert!(args.full);
            assert!(!args.scoped);
            assert_eq!(args.database, "ut");
            assert_eq!(args.output, PathBuf::from("matrix.json"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
    let cli = Cli::parse_from([
        "ibcmd-rs",
        "source-diff-matrix-merge",
        "ut.json",
        "bsp.json",
        "-o",
        "all.json",
    ]);
    match cli.command {
        Commands::SourceDiffMatrixMerge(args) => assert_eq!(args.matrices.len(), 2),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn cli_builds_and_merges_fixture_matrices() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures = root.join("tests/fixtures/source-parity");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output = std::env::temp_dir().join(format!(
        "ibcmd-rs-parity-cli-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&output).unwrap();
    let ut = output.join("ut.json");
    let bsp = output.join("bsp.json");
    let merged = output.join("merged.json");
    let binary = env!("CARGO_BIN_EXE_ibcmd-rs");
    for (input, database, run, target) in [
        (fixtures.join("ut-diff.json"), "ut", "ut-run", &ut),
        (fixtures.join("bsp-diff.json"), "bsp", "bsp-run", &bsp),
    ] {
        let command_output = Command::new(binary)
            .args([
                "source-diff-matrix",
                input.to_str().unwrap(),
                "--database",
                database,
                "--run-id",
                run,
                "--git-sha",
                "abc",
                "--full",
                "--output",
                target.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(command_output.status.success());
        assert!(command_output.stdout.len() < 2_000);
        assert!(!String::from_utf8_lossy(&command_output.stdout).contains("xml_signatures"));
    }
    assert!(
        Command::new(binary)
            .args([
                "source-diff-matrix-merge",
                ut.to_str().unwrap(),
                bsp.to_str().unwrap(),
                "--output",
                merged.to_str().unwrap()
            ])
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(read_parity_matrix(&merged).unwrap().runs.len(), 2);
    fs::remove_dir_all(output).unwrap();
}

#[test]
fn aggregate_invariant_fails_closed() {
    let mut matrix = build_parity_matrix(
        &fixture("ut-diff.json"),
        "ut".into(),
        "bad".into(),
        "a".into(),
        true,
    )
    .unwrap();
    matrix.aggregates.clear();
    assert!(validate_parity_matrix(&matrix).is_err());
}
