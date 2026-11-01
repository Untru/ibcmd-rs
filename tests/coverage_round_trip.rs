//! Offline round trips of the combined coverage configurations through
//! ibcmd-rs's own writers (#674).
//!
//! `coverage_corpus` reads the platform's `.cf` of each set. This suite starts
//! from the platform's XML of the corpus's all-in-one sets instead (the
//! `combined` configuration of each platform build, the `combined` extension
//! and the `combined-epf`/`combined-erf` external files), writes it with
//! ibcmd-rs into a store, reads it back and compares the result with that XML,
//! variant by variant:
//!
//! - `xml-cf-xml`: `cf bootstrap` of `native/` into a `.cf` (`.cfe`, `.epf`,
//!   `.erf`), then `cf export`;
//! - `xml-file-infobase-xml`: `infobase config import --no-verify --db-path`
//!   of `native/` into the empty file infobase the platform made
//!   (`tests/fixtures/file-infobase/empty.1CD.gz`), then
//!   `infobase config export --db-path` (configurations only).
//!
//! The class sets are left out on purpose: of the lines that the round
//! trips of every set reach and nothing else in the suite does, these sets
//! reach 98 % in a ninth of the run time (`docs/test-audit.md`).
//!
//! Every variant must come back byte for byte, except the known gaps listed
//! with their reason in `tests/fixtures/coverage/round-trip.json`. A route
//! that refuses a set as a whole names the files it cannot write; those
//! files are listed under `refused` and no other file may join them. A
//! variant that differs or is refused is listed under `gaps`. A listed entry
//! that comes back is reported, not failed, so it can leave the list.

use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use serde_json::Value;

use crate::coverage_corpus::{TempDirectory, Verdict, compare, coverage_sets, read_tree};

#[path = "common/coverage_paths.rs"]
mod coverage_paths;

const EXPECTATIONS: &str = "round-trip.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Route {
    CfBootstrap,
    FileInfobase,
}

impl Route {
    const ALL: [Route; 2] = [Route::CfBootstrap, Route::FileInfobase];

    fn key(self) -> &'static str {
        match self {
            Route::CfBootstrap => "xml-cf-xml",
            Route::FileInfobase => "xml-file-infobase-xml",
        }
    }
}

/// What one route made of one set.
#[derive(Debug)]
enum Outcome {
    /// The route refused the whole set: each file it named, with the first
    /// message for it.
    Refused(BTreeMap<String, String>),
    /// A verdict per variant; a refused variant carries its message.
    Compared(BTreeMap<String, (Verdict, Option<String>)>),
}

fn coverage_root() -> PathBuf {
    coverage_paths::root().join("coverage")
}

