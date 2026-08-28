//! Reverse-direction gate against real platform evidence:
//! native XML tree -> `cf bootstrap` -> CF -> `cf export` -> XML tree.
//!
//! The forward direction (`cf export` of a retained native CF) is already
//! byte-exact against 1C 8.3.27.2214 captures.  This file measures the reverse
//! direction, which has never been gated, so that "can ibcmd-rs rebuild what it
//! can read?" stops being an open question and becomes a number.
//!
//! # Where the reference tree comes from
//!
//! The reference trees are produced here by exporting the bundled retained CF
//! (`tests/fixtures/native-evidence/8.3.27.2214/<corpus>/configuration.cf.b64`),
//! and every produced file is then pinned against the sha256 of the
//! corresponding file in the *platform's own* `config export` capture
//! (`scratchpad/evidence-batch/session12/{T1,T2,T3}/native-tree-manifest.json`,
//! 39 files across three corpora, transcribed into [`CORPORA`] below).  That
//! keeps the gate anchored to platform bytes while remaining runnable from a
//! clean checkout with no extra fixture files: if the export direction ever
//! regresses, this test says so before it starts measuring the reverse
//! direction, instead of quietly comparing our own output against itself.
//!
//! # Why this test is `#[ignore]`d
//!
//! See the attribute comment on [`native_tree_rebuilds_into_an_identical_tree`]
//! for the exact remaining blockers and the condition for re-enabling it.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use sha2::{Digest, Sha256};

/// One retained corpus: fixture directory, plus the platform's own export
/// inventory for that corpus, pinned by sha256.
struct Corpus {
    label: &'static str,
    fixture: &'static str,
    /// sha256 of the decoded `configuration.cf.b64`, checked so a swapped
    /// fixture cannot silently change what is being measured.  Pinned here
    /// rather than read back from each corpus `manifest.json`, because those
    /// manifests record it under corpus-specific keys
    /// (`retained.configuration.sha256`, `rounds.round_2_cf_sha256`,
    /// `configuration_cf.decoded_sha256`, ...).
    configuration_cf_sha256: &'static str,
    /// `(relative path, sha256)` of every file in the platform's native export
    /// tree, transcribed from `native-tree-manifest.json`.
    native_tree: &'static [(&'static str, &'static str)],
}

