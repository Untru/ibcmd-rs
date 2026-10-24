//! `cargo xtask`: the repository's verification loop.
//!
//! People, coding agents and agent hooks run the same commands through this
//! one entry point. `ci` mirrors the required merge checks of
//! `.github/workflows/offline-e2e.yml` (docs/release-criteria.md) as far as
//! they run on one machine; change the two together.

use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::Instant;

const USAGE: &str = "\
usage: cargo xtask <task> [--keep-going] [--base <rev>]

  check         the inner loop: format, lint, compile the root package, then
                test the crates and root suites changed since --base
  ci            the required merge checks that run on this machine
  affected      print the test commands for the files changed since --base
  hook-rustfmt  format the .rs file named in an agent PostToolUse payload
  hook-stop     the lint steps of `check` for an agent Stop hook

--base defaults to the merge base of HEAD and origin/master.
Logs of every step: target/xtask/<step>.log";

const ROOT_PACKAGE: &str = "ibcmd-rs";
const MERGE_WORKFLOW: &str = ".github/workflows/offline-e2e.yml";
const TAIL_LINES: usize = 40;

const NOTE_ROOT_SRC: &str = "src/ changed: run the root suites that cover it \
     (`cargo test --locked -p ibcmd-rs --test <suite> --no-default-features`, \
     unit tests with `--lib <module>`), then `cargo xtask ci`";
const NOTE_SHARED_TESTS: &str =
    "tests/common or tests/fixtures changed: run the suites that use them, or `cargo xtask ci`";
const NOTE_GATES: &str =
    "manifests, workflows, tools, scripts or OpenSpec changed: run `cargo xtask ci`";

fn main() -> ExitCode {
    let mut task = None;
    let mut keep_going = false;
    let mut base = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--keep-going" => keep_going = true,
            "--base" => match args.next() {
                Some(rev) => base = Some(rev),
                None => return usage_error("--base needs a revision"),
            },
            "-h" | "--help" | "help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ if task.is_none() && !arg.starts_with('-') => task = Some(arg),
            _ => return usage_error(&format!("unexpected argument `{arg}`")),
        }
    }
    let root = repository_root();
    match task.as_deref() {
        Some("check") => check(&root, base.as_deref(), keep_going),
        Some("ci") => finish(
            run_steps(&root, &ci_steps(&root), keep_going),
            "ci",
            CI_NOT_RUN,
        ),
        Some("affected") => print_affected(&root, base.as_deref()),
        Some("hook-rustfmt") => hook_rustfmt(&root),
        Some("hook-stop") => hook_stop(&root),
        Some(other) => usage_error(&format!("unknown task `{other}`")),
        None => usage_error("no task given"),
    }
}

fn usage_error(message: &str) -> ExitCode {
    eprintln!("xtask: {message}\n\n{USAGE}");
    ExitCode::from(2)
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the repository root")
        .to_path_buf()
}

// ---------------------------------------------------------------- steps --

struct Step {
    name: String,
    program: String,
    args: Vec<String>,
    env: Vec<(&'static str, &'static str)>,
    hint: Option<&'static str>,
    /// Why the step cannot run here. It is reported, never silently dropped.
    missing: Option<String>,
}

impl Step {
    fn new(name: impl Into<String>, program: &str, args: &[&str]) -> Self {
        Step {
            name: name.into(),
            program: program.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            env: Vec::new(),
            hint: None,
            missing: None,
        }
    }

    fn cargo(name: impl Into<String>, args: &[&str]) -> Self {
        let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
        Step::new(name, &cargo, args)
    }

    fn hint(mut self, hint: &'static str) -> Self {
        self.hint = Some(hint);
        self
    }

    fn env(mut self, key: &'static str, value: &'static str) -> Self {
        self.env.push((key, value));
        self
    }

    fn needs_program(mut self) -> Self {
        if !available(&self.program) {
            self.missing = Some(format!("`{}` not found", self.program));
        }
        self
    }
}

fn available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn format_step() -> Step {
    Step::cargo("format", &["fmt", "--all", "--check"]).hint("run `cargo fmt --all`")
}