/// `8.5.1.1150/combined`, `8.5.1.1150/extensions/combined`: the set's place
/// in the corpus.
fn set_key(set: &Path) -> String {
    set.strip_prefix(coverage_root())
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

/// The corpus's all-in-one sets: each build's combined configuration and the
/// extension and external files that hold every case of their kind.
fn is_combined(set: &Path, manifest: &Value) -> bool {
    let name = set.file_name().unwrap().to_string_lossy();
    match manifest["kind"].as_str() {
        Some("combined") => true,
        Some("extension" | "external") => name.starts_with("combined"),
        _ => false,
    }
}

fn ibcmd_rs(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(args)
        .env("PATH", "")
        .output()
        .expect("run ibcmd-rs")
}

/// A file name as the corpus writes it, without the runner's path to it.
fn corpus_relative(text: &str) -> String {
    let root = coverage_root();
    let mut text = text.to_owned();
    for spelling in [
        root.to_string_lossy().into_owned(),
        root.to_string_lossy().replace('\\', "/"),
    ] {
        text = text.replace(&spelling, "<coverage>");
    }
    text
}

/// The files a refusing command names. `cf bootstrap` reports them in the
/// `message` of its JSON report, `infobase config import` on its `[ERROR]`
/// lines: one line per rejected row, `<file> <part>: <reason>`, under a
/// header line that ends with `:`. A refusal that names no file is kept
/// whole under `*`.
fn refused_files(output: &Output) -> BTreeMap<String, String> {
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    let report = [text(&output.stdout), text(&output.stderr)]
        .into_iter()
        .find_map(|text| serde_json::from_str::<Value>(&text[text.find('{')?..]).ok());
    let message = match report
        .as_ref()
        .and_then(|report| report["errors"].get(0))
        .and_then(|error| error["message"].as_str())
    {
        Some(message) => message.to_owned(),
        None => text(&output.stderr)
            .lines()
            .filter_map(|line| line.strip_prefix("[ERROR]"))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    let mut files = BTreeMap::new();
    let mut unnamed = Vec::new();
    for line in message.lines().map(str::trim) {
        if line.is_empty() || line.ends_with(':') {
            continue;
        }
        let file = line.split(' ').next().unwrap_or_default();
        if line.contains(": ") && (file.ends_with(".xml") || file.ends_with(".bsl")) {
            let reason = line.split_once(": ").unwrap().1;
            files
                .entry(file.to_owned())
                .or_insert_with(|| corpus_relative(reason));
        } else {
            unnamed.push(corpus_relative(line));
        }
    }
    if files.is_empty() {
        files.insert("*".to_owned(), unnamed.join(" / "));
    }
    files
}

/// The `--platform` of a set: its exact build when ibcmd-rs knows it, else
/// the release (8.3.27.2342 is written as 8.3.27).
fn platform(set_key: &str) -> String {
    let build = set_key.split('/').next().unwrap();
    if ibcmd_platform_spec::platform::parse(build).is_ok() {
        build.to_owned()
    } else {
        build.splitn(4, '.').take(3).collect::<Vec<_>>().join(".")
    }
}

/// `cf bootstrap` of `source` into `output`, then `cf export`; the tree, or
/// the files the bootstrap refused.
fn through_cf(
    source: &Path,
    output: &Path,
    base_free: bool,
    platform: &str,
    source_version: &str,
) -> Result<BTreeMap<String, Vec<u8>>, BTreeMap<String, String>> {
    let mut args: Vec<&std::ffi::OsStr> = vec!["cf".as_ref(), "bootstrap".as_ref()];
    if base_free {
        args.push("--base-free".as_ref());
    }
    args.extend([
        "--platform".as_ref(),
        platform.as_ref(),
        source.as_os_str(),
        output.as_os_str(),
    ]);
    let built = ibcmd_rs(&args);
    if !built.status.success() {
        return Err(refused_files(&built));
    }
    let tree = output.with_extension("export");
    match crate::coverage_corpus::cf_export(output, &tree, source_version) {
        Ok(exported) if exported.refusal.is_none() => Ok(exported.files),
        Ok(exported) => Err(BTreeMap::from([(
            "*".to_owned(),
            format!("cf export: {}", exported.refusal.unwrap()),
        )])),
        Err(message) => Err(BTreeMap::from([(
            "*".to_owned(),
            format!("cf export: {message}"),
        )])),
    }
}

/// The platform's empty file infobase (`tests/fixtures/file-infobase`).
fn empty_infobase(dir: &Path) -> PathBuf {
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(
        fs::File::open(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join("file-infobase")
                .join("empty.1CD.gz"),
        )
        .unwrap(),
    )
    .read_to_end(&mut bytes)
    .unwrap();
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("1Cv8.1CD"), bytes).unwrap();
    dir.to_path_buf()
}

/// `infobase config import` of `source` into an empty file infobase, then
/// `infobase config export`; the tree, or the files the import refused.
fn through_file_infobase(
    source: &Path,
    work: &Path,
) -> Result<BTreeMap<String, Vec<u8>>, BTreeMap<String, String>> {
    let db = empty_infobase(&work.join("db"));
    let mut db_path = std::ffi::OsString::from("--db-path=");
    db_path.push(&db);
    let imported = ibcmd_rs(&[
        "infobase".as_ref(),
        "config".as_ref(),
        "import".as_ref(),
        // Load what can be loaded: a file that would not come back is then
        // found missing or different by the comparison below, variant by
        // variant, rather than refusing the whole set.
        "--no-verify".as_ref(),
        &db_path,
        source.as_os_str(),
    ]);
    if !imported.status.success() {
        return Err(refused_files(&imported));
    }
    let tree = work.join("export");
    let exported = ibcmd_rs(&[
        "infobase".as_ref(),
        "config".as_ref(),
        "export".as_ref(),
        &db_path,
        tree.as_os_str(),
    ]);
    if !exported.status.success() {
        return Err(refused_files(&exported));
    }
    Ok(read_tree(&tree))
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

/// One route over one set; `None` when the route does not apply to it.
fn measure(set: &Path, manifest: &Value, route: Route, work: &Path) -> Option<Outcome> {
    let key = set_key(set);
    let platform = platform(&key);
    let source_version = manifest["evidence"]["source_version"].as_str().unwrap();
    let native_root = set.join("native");
    let native = read_tree(&native_root);
    let kind = manifest["kind"].as_str().unwrap_or("configuration");
    let variants = manifest["variants"].as_array().unwrap();
    let mut claimed = Default::default();
    let mut verdicts = BTreeMap::new();

    if matches!(kind, "external" | "extension") {
        if route != Route::CfBootstrap {
            return None;
        }
        // One file per variant, written and read on its own.
        for variant in variants {
            let name = variant["name"].as_str().unwrap().to_owned();
            if variant["status"] == "platform_refused" {
                verdicts.insert(name, (Verdict::PlatformRefused, None));
                continue;
            }
            let encoded = variant["file"]["path"].as_str().unwrap();
            let extension = Path::new(encoded.trim_end_matches(".b64"))
                .extension()
                .unwrap()
                .to_owned();
            let variant_work = work.join(&name);
            fs::create_dir_all(&variant_work).unwrap();
            let (source, base_free) = if kind == "extension" {
                (native_root.join(&name), false)
            } else {
                // An external file's tree is `<name>.xml` plus `<name>/`.
                let source = variant_work.join("source");
                fs::create_dir_all(&source).unwrap();
                let root_file = format!("{name}.xml");
                fs::copy(native_root.join(&root_file), source.join(&root_file)).unwrap();
                if native_root.join(&name).is_dir() {
                    copy_tree(&native_root.join(&name), &source.join(&name));
                }
                (source, true)
            };
            let output = variant_work.join(&name).with_extension(extension);
            let verdict = match through_cf(&source, &output, base_free, &platform, source_version) {
                Ok(files) => {
                    let files = if kind == "extension" {
                        files
                            .into_iter()
                            .map(|(path, bytes)| (format!("{name}/{path}"), bytes))
                            .collect()
                    } else {
                        files
                    };
                    (
                        compare(
                            variant,
                            &native,
                            &files,
                            &[name.clone()],
                            &mut claimed,
                            &key,
                        ),
                        None,
                    )
                }
                Err(files) => (
                    Verdict::Refused,
                    Some(
                        files
                            .iter()
                            .map(|(file, reason)| format!("{file}: {reason}"))
                            .collect::<Vec<_>>()
                            .join("; "),
                    ),
                ),
            };
            verdicts.insert(name, verdict);
        }
        return Some(Outcome::Compared(verdicts));
    }

    let ours = match route {
        Route::CfBootstrap => through_cf(
            &native_root,
            &work.join("set.cf"),
            true,
            &platform,
            source_version,
        ),
        Route::FileInfobase => through_file_infobase(&native_root, work),
    };
    let ours = match ours {
        Ok(ours) => ours,
        Err(files) => return Some(Outcome::Refused(files)),
    };
    let collection = manifest["collection"].as_str();
    for variant in variants {
        let name = variant["name"].as_str().unwrap().to_owned();
        if variant["status"] == "platform_refused" {
            verdicts.insert(name, (Verdict::PlatformRefused, None));
            continue;
        }
        let own = match collection {
            Some(collection) => format!("{collection}/{name}"),
            None => name.clone(),
        };
        let verdict = compare(variant, &native, &ours, &[own], &mut claimed, &key);
        verdicts.insert(name, (verdict, None));
    }
    Some(Outcome::Compared(verdicts))
}

/// Every (route, set) pair, measured on as many threads as the machine has:
/// each runs ibcmd-rs as child processes in a scratch directory of its own.
fn measure_all(sets: &[(PathBuf, Value)]) -> BTreeMap<(Route, String), Outcome> {
    let jobs: Vec<(Route, &(PathBuf, Value))> = Route::ALL
        .iter()
        .flat_map(|route| sets.iter().map(move |set| (*route, set)))
        .collect();
    let next = AtomicUsize::new(0);
    let results = Mutex::new(BTreeMap::new());
    let threads = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(jobs.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some((route, (set, manifest))) =
                    jobs.get(next.fetch_add(1, Ordering::Relaxed))
                {
                    let temp = TempDirectory::new();
                    if let Some(outcome) = measure(set, manifest, *route, &temp.0) {
                        results
                            .lock()
                            .unwrap()
                            .insert((*route, set_key(set)), outcome);
                    }
                }
            });
        }
    });
    results.into_inner().unwrap()
}