const CORPORA: &[Corpus] = &[
    Corpus {
        label: "T1",
        fixture: "dcs-area-style-item-uuid",
        configuration_cf_sha256: "bd64046b8e86ca9db7aa2bd6c74f2255d5c8efa2db580f249a1cf65bf1da1e49",
        native_tree: &[
            (
                "ConfigDumpInfo.xml",
                "0fe1a64f9e11fe698cf9188b429f7fbbe8c46b27b94252f380f7255463d9e310",
            ),
            (
                "Configuration.xml",
                "193e665b81eaa3fbb390644860ef0bfaf3d6f48bbe852d7a8fb33d33dd3be24f",
            ),
            (
                "Ext/ClientApplicationInterface.xml",
                "0a84b26395fff630f14b8f18797442a56711213444fcd8ba69d28f5e42c100e8",
            ),
            (
                "Languages/Русский.xml",
                "ddfd511fb5397985bb76298eea53c970dd647e10c40c24e2a0e7a4ca66aae250",
            ),
            (
                "Reports/DcsCorpus.xml",
                "d3aa54cbac8d42f6e3a835aae76606a301a3a3cb175f958985ba661a58de9650",
            ),
            (
                "Reports/DcsCorpus/Ext/ManagerModule.bsl",
                "f1945cd6c19e56b3c1c78943ef5ec18116907a4ca1efc40a57d48ab1db7adfc5",
            ),
            (
                "Reports/DcsCorpus/Ext/ObjectModule.bsl",
                "f1945cd6c19e56b3c1c78943ef5ec18116907a4ca1efc40a57d48ab1db7adfc5",
            ),
            (
                "Reports/DcsCorpus/Templates/MainSchema.xml",
                "92d72ca2e6fcedf3b0b4e6099b09a4fd31e365766b5d5df9806bca250faab62f",
            ),
            (
                "Reports/DcsCorpus/Templates/MainSchema/Ext/Template.xml",
                "98f1857d3424198275cc35834a6635c28623568aae8d01a95cb5e220f91b818f",
            ),
            (
                "StyleItems/CorpusAccent.xml",
                "a62b959471790db01992201232f08238fb84ff70013897614af11944b6f39213",
            ),
        ],
    },
    Corpus {
        label: "T2",
        fixture: "dcs-form-list-settings-server-state",
        configuration_cf_sha256: "968594192a6610a02a199710547f169c96a8ec821f597b1cf92bd801a0d013ba",
        native_tree: &[
            (
                "Catalogs/CorpusList.xml",
                "f27dbf9a75305d0ba5486fd52093ab641b8e26fd13651d08687b11443777822f",
            ),
            (
                "Catalogs/CorpusList/Ext/ObjectModule.bsl",
                "f1945cd6c19e56b3c1c78943ef5ec18116907a4ca1efc40a57d48ab1db7adfc5",
            ),
            (
                "Catalogs/CorpusList/Forms/ListForm.xml",
                "198673b15ea0adbf1e1543ab08c7c2f978f06103c6e733b8edaf3e77360b9d48",
            ),
            (
                "Catalogs/CorpusList/Forms/ListForm/Ext/Form.xml",
                "b00707828886c454e17736f79573333fd89b4fec9f04618bdd740ecb9a4293ae",
            ),
            (
                "Catalogs/CorpusList/Forms/ListForm/Ext/Form/Module.bsl",
                "a484a6eb7807068684a82463043cad6a07666e4903c44c509f7bfd16cbee9805",
            ),
            (
                "Catalogs/FilterProbe.xml",
                "b1863a5d4f6379912b1784733e09553bf4095fac7794ea65426d9de670d094bf",
            ),
            (
                "Catalogs/FilterProbe/Ext/ObjectModule.bsl",
                "f1945cd6c19e56b3c1c78943ef5ec18116907a4ca1efc40a57d48ab1db7adfc5",
            ),
            (
                "Catalogs/FilterProbe/Forms/ListForm.xml",
                "f8e11a1cf75666c4b989ca9222cc7a816bffd2659f86d7f833518d236cde0cf8",
            ),
            (
                "Catalogs/FilterProbe/Forms/ListForm/Ext/Form.xml",
                "4260e513e185a8cc09854327958a6ec39309d1a607ca2166dbc0be3e672ef23a",
            ),
            (
                "Catalogs/FilterProbe/Forms/ListForm/Ext/Form/Module.bsl",
                "a484a6eb7807068684a82463043cad6a07666e4903c44c509f7bfd16cbee9805",
            ),
            (
                "ConfigDumpInfo.xml",
                "eff594acd7ea41c6ce98edfa4f09abb1e3091726a571b3b9921a6a1b326fdb2d",
            ),
            (
                "Configuration.xml",
                "51aec816e906e9f4d107918dace63cfa8dbcaf658566935faf1c72261878182c",
            ),
            (
                "Ext/ClientApplicationInterface.xml",
                "0a84b26395fff630f14b8f18797442a56711213444fcd8ba69d28f5e42c100e8",
            ),
            (
                "Languages/Русский.xml",
                "ddfd511fb5397985bb76298eea53c970dd647e10c40c24e2a0e7a4ca66aae250",
            ),
            (
                "Reports/FilterProbeReport.xml",
                "d75ebaf21c9ee4e9482552e8b92b7a4225c453451f312b32d372b1fa0ac75e1b",
            ),
            (
                "Reports/FilterProbeReport/Ext/ManagerModule.bsl",
                "f1945cd6c19e56b3c1c78943ef5ec18116907a4ca1efc40a57d48ab1db7adfc5",
            ),
            (
                "Reports/FilterProbeReport/Ext/ObjectModule.bsl",
                "f1945cd6c19e56b3c1c78943ef5ec18116907a4ca1efc40a57d48ab1db7adfc5",
            ),
            (
                "Reports/FilterProbeReport/Templates/MainSchema.xml",
                "a18fda8c76a551fd89d7ddbf8fcd0b947606a380459b8e8e22eaeed946c3614c",
            ),
            (
                "Reports/FilterProbeReport/Templates/MainSchema/Ext/Template.xml",
                "bfc3b612dd6140d3cb0cba5f8a0cf11e4867986a1518b13db46ada8a31901487",
            ),
        ],
    },
    Corpus {
        label: "T3",
        fixture: "configuration-properties-enum-group",
        configuration_cf_sha256: "32e9cf8be130ce284d3ca2473bbc957b256e82d26b9eea37a7501c289b7b26ed",
        native_tree: &[
            (
                "ConfigDumpInfo.xml",
                "2e386d8672c0fbd470c64571c5e474b2ef7cb73bb03afb685e73e12a2e3bbcce",
            ),
            (
                "Configuration.xml",
                "97a1c9a5b314587da1f1bf0babac6995af889c54067de704d4856d665a3de3e7",
            ),
            (
                "Ext/ClientApplicationInterface.xml",
                "0a84b26395fff630f14b8f18797442a56711213444fcd8ba69d28f5e42c100e8",
            ),
            (
                "Languages/Русский.xml",
                "ddfd511fb5397985bb76298eea53c970dd647e10c40c24e2a0e7a4ca66aae250",
            ),
            (
                "Reports/DcsCorpus.xml",
                "d3aa54cbac8d42f6e3a835aae76606a301a3a3cb175f958985ba661a58de9650",
            ),
            (
                "Reports/DcsCorpus/Ext/ManagerModule.bsl",
                "f1945cd6c19e56b3c1c78943ef5ec18116907a4ca1efc40a57d48ab1db7adfc5",
            ),
            (
                "Reports/DcsCorpus/Ext/ObjectModule.bsl",
                "f1945cd6c19e56b3c1c78943ef5ec18116907a4ca1efc40a57d48ab1db7adfc5",
            ),
            (
                "Reports/DcsCorpus/Templates/MainSchema.xml",
                "92d72ca2e6fcedf3b0b4e6099b09a4fd31e365766b5d5df9806bca250faab62f",
            ),
            (
                "Reports/DcsCorpus/Templates/MainSchema/Ext/Template.xml",
                "98f1857d3424198275cc35834a6635c28623568aae8d01a95cb5e220f91b818f",
            ),
            (
                "StyleItems/CorpusAccent.xml",
                "a62b959471790db01992201232f08238fb84ff70013897614af11944b6f39213",
            ),
        ],
    },
];

