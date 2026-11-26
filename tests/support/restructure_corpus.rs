//! Conformance of the cache decoders against the lab corpora of the `ddl` track (snapshots of
//! `scripts/restructure-lab/snapshot.py`). The samples are big and live in the lab, so every test
//! skips itself when the lab folder is absent: set `IBCMD_RS_DDL_LAB` to another location, default
//! `F:\ibcmd\lab\04\restructure`.
//!
//! The snapshots used, by the names of this file:
//!
//! | name | database / label | what it is |
//! |---|---|---|
//! | `pristine` | `bsp8327_a` / `a2_staged` | the caches before case c and case h |
//! | `c2` | `bsp8327_c2` / `c2_now` | native after case c (a new catalog) |
//! | `m` | `bsp8327_m` / `m_now` | native after case h (a new tabular section, among other changes) |
//! | `t1_before`, `t1_nat` | `s1_base` / `t1_staged`, `s1_t1_nat` / `nat_after` | the types case |
//! | `e1_*`, `e3_*`, `e4_*` | `s2_e1_base` / `e1_staged`, `s2_e1_nat` / `nat_after`, ... | S1-E: five new tabular sections; new attributes of old sections; own attributes, an old section's attribute and new sections of a catalog and a document at once |

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;

use crate::restructure::names::inflate;
use ibcmd_restructure::brace::{Brace, parse_row};

pub(crate) fn lab() -> Option<PathBuf> {
    let root = std::env::var_os("IBCMD_RS_DDL_LAB")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"F:\ibcmd\lab\04\restructure"));
    root.join("snap").is_dir().then_some(root)
}

type Parts = BTreeMap<String, BTreeMap<i64, String>>;

pub(crate) struct Snap {
    root: PathBuf,
    params: Parts,
    config: Parts,
    save: Parts,
    /// The blob of `SchemaStorage.CurrentSchema` of `SchemaID 0`.
    schema: Option<String>,
}

fn parts_of(svc: &Value, table: &str) -> Parts {
    let mut out: Parts = BTreeMap::new();
    for row in svc[table].as_array().unwrap() {
        out.entry(row["name"].as_str().unwrap().to_owned())
            .or_default()
            .insert(
                row["part"].as_i64().unwrap(),
                row["sha"].as_str().unwrap().to_owned(),
            );
    }
    out
}

/// The snapshots of the `trace` track (`IBCMD_RS_TRACE_LAB`, default `F:\ibcmd\lab\05\s1g\store`):
/// case d, made by the kit `scripts/apply-trace/lab/s1g-caches/`.
fn trace_store() -> Option<PathBuf> {
    let root = std::env::var_os("IBCMD_RS_TRACE_LAB")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"F:\ibcmd\lab\05\s1g\store"));
    root.join("snap").is_dir().then_some(root)
}