fn listed(expectation: Option<&Value>, field: &str) -> BTreeMap<String, String> {
    expectation
        .and_then(|value| value[field].as_object())
        .map(|entries| {
            entries
                .iter()
                .map(|(name, reason)| (name.clone(), reason.as_str().unwrap().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

/// The failures of `measured` against `expected`, and the report lines.
fn check(
    measured: &BTreeMap<(Route, String), Outcome>,
    expected: &Value,
) -> (Vec<String>, Vec<String>) {
    let mut failures = Vec::new();
    let mut report = Vec::new();
    for route in Route::ALL {
        let route_key = route.key();
        let expected_sets = expected[route_key].as_object().cloned().unwrap_or_default();
        let (mut sets, mut refused_sets, mut variants, mut equal, mut gaps) = (0, 0, 0, 0, 0);
        for ((measured_route, key), outcome) in measured {
            if *measured_route != route {
                continue;
            }
            sets += 1;
            let expectation = expected_sets.get(key);
            let listed_refused = listed(expectation, "refused");
            let listed_gaps = listed(expectation, "gaps");
            match outcome {
                Outcome::Refused(files) => {
                    refused_sets += 1;
                    report.push(format!(
                        "{route_key} {key}: refused, {} files named",
                        files.len()
                    ));
                    for (file, reason) in files {
                        if !listed_refused.contains_key(file) {
                            failures.push(format!(
                                "{route_key} {key}: {file} is refused and not listed: {reason}"
                            ));
                        }
                    }
                    for (file, reason) in &listed_refused {
                        if !files.contains_key(file) {
                            report.push(format!(
                                "{route_key} {key}: no longer refused (remove from refused): {file} ({reason})"
                            ));
                        }
                    }
                }
                Outcome::Compared(verdicts) => {
                    if !listed_refused.is_empty() {
                        report.push(format!(
                            "{route_key} {key}: listed as refused but now round-trips; list its gaps instead"
                        ));
                        continue;
                    }
                    for (variant, (verdict, message)) in verdicts {
                        if *verdict == Verdict::PlatformRefused {
                            continue;
                        }
                        variants += 1;
                        match (verdict, listed_gaps.get(variant)) {
                            (Verdict::Equal, None) => equal += 1,
                            (Verdict::Equal, Some(reason)) => {
                                equal += 1;
                                report.push(format!(
                                    "{route_key} {key}: newly equal (remove from gaps): {variant} ({reason})"
                                ));
                            }
                            (_, Some(_)) => gaps += 1,
                            (verdict, None) => {
                                let detail = match verdict {
                                    Verdict::Different(first) => first.clone(),
                                    _ => message.clone().unwrap_or_default(),
                                };
                                failures.push(format!(
                                    "{route_key} {key}: {variant} does not round-trip: {detail}"
                                ));
                            }
                        }
                    }
                    for variant in listed_gaps.keys() {
                        if !verdicts.contains_key(variant) {
                            failures.push(format!(
                                "{route_key} {key}: gaps name {variant}, which the set does not have"
                            ));
                        }
                    }
                }
            }
        }
        for key in expected_sets.keys() {
            if !measured.contains_key(&(route, key.clone())) {
                failures.push(format!(
                    "{route_key}: {key} is listed but is no combined set of this route"
                ));
            }
        }
        report.push(format!(
            "{route_key}: sets {sets}, refused {refused_sets}; variants compared {variants}, equal {equal}, listed gaps {gaps}"
        ));
    }
    (failures, report)
}

#[test]
fn combined_coverage_sets_round_trip_through_ibcmd_rs_writers() {
    let sets: Vec<(PathBuf, Value)> = coverage_sets()
        .into_iter()
        .map(|set| {
            let manifest =
                serde_json::from_slice(&fs::read(set.join("manifest.json")).unwrap()).unwrap();
            (set, manifest)
        })
        .filter(|(set, manifest)| is_combined(set, manifest))
        .collect();
    assert!(
        !sets.is_empty(),
        "no combined coverage set in the pinned corpus"
    );
    let measured = measure_all(&sets);
    let expected = serde_json::from_slice(
        &fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join("coverage")
                .join(EXPECTATIONS),
        )
        .unwrap(),
    )
    .unwrap();
    let (failures, report) = check(&measured, &expected);
    for line in &report {
        println!("{line}");
    }
    assert!(
        failures.is_empty(),
        "coverage round trips regressed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn round_trip_expectations_fail_on_regressions_and_report_fixes() {
    let equal = |name: &str| (name.to_owned(), (Verdict::Equal, None));
    let measured = BTreeMap::from([
        (
            (Route::CfBootstrap, "p/a".to_owned()),
            Outcome::Compared(BTreeMap::from([
                equal("Same"),
                equal("Fixed"),
                (
                    "Broken".to_owned(),
                    (
                        Verdict::Different("Broken.xml (1 bytes, native 2)".into()),
                        None,
                    ),
                ),
                ("Skipped".to_owned(), (Verdict::PlatformRefused, None)),
            ])),
        ),
        (
            (Route::CfBootstrap, "p/b".to_owned()),
            Outcome::Refused(BTreeMap::from([
                ("Known.xml".to_owned(), "listed".to_owned()),
                ("New.xml".to_owned(), "a new refusal".to_owned()),
            ])),
        ),
        (
            (Route::FileInfobase, "p/a".to_owned()),
            Outcome::Refused(BTreeMap::from([(
                "*".to_owned(),
                "import failed".to_owned(),
            )])),
        ),
    ]);
    let expected = serde_json::json!({
        "xml-cf-xml": {
            "p/a": {"gaps": {"Fixed": "#1", "Gone": "#2"}},
            "p/b": {"refused": {"Known.xml": "#3", "Healed.xml": "#4"}},
            "p/c": {"refused": {"*": "#5"}}
        },
        "xml-file-infobase-xml": {}
    });
    let (failures, report) = check(&measured, &expected);
    assert_eq!(
        failures,
        [
            "xml-cf-xml p/a: Broken does not round-trip: Broken.xml (1 bytes, native 2)",
            "xml-cf-xml p/a: gaps name Gone, which the set does not have",
            "xml-cf-xml p/b: New.xml is refused and not listed: a new refusal",
            "xml-cf-xml: p/c is listed but is no combined set of this route",
            "xml-file-infobase-xml p/a: * is refused and not listed: import failed",
        ]
    );
    for line in [
        "xml-cf-xml p/a: newly equal (remove from gaps): Fixed (#1)",
        "xml-cf-xml p/b: no longer refused (remove from refused): Healed.xml (#4)",
        "xml-cf-xml: sets 2, refused 1; variants compared 3, equal 2, listed gaps 0",
    ] {
        assert!(report.iter().any(|reported| reported == line), "{line}");
    }
}

#[test]
fn refusals_are_split_into_the_files_they_name() {
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;
    let output = |stdout: &str, stderr: &str| Output {
        status: std::process::ExitStatus::from_raw(1),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    };
    let bootstrap = serde_json::json!({"errors": [{"message":
        "2 rows of the tree cannot be compiled without a base:\nA/B.xml descriptor 1: first\nA/B.xml kind body 2: second\nConfiguration.xml kind body : third"}]});
    assert_eq!(
        refused_files(&output(&bootstrap.to_string(), "")),
        BTreeMap::from([
            ("A/B.xml".to_owned(), "first".to_owned()),
            ("Configuration.xml".to_owned(), "third".to_owned()),
        ])
    );
    let import = "[INFO] start\n[ERROR] 1 rows of the tree cannot be compiled without a base:\n[ERROR] C/D.xml descriptor 3: fourth\n[ERROR] Импорт конфигурации из XML завершен с ошибкой\n";
    assert_eq!(
        refused_files(&output("", import)),
        BTreeMap::from([("C/D.xml".to_owned(), "fourth".to_owned())])
    );
    assert_eq!(
        refused_files(&output("", "[ERROR] disk full\n")),
        BTreeMap::from([("*".to_owned(), "disk full".to_owned())])
    );
}