const SOURCE_VERSION: &str = "2.20";
const TARGET_PROFILE: &str = "platform-8.3.27.1989";

// -------------------------------------------------------------------------
// Path normalization
// -------------------------------------------------------------------------

/// Canonical compositions for every precomposed character in the Cyrillic
/// blocks, i.e. exactly the `(base, combining mark) -> composed` pairs macOS
/// can hand back when a directory entry is stored decomposed (HFS+ always
/// decomposes; APFS preserves what was written).
///
/// `Languages/Русский.xml` is the concrete case this exists for: the exporter
/// writes `й` as U+0439, and a decomposing volume returns U+0438 U+0306, which
/// compares unequal byte-for-byte against the pinned inventory above.
const CYRILLIC_COMPOSITIONS: &[(char, char, char)] = &[
    ('\u{0415}', '\u{0300}', '\u{0400}'),
    ('\u{0415}', '\u{0308}', '\u{0401}'),
    ('\u{0413}', '\u{0301}', '\u{0403}'),
    ('\u{0406}', '\u{0308}', '\u{0407}'),
    ('\u{041A}', '\u{0301}', '\u{040C}'),
    ('\u{0418}', '\u{0300}', '\u{040D}'),
    ('\u{0423}', '\u{0306}', '\u{040E}'),
    ('\u{0418}', '\u{0306}', '\u{0419}'),
    ('\u{0438}', '\u{0306}', '\u{0439}'),
    ('\u{0435}', '\u{0300}', '\u{0450}'),
    ('\u{0435}', '\u{0308}', '\u{0451}'),
    ('\u{0433}', '\u{0301}', '\u{0453}'),
    ('\u{0456}', '\u{0308}', '\u{0457}'),
    ('\u{043A}', '\u{0301}', '\u{045C}'),
    ('\u{0438}', '\u{0300}', '\u{045D}'),
    ('\u{0443}', '\u{0306}', '\u{045E}'),
    ('\u{0474}', '\u{030F}', '\u{0476}'),
    ('\u{0475}', '\u{030F}', '\u{0477}'),
    ('\u{0416}', '\u{0306}', '\u{04C1}'),
    ('\u{0436}', '\u{0306}', '\u{04C2}'),
    ('\u{0410}', '\u{0306}', '\u{04D0}'),
    ('\u{0430}', '\u{0306}', '\u{04D1}'),
    ('\u{0410}', '\u{0308}', '\u{04D2}'),
    ('\u{0430}', '\u{0308}', '\u{04D3}'),
    ('\u{0415}', '\u{0306}', '\u{04D6}'),
    ('\u{0435}', '\u{0306}', '\u{04D7}'),
    ('\u{04D8}', '\u{0308}', '\u{04DA}'),
    ('\u{04D9}', '\u{0308}', '\u{04DB}'),
    ('\u{0416}', '\u{0308}', '\u{04DC}'),
    ('\u{0436}', '\u{0308}', '\u{04DD}'),
    ('\u{0417}', '\u{0308}', '\u{04DE}'),
    ('\u{0437}', '\u{0308}', '\u{04DF}'),
    ('\u{0418}', '\u{0304}', '\u{04E2}'),
    ('\u{0438}', '\u{0304}', '\u{04E3}'),
    ('\u{0418}', '\u{0308}', '\u{04E4}'),
    ('\u{0438}', '\u{0308}', '\u{04E5}'),
    ('\u{041E}', '\u{0308}', '\u{04E6}'),
    ('\u{043E}', '\u{0308}', '\u{04E7}'),
    ('\u{04E8}', '\u{0308}', '\u{04EA}'),
    ('\u{04E9}', '\u{0308}', '\u{04EB}'),
    ('\u{042D}', '\u{0308}', '\u{04EC}'),
    ('\u{044D}', '\u{0308}', '\u{04ED}'),
    ('\u{0423}', '\u{0304}', '\u{04EE}'),
    ('\u{0443}', '\u{0304}', '\u{04EF}'),
    ('\u{0423}', '\u{0308}', '\u{04F0}'),
    ('\u{0443}', '\u{0308}', '\u{04F1}'),
    ('\u{0423}', '\u{030B}', '\u{04F2}'),
    ('\u{0443}', '\u{030B}', '\u{04F3}'),
    ('\u{0427}', '\u{0308}', '\u{04F4}'),
    ('\u{0447}', '\u{0308}', '\u{04F5}'),
    ('\u{042B}', '\u{0308}', '\u{04F8}'),
    ('\u{044B}', '\u{0308}', '\u{04F9}'),
];