impl Snap {
    /// One of `pristine`, `c2`, `m`, `t1_before`, `t1_nat` (the `ddl` lab) or `d_staged`, `d_after`
    /// (the `trace` store).
    pub(crate) fn open(name: &str) -> Option<Self> {
        let (db, label) = match name {
            "pristine" => (
                "ibcmd_rs_04_ddl_bsp8327_a".to_owned(),
                "a2_staged".to_owned(),
            ),
            "c2" => ("ibcmd_rs_04_ddl_bsp8327_c2".to_owned(), "c2_now".to_owned()),
            "m" => ("ibcmd_rs_04_ddl_bsp8327_m".to_owned(), "m_now".to_owned()),
            "t1_before" => ("ibcmd_rs_04_ddl_s1_base".to_owned(), "t1_staged".to_owned()),
            "t1_nat" => (
                "ibcmd_rs_04_ddl_s1_t1_nat".to_owned(),
                "nat_after".to_owned(),
            ),
            "d_staged" | "d_after" => ("ibcmd_rs_05_trace_d_base".to_owned(), name.to_owned()),
            "e1_before" => (
                "ibcmd_rs_04_ddl_s2_e1_base".to_owned(),
                "e1_staged".to_owned(),
            ),
            "e1_nat" => (
                "ibcmd_rs_04_ddl_s2_e1_nat".to_owned(),
                "nat_after".to_owned(),
            ),
            "e3_before" => (
                "ibcmd_rs_04_ddl_s2_e3_base".to_owned(),
                "e3_staged".to_owned(),
            ),
            "e3_nat" => (
                "ibcmd_rs_04_ddl_s2_e3_nat".to_owned(),
                "nat_after".to_owned(),
            ),
            "e4_before" => (
                "ibcmd_rs_04_ddl_s2_e4_base".to_owned(),
                "e4_staged".to_owned(),
            ),
            "e4_nat" => (
                "ibcmd_rs_04_ddl_s2_e4_nat".to_owned(),
                "nat_after".to_owned(),
            ),
            "e5_before" => (
                "ibcmd_rs_04_ddl_s2_e5_base".to_owned(),
                "e5_staged".to_owned(),
            ),
            "e5_nat" => (
                "ibcmd_rs_04_ddl_s2_e5_nat".to_owned(),
                "nat_after".to_owned(),
            ),
            "e6_before" => (
                "ibcmd_rs_04_ddl_s2_e6_base".to_owned(),
                "e6_staged".to_owned(),
            ),
            "e6_nat" => (
                "ibcmd_rs_04_ddl_s2_e6_nat".to_owned(),
                "nat_after".to_owned(),
            ),
            // the native cases of S1-F: the staged state `n1_staged` of the database `..._tw1_st` and the platform's
            // result `n1_after` (the snapshot `nat_after` of `..._tw1_nat`), from the twin runs of `twin_stage.ps1`
            other
                if other.len() > 3
                    && other.starts_with('n')
                    && other[1..2].chars().all(|c| c.is_ascii_digit()) =>
            {
                let tag = match &other[..2] {
                    "n1" => "tw1",
                    "n2" => "tw2",
                    "n3" => "tw3c",
                    "n4" => "tw4",
                    "n5" => "tw5",
                    "n6" => "tw6",
                    "n7" => "tw7",
                    _ => panic!("unknown snapshot {other}"),
                };
                if other.ends_with("_staged") {
                    (format!("ibcmd_rs_05_trace_{tag}_st"), other.to_owned())
                } else {
                    (
                        format!("ibcmd_rs_05_trace_{tag}_nat"),
                        "nat_after".to_owned(),
                    )
                }
            }
            other => panic!("unknown snapshot {other}"),
        };
        let root = if name.starts_with("d_") || (name.starts_with('n') && name.len() > 3) {
            trace_store()?
        } else {
            lab()?
        };
        let text = std::fs::read_to_string(root.join("snap").join(db).join(label).join("svc.json"))
            .ok()?;
        let svc: Value = serde_json::from_str(&text).ok()?;
        let schema = svc["SchemaStorage"]
            .as_array()
            .and_then(|rows| rows.iter().find(|row| row["id"].as_i64() == Some(0)))
            .and_then(|row| row["cur"].as_str())
            .map(str::to_owned);
        Some(Self {
            root,
            params: parts_of(&svc, "Params"),
            config: parts_of(&svc, "Config"),
            save: parts_of(&svc, "ConfigSave"),
            schema,
        })
    }

    fn stored(&self, parts: &Parts, name: &str) -> Option<Vec<u8>> {
        let parts = parts.get(name)?;
        let mut bytes = Vec::new();
        for sha in parts.values() {
            bytes.extend(std::fs::read(self.root.join("blobs").join(sha)).unwrap());
        }
        Some(bytes)
    }

    /// The names of the rows of `Params`, `Config` or `ConfigSave`.
    pub(crate) fn names(&self, table: &str) -> Vec<String> {
        let parts = match table {
            "Params" => &self.params,
            "Config" => &self.config,
            "ConfigSave" => &self.save,
            other => panic!("no table {other}"),
        };
        parts.keys().cloned().collect()
    }