/// The CI lint gate: the root package still carries clippy debt, so it is
/// compile-checked instead (`lint_steps`).
fn clippy_step() -> Step {
    Step::cargo(
        "clippy (workspace crates)",
        &[
            "clippy",
            "--locked",
            "--workspace",
            "--exclude",
            ROOT_PACKAGE,
            "--all-targets",
            "--message-format=short",
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn lint_steps() -> Vec<Step> {
    vec![
        format_step(),
        clippy_step(),
        Step::cargo(
            "check (root package)",
            &[
                "check",
                "--locked",
                "-p",
                ROOT_PACKAGE,
                "--all-targets",
                "--no-default-features",
                "--message-format=short",
            ],
        ),
    ]
}

const CI_NOT_RUN: &str = "not run locally: the release build, native micro-corpus parity, \
     SBOM and release audit of the offline matrix (docs/release-criteria.md)";

fn ci_steps(root: &Path) -> Vec<Step> {
    let pwsh = |name: &str, script: &str, self_test: bool| {
        let mut args = vec!["-NoProfile", "-File", script, "-RepositoryRoot", "."];
        if self_test {
            args.push("-SelfTest");
        }
        Step::new(name, "pwsh", &args).needs_program()
    };
    let python = if cfg!(windows) { "python" } else { "python3" };

    let mut steps = vec![format_step()];
    steps.push(pwsh(
        "EDT corpus governance self-test",
        "tools/validate-edt-corpus.ps1",
        true,
    ));
    steps.push(pwsh(
        "EDT corpus governance",
        "tools/validate-edt-corpus.ps1",
        false,
    ));
    steps.push(pwsh(
        "physical-adapter policy",
        "tools/validate-physical-adapter-policy.ps1",
        false,
    ));
    steps.push(pwsh(
        "physical-adapter policy self-test",
        "tools/validate-physical-adapter-policy.ps1",
        true,
    ));
    steps.extend(openspec_steps(root));
    steps.push(clippy_step());
    for script in [
        "edt_oracle_test",
        "edt_warm_context_test",
        "edt_release_audit_test",
        "linux_abi_test",
    ] {
        let path = format!("tests/scripts/{script}.py");
        steps.push(Step::new(format!("python {script}"), python, &[path.as_str()]).needs_program());
    }
    let provenance_fixtures = root.join("target/xtask-fixtures/observer-provenance");
    let provenance_fixtures = provenance_fixtures.to_string_lossy();
    steps.push(
        Step::new(
            "python form_observer_provenance_test",
            python,
            &[
                "tests/scripts/form_observer_provenance_test.py",
                provenance_fixtures.as_ref(),
            ],
        )
        .env("IBCMD_TEST_KEEP_FIXTURES", "1")
        .needs_program(),
    );
    steps.push(Step::cargo(
        "test -p ibcmd-schema",
        &["test", "--locked", "-p", "ibcmd-schema"],
    ));
    steps.push(Step::cargo(
        "check (all targets)",
        &[
            "check",
            "--locked",
            "--workspace",
            "--all-targets",
            "--no-default-features",
            "--message-format=short",
        ],
    ));
    steps.push(Step::cargo(
        "test (workspace crates)",
        &["test", "--locked", "--workspace", "--exclude", ROOT_PACKAGE],
    ));
    steps.push(Step::cargo(
        "test (root integration suites)",
        &[
            "test",
            "--locked",
            "-p",
            ROOT_PACKAGE,
            "--test",
            "*",
            "--no-default-features",
        ],
    ));
    steps.push(Step::cargo(
        "test (root configuration contracts)",
        &[
            "test",
            "--locked",
            "-p",
            ROOT_PACKAGE,
            "--lib",
            "configuration_",
            "--no-default-features",
        ],
    ));
    for (name, filter) in [
        ("bounded Form field-map observation", "field_map_observer"),
        ("bounded Form query identity", "query_identity_observer"),
        (
            "shared Form observer schema",
            "form_observer_shared_schema_tests",
        ),
    ] {
        steps.push(
            Step::cargo(
                format!("test ({name})"),
                &[
                    "test",
                    "--locked",
                    "-p",
                    ROOT_PACKAGE,
                    "--lib",
                    filter,
                    "--no-default-features",
                ],
            )
            .env("IBCMD_TEST_KEEP_FIXTURES", "1"),
        );
    }
    steps
}

/// The strict OpenSpec validations exactly as the merge workflow lists them,
/// so a change validated there is validated here without a second list.
fn openspec_steps(root: &Path) -> Vec<Step> {
    let npx = if cfg!(windows) { "npx.cmd" } else { "npx" };
    let workflow = fs::read_to_string(root.join(MERGE_WORKFLOW)).unwrap_or_default();
    let mut steps = Vec::new();
    for line in workflow.lines() {
        let Some(start) = line.find("@fission-ai/openspec@") else {
            continue;
        };
        let package = line[start..].split_whitespace().next().unwrap_or_default();
        let Some(change) = line
            .split(" validate ")
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
        else {
            continue;
        };
        steps.push(
            Step::new(
                format!("OpenSpec {change}"),
                npx,
                &[
                    "--yes",
                    package,
                    "validate",
                    change,
                    "--strict",
                    "--no-interactive",
                ],
            )
            .env("CI", "true")
            .env("OPENSPEC_TELEMETRY", "0")
            .env("NPM_CONFIG_AUDIT", "false")
            .env("NPM_CONFIG_FUND", "false")
            .needs_program(),
        );
    }
    if steps.is_empty() {
        let mut step = Step::new("OpenSpec", npx, &[]);
        step.missing = Some(format!("no OpenSpec validation found in {MERGE_WORKFLOW}"));
        steps.push(step);
    }
    steps
}

// --------------------------------------------------------------- runner --

struct Report {
    passed: usize,
    failed: Vec<String>,
    skipped: Vec<String>,
    not_run: usize,
    /// Failing step names with the tail of their logs, for hook feedback.
    feedback: String,
}

fn run_steps(root: &Path, steps: &[Step], keep_going: bool) -> Report {
    let log_dir = root.join("target").join("xtask");
    let _ = fs::create_dir_all(&log_dir);
    let mut report = Report {
        passed: 0,
        failed: Vec::new(),
        skipped: Vec::new(),
        not_run: 0,
        feedback: String::new(),
    };
    for step in steps {
        if !report.failed.is_empty() && !keep_going {
            report.not_run += 1;
            continue;
        }
        if let Some(reason) = &step.missing {
            eprintln!("SKIP {} ({reason}; CI still runs it)", step.name);
            report.skipped.push(step.name.clone());
            continue;
        }
        let log = log_dir.join(format!("{}.log", slug(&step.name)));
        let shown_log = log.strip_prefix(root).unwrap_or(&log).display().to_string();
        let started = Instant::now();
        let outcome = run_logged(root, step, &log);
        let seconds = started.elapsed().as_secs();
        match outcome {
            Ok(true) => {
                eprintln!("PASS {} ({seconds}s)", step.name);
                report.passed += 1;
            }
            Ok(false) => {
                let mut text = format!("FAIL {} ({seconds}s), full log: {shown_log}\n", step.name);
                for line in tail(&log) {
                    text.push_str("  | ");
                    text.push_str(&line);
                    text.push('\n');
                }
                if let Some(hint) = step.hint {
                    text.push_str(&format!("  hint: {hint}\n"));
                }
                eprint!("{text}");
                report.feedback.push_str(&text);
                report.failed.push(step.name.clone());
            }
            Err(error) => {
                let text = format!(
                    "FAIL {}: could not start `{}`: {error}\n",
                    step.name, step.program
                );
                eprint!("{text}");
                report.feedback.push_str(&text);
                report.failed.push(step.name.clone());
            }
        }
    }
    report
}

fn run_logged(root: &Path, step: &Step, log: &Path) -> io::Result<bool> {
    let mut out = fs::File::create(log)?;
    writeln!(out, "$ {} {}", step.program, step.args.join(" "))?;
    let err = out.try_clone()?;
    let status = Command::new(&step.program)
        .args(&step.args)
        .envs(step.env.iter().copied())
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .status()?;
    Ok(status.success())
}

fn tail(log: &Path) -> Vec<String> {
    let bytes = fs::read(log).unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(TAIL_LINES);
    lines[start..]
        .iter()
        .map(|line| (*line).to_owned())
        .collect()
}

fn slug(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_owned()
}

fn finish(report: Report, task: &str, footnote: &str) -> ExitCode {
    let mut summary = format!("{task}: {} passed", report.passed);
    if !report.failed.is_empty() {
        summary.push_str(&format!(
            ", {} FAILED ({})",
            report.failed.len(),
            report.failed.join(", ")
        ));
    }
    if report.not_run > 0 {
        summary.push_str(&format!(
            ", {} not run after the failure (--keep-going runs them)",
            report.not_run
        ));
    }
    if !report.skipped.is_empty() {
        summary.push_str(&format!(
            ", {} SKIPPED ({})",
            report.skipped.len(),
            report.skipped.join(", ")
        ));
    }
    eprintln!("{summary}");
    if !footnote.is_empty() {
        eprintln!("{footnote}");
    }
    if report.failed.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

// ------------------------------------------------------- changed files --

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(["-c", "core.quotepath=off"])
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn merge_base(root: &Path, base: Option<&str>) -> Result<String, String> {
    if let Some(base) = base {
        return Ok(base.to_owned());
    }
    git(root, &["merge-base", "HEAD", "origin/master"])
        .or_else(|_| git(root, &["merge-base", "HEAD", "master"]))
        .map(|sha| sha.trim().to_owned())
}

/// Committed, staged, unstaged and untracked paths that differ from `base`.
fn changed_files(root: &Path, base: &str) -> Result<Vec<String>, String> {
    let mut files = BTreeSet::new();
    for listing in [
        git(root, &["diff", "--name-only", base])?,
        git(root, &["ls-files", "--others", "--exclude-standard"])?,
    ] {
        files.extend(
            listing
                .lines()
                .filter(|line| !line.is_empty())
                .map(str::to_owned),
        );
    }
    Ok(files.into_iter().collect())
}

#[derive(Default)]
struct Affected {
    packages: BTreeSet<String>,
    root_suites: BTreeSet<String>,
    notes: BTreeSet<&'static str>,
}

fn affected(root: &Path, files: &[String]) -> Affected {
    let mut affected = Affected::default();
    for file in files {
        let parts: Vec<&str> = file.split('/').collect();
        match parts.as_slice() {
            ["crates", dir, ..] => {
                if let Some(name) = package_name(&root.join("crates").join(dir).join("Cargo.toml"))
                {
                    affected.packages.insert(name);
                }
            }
            ["tests", "scripts", ..] => {
                affected.notes.insert(NOTE_GATES);
            }
            ["tests", suite] if suite.ends_with(".rs") && root.join(file).is_file() => {
                affected
                    .root_suites
                    .insert(suite.trim_end_matches(".rs").to_owned());
            }
            ["tests", ..] => {
                affected.notes.insert(NOTE_SHARED_TESTS);
            }
            ["src", ..] => {
                affected.notes.insert(NOTE_ROOT_SRC);
            }
            ["Cargo.toml" | "Cargo.lock"] | [".github" | "tools" | "scripts" | "openspec", ..] => {
                affected.notes.insert(NOTE_GATES);
            }
            _ => {}
        }
    }
    affected
}

fn affected_test_steps(affected: &Affected) -> Vec<Step> {
    let mut steps = Vec::new();
    for package in &affected.packages {
        steps.push(Step::cargo(
            format!("test -p {package}"),
            &["test", "--locked", "-p", package],
        ));
    }
    for suite in &affected.root_suites {
        steps.push(Step::cargo(
            format!("test --test {suite}"),
            &[
                "test",
                "--locked",
                "-p",
                ROOT_PACKAGE,
                "--test",
                suite,
                "--no-default-features",
            ],
        ));
    }
    steps
}

/// The `name` of the `[package]` table; a line scan is enough for this
/// repository's manifests and keeps xtask free of dependencies.
fn package_name(manifest: &Path) -> Option<String> {
    manifest_value(manifest, "name")
}

fn manifest_value(manifest: &Path, key: &str) -> Option<String> {
    let text = fs::read_to_string(manifest).ok()?;
    let mut in_package = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package
            && let Some((name, value)) = line.split_once('=')
            && name.trim() == key
        {
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }
    None
}

fn print_affected(root: &Path, base: Option<&str>) -> ExitCode {
    let changed = merge_base(root, base)
        .and_then(|base| changed_files(root, &base).map(|files| (base, files)));
    let (base, files) = match changed {
        Ok(changed) => changed,
        Err(error) => {
            eprintln!("xtask: {error}");
            return ExitCode::FAILURE;
        }
    };
    let affected = affected(root, &files);
    println!("# {} files changed since {}", files.len(), short(&base));
    for step in affected_test_steps(&affected) {
        println!("cargo {}", step.args.join(" "));
    }
    for note in &affected.notes {
        println!("# {note}");
    }
    ExitCode::SUCCESS
}

fn short(rev: &str) -> &str {
    if rev.len() == 40 { &rev[..10] } else { rev }
}

// ---------------------------------------------------------------- tasks --

fn check(root: &Path, base: Option<&str>, keep_going: bool) -> ExitCode {
    let mut steps = lint_steps();
    let mut notes = BTreeSet::new();
    match merge_base(root, base).and_then(|base| changed_files(root, &base)) {
        Ok(files) => {
            let affected = affected(root, &files);
            steps.extend(affected_test_steps(&affected));
            notes = affected.notes;
        }
        Err(error) => {
            eprintln!("xtask: changed files unknown ({error}); running the lint steps only")
        }
    }
    let report = run_steps(root, &steps, keep_going);
    let footnote: Vec<String> = notes.iter().map(|note| format!("next: {note}")).collect();
    finish(report, "check", &footnote.join("\n"))
}

/// PostToolUse: format the edited Rust file in place. Exit 2 hands a rustfmt
/// parse error back to the agent; anything else passes silently.
fn hook_rustfmt(root: &Path) -> ExitCode {
    let payload = read_stdin();
    let Some(file) = json_string(&payload, "file_path") else {
        return ExitCode::SUCCESS;
    };
    let Ok(path) = fs::canonicalize(&file) else {
        return ExitCode::SUCCESS;
    };
    let inside = fs::canonicalize(root).is_ok_and(|root| path.starts_with(root));
    let vendored = path.components().any(|part| part.as_os_str() == "vendor");
    if !inside || vendored || path.extension().is_none_or(|ext| ext != "rs") {
        return ExitCode::SUCCESS;
    }
    let edition = path
        .ancestors()
        .skip(1)
        .map(|dir| dir.join("Cargo.toml"))
        .find(|manifest| manifest.is_file())
        .and_then(|manifest| manifest_value(&manifest, "edition"))
        .unwrap_or_else(|| "2024".to_owned());
    match Command::new("rustfmt")
        .args(["--edition", &edition])
        .arg(&path)
        .output()
    {
        Ok(output) if !output.status.success() => {
            eprintln!(
                "rustfmt could not format {file}:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            ExitCode::from(2)
        }
        // Formatted, or no rustfmt here; the format check of `check` still applies.
        _ => ExitCode::SUCCESS,
    }
}

/// Stop: when Rust sources differ from the base, run the lint steps of
/// `check` before the agent may finish. Exit 2 returns the failures to it.
/// A passing state is remembered so an unchanged tree is not linted twice,
/// and a repeated stop on the same failing state is let through so the
/// agent can explain instead of looping.
fn hook_stop(root: &Path) -> ExitCode {
    let payload = read_stdin();
    let retry = json_bool(&payload, "stop_hook_active").unwrap_or(false);
    let Ok(base) = merge_base(root, None) else {
        return ExitCode::SUCCESS;
    };
    let Some(state) = rust_state(root, &base) else {
        return ExitCode::SUCCESS;
    };
    let state_dir = root.join("target").join("xtask");
    let passed = state_dir.join("stop-passed");
    let failed = state_dir.join("stop-failed");
    let seen = |path: &Path| fs::read_to_string(path).is_ok_and(|saved| saved == state);
    if seen(&passed) || (retry && seen(&failed)) {
        return ExitCode::SUCCESS;
    }
    let report = run_steps(root, &lint_steps(), false);
    let _ = fs::create_dir_all(&state_dir);
    if report.failed.is_empty() {
        let _ = fs::write(&passed, &state);
        return ExitCode::SUCCESS;
    }
    let _ = fs::write(&failed, &state);
    eprintln!(
        "cargo xtask check found problems in your change:\n{}\
         Fix them before finishing. A failure counts as pre-existing only after you \
         reproduce it on {} and say so.",
        report.feedback,
        short(&base)
    );
    ExitCode::from(2)
}

/// A fingerprint of the Rust sources and manifests that differ from `base`,
/// or `None` when there are none.
fn rust_state(root: &Path, base: &str) -> Option<String> {
    let relevant =
        |file: &str| file.ends_with(".rs") || file.ends_with("Cargo.toml") || file == "Cargo.lock";
    let files: Vec<String> = changed_files(root, base)
        .ok()?
        .into_iter()
        .filter(|file| relevant(file))
        .collect();
    if files.is_empty() {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    for file in &files {
        file.hash(&mut hasher);
        fs::read(root.join(file))
            .unwrap_or_default()
            .hash(&mut hasher);
    }
    Some(format!("{base} {:016x}", hasher.finish()))
}

// ------------------------------------------------------- hook payloads --

fn read_stdin() -> String {
    let mut payload = String::new();
    let _ = io::stdin().read_to_string(&mut payload);
    payload
}

/// The first string value of `key` in a JSON document. Hook payloads are
/// small and flat enough that this avoids a JSON dependency.
fn json_string(json: &str, key: &str) -> Option<String> {
    let rest = json_value_start(json, key)?.strip_prefix('"')?;
    let mut value = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(value),
            '\\' => match chars.next()? {
                'n' => value.push('\n'),
                't' => value.push('\t'),
                'r' => value.push('\r'),
                'b' => value.push('\u{8}'),
                'f' => value.push('\u{c}'),
                'u' => {
                    let code: String = chars.by_ref().take(4).collect();
                    value.push(char::from_u32(u32::from_str_radix(&code, 16).ok()?)?);
                }
                other => value.push(other),
            },
            c => value.push(c),
        }
    }
    None
}

fn json_bool(json: &str, key: &str) -> Option<bool> {
    let rest = json_value_start(json, key)?;
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn json_value_start<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let quoted = format!("\"{key}\"");
    let after = &json[json.find(&quoted)? + quoted.len()..];
    Some(after.trim_start().strip_prefix(':')?.trim_start())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_hook_payload_fields() {
        let payload = r#"{"tool_name":"Edit","tool_input":{"file_path":"C:\\src\\\u0444.rs"},"stop_hook_active": true}"#;
        assert_eq!(
            json_string(payload, "file_path").as_deref(),
            Some("C:\\src\\ф.rs")
        );
        assert_eq!(json_bool(payload, "stop_hook_active"), Some(true));
        assert_eq!(json_string(payload, "missing"), None);
    }

    #[test]
    fn maps_changed_files_to_test_targets() {
        let root = repository_root();
        let files: Vec<String> = [
            "crates/ibcmd-filedb/src/lib.rs",
            "tests/file_infobase.rs",
            "tests/fixtures/file-infobase/x",
            "src/main.rs",
            "Cargo.lock",
            "README.md",
        ]
        .map(str::to_owned)
        .to_vec();
        let affected = affected(&root, &files);
        assert_eq!(
            affected.packages.iter().collect::<Vec<_>>(),
            ["ibcmd-filedb"]
        );
        assert_eq!(
            affected.root_suites.iter().collect::<Vec<_>>(),
            ["file_infobase"]
        );
        assert!(affected.notes.contains(NOTE_ROOT_SRC));
        assert!(affected.notes.contains(NOTE_SHARED_TESTS));
        assert!(affected.notes.contains(NOTE_GATES));
    }

    #[test]
    fn mirrors_the_merge_workflow_openspec_list() {
        let steps = openspec_steps(&repository_root());
        assert!(
            steps
                .iter()
                .any(|step| step.name == "OpenSpec build-offline-converter")
        );
        assert!(
            steps
                .iter()
                .all(|step| step.args.contains(&"--strict".to_owned()))
        );
    }

    #[test]
    fn slugs_are_file_names() {
        assert_eq!(slug("clippy (workspace crates)"), "clippy-workspace-crates");
    }
}