const fn is_combining_mark(value: char) -> bool {
    matches!(value, '\u{0300}'..='\u{036F}')
}

/// Recomposes a filesystem-provided path so it can be compared against the NFC
/// literals the exporter writes and this file pins.
///
/// Deliberately fail-closed rather than best-effort: a combining mark this
/// table does not cover is returned as an `Err` naming the exact code points,
/// so an unsupported script surfaces as a stated limitation of the gate instead
/// of a phantom "file missing / file extra" pair.  Extending it means adding
/// the missing canonical compositions here.
fn normalize_nfc(input: &str) -> Result<String, String> {
    let mut output = String::with_capacity(input.len());
    for value in input.chars() {
        if !is_combining_mark(value) {
            output.push(value);
            continue;
        }
        let base = output.pop().ok_or_else(|| {
            format!(
                "combining mark U+{:04X} has no base character in `{input}`",
                value as u32
            )
        })?;
        let composed = CYRILLIC_COMPOSITIONS
            .iter()
            .find(|(candidate_base, mark, _)| *candidate_base == base && *mark == value)
            .map(|(_, _, composed)| *composed)
            .ok_or_else(|| {
                format!(
                    "no canonical composition known for U+{:04X} + U+{:04X} in `{input}`; \
                     extend CYRILLIC_COMPOSITIONS",
                    base as u32, value as u32
                )
            })?;
        output.push(composed);
    }
    Ok(output)
}

// -------------------------------------------------------------------------
// Fixtures and process helpers
// -------------------------------------------------------------------------

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let nonce = format!(
            "{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(format!(
            "ibcmd-rs-native-roundtrip-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture_dir(corpus: &Corpus) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/native-evidence/8.3.27.2214")
        .join(corpus.fixture)
}