    /// The row as stored (the parts joined, not inflated).
    pub(crate) fn stored_row(&self, table: &str, name: &str) -> Option<Vec<u8>> {
        let parts = match table {
            "Params" => &self.params,
            "Config" => &self.config,
            "ConfigSave" => &self.save,
            other => panic!("no table {other}"),
        };
        self.stored(parts, name)
    }

    /// The inflated text of a `Params` row (raw when it is not deflated).
    pub(crate) fn params(&self, name: &str) -> Option<Vec<u8>> {
        let stored = self.stored(&self.params, name)?;
        Some(inflate(&stored).unwrap_or(stored))
    }

    /// The inflated text of a `Config` row.
    pub(crate) fn config(&self, name: &str) -> Option<Vec<u8>> {
        let stored = self.stored(&self.config, name)?;
        Some(inflate(&stored).unwrap_or(stored))
    }

    /// The parsed descriptor row of an object.
    pub(crate) fn descriptor(&self, uuid: &str) -> Option<Brace> {
        parse_row(&self.config(uuid)?).ok()
    }

    /// The text of the stored `DBSchema` (`SchemaStorage.CurrentSchema`, inflated).
    pub(crate) fn schema(&self) -> Option<Vec<u8>> {
        let sha = self.schema.as_ref()?;
        let bytes = std::fs::read(self.root.join("blobs").join(sha)).ok()?;
        Some(inflate(&bytes).unwrap_or(bytes))
    }

    /// Whether a `Config` row exists.
    pub(crate) fn has_config(&self, name: &str) -> bool {
        self.config.contains_key(name)
    }
}

/// The 16 cache rows by the short id of their uuid.
pub(crate) const CACHE_ROWS: &[(&str, &str)] = &[
    ("0b698dcd", "0b698dcd-501d-42d9-892d-5a9157bc996a.si"),
    ("1a621f0f", "1a621f0f-5568-4183-bd9f-f6ef670e7090.si"),
    ("215d232c", "215d232c-9c9e-4f7c-8a87-142cd3797264.si"),
    ("2203278d", "2203278d-ef4f-4f68-98f1-feb257d53ecc.si"),
    ("42ed49cc", "42ed49cc-765d-4314-bc2d-af425af7bf13.si"),
    ("59274b8d", "59274b8d-4447-4bf4-9d29-bfa099a1de37.si"),
    ("a07b62f0", "a07b62f0-1f01-484a-93d9-d42764cedac0.si"),
    ("c40aafd6", "c40aafd6-c889-4229-807a-851d0bc5bc97.si"),
    ("c4629235", "c4629235-4823-4320-b8b5-1d08f4c6d612.si"),
    ("c77bc206", "c77bc206-5935-48ea-b32e-508a572d94f4.si"),
    ("cf8b5e0f", "cf8b5e0f-5e46-4cf4-bc6f-204eae2c4e8a.si"),
    ("e05c0074", "e05c0074-0404-4b7a-835e-9cacd405960e.si"),
    ("ea13a2c9", "ea13a2c9-0c2f-40fa-b855-710387e3271d.si"),
    ("facbfffe", "facbfffe-feb2-4d30-8930-a557b185e5c4.si"),
    ("fd1b2a86", "fd1b2a86-b7df-4f32-84e2-befd4f3a2331.si"),
    ("fe8acd6a", "fe8acd6a-22c9-4b5a-aeae-232a1c8324cb.si"),
];

pub(crate) fn row_name(short: &str) -> &'static str {
    CACHE_ROWS.iter().find(|(id, _)| *id == short).unwrap().1
}

/// The root row of the configuration.
pub(crate) const ROOT_ROW: &str = "66193438-abc5-410b-a1f1-a204102d1a62";

macro_rules! lab_snap {
    ($name:expr) => {
        match $crate::restructure::caches::tests_corpus::Snap::open($name) {
            Some(snap) => snap,
            None => {
                eprintln!("skipped: no lab snapshot {}", $name);
                return;
            }
        }
    };
}

pub(crate) use lab_snap;