fn decode_base64(source: &str) -> Vec<u8> {
    let mut output = Vec::new();
    let mut buffer = 0_u32;
    let mut bits = 0_u8;
    let mut saw_padding = false;
    for byte in source.bytes() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if byte == b'=' {
            saw_padding = true;
            continue;
        }
        assert!(!saw_padding, "non-padding data after Base64 padding");
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("invalid Base64 byte 0x{byte:02x}"),
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
            buffer &= if bits == 0 { 0 } else { (1_u32 << bits) - 1 };
        }
    }
    assert!(bits == 0 || buffer == 0, "non-zero trailing Base64 bits");
    output
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Result of one CLI invocation, with the JSON report already parsed when the
/// command produced one on either stream.
struct CliRun {
    success: bool,
    report: Option<Value>,
    stderr: String,
}

impl CliRun {
    /// Renders the command's own typed diagnostics, which is what makes a
    /// failure of this gate actionable instead of a bare non-zero exit.
    fn blockers(&self) -> String {
        let Some(errors) = self
            .report
            .as_ref()
            .and_then(|report| report.get("errors"))
            .and_then(Value::as_array)
        else {
            return format!("      (no JSON report; stderr: {})", self.stderr.trim());
        };
        if errors.is_empty() {
            return "      (command failed without recording any diagnostic)".to_owned();
        }
        errors
            .iter()
            .map(|error| {
                let code = error["code"].as_str().unwrap_or("<no code>");
                let message = error["message"].as_str().unwrap_or("<no message>");
                let element = error["element"]
                    .as_str()
                    .map(|path| format!(" @ {path}"))
                    .unwrap_or_default();
                let expected = error["expected"]
                    .as_str()
                    .map(|value| format!("\n        expected: {value}"))
                    .unwrap_or_default();
                let actual = error["actual"]
                    .as_str()
                    .map(|value| format!("\n        actual:   {value}"))
                    .unwrap_or_default();
                format!("      {code}{element}: {message}{expected}{actual}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn run_cli(args: &[&Path], flags: &[&str], subcommand: &[&str]) -> CliRun {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
    command.args(subcommand);
    for path in args {
        command.arg(path);
    }
    let output = command.args(flags).env("PATH", "").output().unwrap();
    let report = serde_json::from_slice::<Value>(&output.stdout)
        .ok()
        .or_else(|| serde_json::from_slice::<Value>(&output.stderr).ok());
    CliRun {
        success: output.status.success(),
        report,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Reads a whole exported tree as `normalized relative path -> bytes`.
fn read_tree(root: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    fn visit(
        root: &Path,
        current: &Path,
        files: &mut BTreeMap<String, Vec<u8>>,
    ) -> Result<(), String> {
        for entry in fs::read_dir(current).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files)?;
            } else {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                let relative = normalize_nfc(&relative)?;
                let bytes = fs::read(&path).map_err(|error| error.to_string())?;
                files.insert(relative, bytes);
            }
        }
        Ok(())
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files)?;
    Ok(files)
}

// -------------------------------------------------------------------------
// The gate
// -------------------------------------------------------------------------

/// Measures one corpus and appends a human-readable section to `report`.
/// Returns `true` when the reverse direction reproduced the tree exactly.
fn measure(corpus: &Corpus, report: &mut String) -> bool {
    let temp = TempDirectory::new();
    let base_cf = temp.path().join("base.cf");
    let native_dir = temp.path().join("native");
    let rebuilt_cf = temp.path().join("rebuilt.cf");
    let roundtrip_dir = temp.path().join("roundtrip");

    let expected: BTreeMap<&str, &str> = corpus.native_tree.iter().copied().collect();
    writeln!(
        report,
        "\n{} ({}): {} native files",
        corpus.label,
        corpus.fixture,
        expected.len()
    )
    .unwrap();

    // 0. The bundled retained CF must be the exact artifact the pinned native
    //    inventory was captured from.  The per-corpus `manifest.json` files
    //    record this hash under several different keys, so the uniform pin
    //    lives in [`CORPORA`] instead of being read back out of them.
    let cf_path = fixture_dir(corpus).join("configuration.cf.b64");
    let encoded = match fs::read_to_string(&cf_path) {
        Ok(encoded) => encoded,
        Err(error) => {
            writeln!(
                report,
                "  bundled CF unreadable ({}): {error}",
                cf_path.display()
            )
            .unwrap();
            return false;
        }
    };
    let cf_bytes = decode_base64(&encoded);
    let cf_sha256 = sha256_hex(&cf_bytes);
    if cf_sha256 != corpus.configuration_cf_sha256 {
        writeln!(
            report,
            "  fixture mismatch: configuration.cf.b64 decodes to {cf_sha256}, pinned {}",
            corpus.configuration_cf_sha256
        )
        .unwrap();
        return false;
    }
    if let Err(error) = fs::write(&base_cf, &cf_bytes) {
        writeln!(report, "  cannot stage the decoded CF: {error}").unwrap();
        return false;
    }

    // 1. Forward direction: rebuild the reference tree and prove it is the
    //    platform's bytes, not merely our own.
    let export = run_cli(
        &[&base_cf, &native_dir],
        &["--source-version", SOURCE_VERSION],
        &["cf", "export"],
    );
    if !export.success {
        writeln!(
            report,
            "  reference export failed; the forward direction regressed:\n{}",
            export.blockers()
        )
        .unwrap();
        return false;
    }
    let native = match read_tree(&native_dir) {
        Ok(native) => native,
        Err(error) => {
            writeln!(report, "  reference tree unreadable: {error}").unwrap();
            return false;
        }
    };
    let mut reference_drift = Vec::new();
    for (path, pinned) in &expected {
        match native.get(*path) {
            None => reference_drift.push(format!("      missing: {path}")),
            Some(bytes) => {
                let actual = sha256_hex(bytes);
                if actual != *pinned {
                    reference_drift.push(format!(
                        "      differs: {path} (pinned {pinned}, exported {actual})"
                    ));
                }
            }
        }
    }
    for path in native.keys() {
        if !expected.contains_key(path.as_str()) {
            reference_drift.push(format!("      extra:   {path}"));
        }
    }
    if !reference_drift.is_empty() {
        writeln!(
            report,
            "  reference export no longer matches the pinned platform capture:\n{}",
            reference_drift.join("\n")
        )
        .unwrap();
        return false;
    }
    writeln!(
        report,
        "  reference export: {}/{} files match the pinned platform capture",
        expected.len(),
        expected.len()
    )
    .unwrap();

    // 2. Reverse direction: native tree -> CF.
    let bootstrap = run_cli(
        &[&native_dir, &rebuilt_cf],
        &[
            "--source-version",
            SOURCE_VERSION,
            "--target-profile",
            TARGET_PROFILE,
        ],
        &["cf", "bootstrap"],
    );
    if !bootstrap.success {
        writeln!(
            report,
            "  reverse direction did not start: `cf bootstrap` refused the tree\n\
             \x20   matched 0/{}, differing 0, missing {}, extra 0\n\
             \x20   blockers:\n{}",
            expected.len(),
            expected.len(),
            bootstrap.blockers()
        )
        .unwrap();
        return false;
    }

    // 3. Reverse direction: CF -> tree.
    let reexport = run_cli(
        &[&rebuilt_cf, &roundtrip_dir],
        &["--source-version", SOURCE_VERSION],
        &["cf", "export"],
    );
    if !reexport.success {
        writeln!(
            report,
            "  rebuilt CF could not be exported back\n\
             \x20   matched 0/{}, differing 0, missing {}, extra 0\n\
             \x20   blockers:\n{}",
            expected.len(),
            expected.len(),
            reexport.blockers()
        )
        .unwrap();
        return false;
    }
    let rebuilt = match read_tree(&roundtrip_dir) {
        Ok(rebuilt) => rebuilt,
        Err(error) => {
            writeln!(report, "  round-trip tree unreadable: {error}").unwrap();
            return false;
        }
    };

    // 4. Compare, file by file, against the reference tree.
    let mut matched = 0_usize;
    let mut differing = Vec::new();
    let mut missing = Vec::new();
    for (path, expected_bytes) in &native {
        match rebuilt.get(path) {
            None => missing.push(path.clone()),
            Some(actual_bytes) if actual_bytes == expected_bytes => matched += 1,
            Some(actual_bytes) => differing.push(format!(
                "{path} (expected {} bytes / {}, got {} bytes / {})",
                expected_bytes.len(),
                sha256_hex(expected_bytes),
                actual_bytes.len(),
                sha256_hex(actual_bytes)
            )),
        }
    }
    let extra = rebuilt
        .keys()
        .filter(|path| !native.contains_key(*path))
        .cloned()
        .collect::<Vec<_>>();

    writeln!(
        report,
        "  round trip: matched {matched}/{}, differing {}, missing {}, extra {}",
        native.len(),
        differing.len(),
        missing.len(),
        extra.len()
    )
    .unwrap();
    for entry in &differing {
        writeln!(report, "      differs: {entry}").unwrap();
    }
    for entry in &missing {
        writeln!(report, "      missing: {entry}").unwrap();
    }
    for entry in &extra {
        writeln!(report, "      extra:   {entry}").unwrap();
    }
    differing.is_empty() && missing.is_empty() && extra.is_empty()
}

/// Reverse-direction gate over all three retained corpora.
///
/// `#[ignore]`d because the reverse direction is known-incomplete on today's
/// `master`, and the two remaining causes are owned by other work in flight.
/// Measured on this commit, with the export-manifest classification in
/// `src/compiler/bootstrap.rs` in place:
///
/// * T1 and T3 stop in `compile_bootstrap_source_tree`'s Configuration
///   projection — `invalid_configuration`: "Configuration property
///   `UsePurposes` has no base-free projection".  That projection covers 16 of
///   the 55 properties a native `Configuration.xml` carries
///   (`src/compiler/bootstrap.rs`, `project_configuration`).
/// * T2 stops earlier, in the metadata decoder — `invalid_metadata_envelope` on
///   `Catalogs/CorpusList.xml`: "business object unevidenced complex property is
///   not empty", raised by `crates/ibcmd-xml/src/metadata/business_objects.rs`
///   for a non-empty `<StandardAttributes>`.
///
/// Remove `#[ignore]` once both are closed; the assertion below is written
/// against the platform's own bytes and was never relaxed to fit the current
/// behavior, so it will report the real remaining delta the moment it runs.
#[test]
#[ignore = "reverse direction blocked: Configuration base-free property projection (T1/T3) and non-empty StandardAttributes decoding (T2)"]
fn native_tree_rebuilds_into_an_identical_tree() {
    let mut report = String::from("native XML tree -> CF -> native XML tree");
    let mut clean = true;
    for corpus in CORPORA {
        clean &= measure(corpus, &mut report);
    }
    assert!(clean, "{report}");
}

#[test]
fn decomposed_cyrillic_paths_normalize_to_the_exporter_form() {
    // Exactly the case `Languages/Русский.xml` hits on a decomposing volume.
    assert_eq!(
        normalize_nfc("Languages/Ru\u{0438}\u{0306}.xml").unwrap(),
        "Languages/Ruй.xml"
    );
    assert_eq!(
        normalize_nfc("Languages/Русский.xml").unwrap(),
        "Languages/Русский.xml"
    );
    // An uncovered composition is a stated limitation, never a silent mismatch.
    let error = normalize_nfc("Cafe\u{0301}.xml").unwrap_err();
    assert!(error.contains("no canonical composition known"), "{error}");
}

#[test]
fn pinned_native_inventories_are_well_formed() {
    for corpus in CORPORA {
        let mut paths = corpus
            .native_tree
            .iter()
            .map(|(path, _)| *path)
            .collect::<Vec<_>>();
        let count = paths.len();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(
            paths.len(),
            count,
            "{}: duplicate pinned path",
            corpus.label
        );
        for (path, sha256) in corpus.native_tree {
            assert_eq!(sha256.len(), 64, "{}: bad sha256 for {path}", corpus.label);
            assert!(
                sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "{}: non-hex sha256 for {path}",
                corpus.label
            );
        }
        assert!(
            fixture_dir(corpus).join("configuration.cf.b64").is_file(),
            "{}: bundled retained CF is missing",
            corpus.label
        );
    }
}
