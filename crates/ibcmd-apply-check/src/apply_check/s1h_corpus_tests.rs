//! The whole native image of the ddl track's case a2 (a native `config
//! import` of a tree with one attribute added, 9 842 ConfigSave rows) against
//! the corpus it was made from. Ignored by default: the rows live in the lab
//! folder (`fixtures/nat_a2`, `fixtures/nat_a2_base`). Run with
//! `cargo test -p ibcmd-apply-check apply_check::s1h -- --ignored --nocapture`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::PathBuf;

use anyhow::Result;
use flate2::read::DeflateDecoder;

use super::roles::{RowName, parse_row_name};
use super::s1::{self, ObjectId, RefusalCode, S1Operation};
use super::sql::parse_versions;
use super::{Inputs, RowProvider, RuleId, Verdict, check};

const LAB: &str = r"F:\ibcmd\lab\04\restructure-check\fixtures";

fn inflate(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    match DeflateDecoder::new(bytes).read_to_end(&mut out) {
        Ok(_) => out,
        Err(_) => bytes.to_vec(),
    }
}

/// Part 0 of every row of a fixture folder, inflated, by row name.
fn rows(folder: &str, table: &str) -> Option<BTreeMap<String, Vec<u8>>> {
    let dir = PathBuf::from(LAB).join(folder).join(table);
    if !dir.is_dir() {
        println!("{} is not there, skipped", dir.display());
        return None;
    }
    let mut out = BTreeMap::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if let Some(row) = name.strip_suffix("__part0.bin") {
            out.insert(row.to_string(), inflate(&fs::read(&path).unwrap()));
        }
    }
    Some(out)
}

/// The rows of two fixture folders as the check reads them.
struct Fixture {
    active: BTreeMap<String, Vec<u8>>,
    staged: BTreeMap<String, Vec<u8>>,
}

impl RowProvider for Fixture {
    fn old_rows(&self, names: &[String]) -> Result<BTreeMap<String, Vec<u8>>> {
        Ok(names
            .iter()
            .filter_map(|name| self.active.get(name).map(|row| (name.clone(), row.clone())))
            .collect())
    }

    fn staged_rows(&self, names: &[String]) -> Result<BTreeMap<String, Vec<u8>>> {
        Ok(names
            .iter()
            .filter_map(|name| self.staged.get(name).map(|row| (name.clone(), row.clone())))
            .collect())
    }
}

fn descriptors_of(rows: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Vec<u8>> {
    rows.iter()
        .filter(|(name, _)| matches!(parse_row_name(name), RowName::Descriptor(_)))
        .map(|(name, row)| (name.clone(), row.clone()))
        .collect()
}

/// The check of the a2 image, the way `check_staged` assembles it from the
/// database, from the fixture folders.
fn check_a2_image(alter: impl FnOnce(&mut BTreeMap<String, Vec<u8>>)) -> Option<(Verdict, ())> {
    let active = rows("nat_a2_base", "Config")?;
    let mut staged = rows("nat_a2", "ConfigSave")?;
    alter(&mut staged);
    let mut inputs = Inputs {
        xml_version: None,
        ..Inputs::default()
    };
    inputs.old_root = active.get("root").cloned();
    inputs.old_version = active.get("version").cloned();
    inputs.old_versions = parse_versions(&active["versions"]).unwrap();
    inputs.old_inventory = inputs.old_versions.keys().cloned().collect();
    inputs.old_descriptors = descriptors_of(&active)
        .into_iter()
        .filter(|(name, _)| inputs.old_inventory.contains(name))
        .collect();
    inputs.staged_names = staged.keys().cloned().collect::<BTreeSet<_>>();
    inputs.staged_descriptors = descriptors_of(&staged);
    inputs.staged_root = staged.get("root").cloned();
    inputs.staged_version = staged.get("version").cloned();
    inputs.staged_deleted = staged.get("deleted").cloned();
    if let Some(versions) = staged.get("versions") {
        inputs.staged_has_versions = true;
        inputs.new_versions = parse_versions(versions).unwrap();
        inputs.new_inventory = inputs.new_versions.keys().cloned().collect();
    }
    let fixture = Fixture { active, staged };
    Some((check(&inputs, &fixture), ()))
}

#[test]
#[ignore = "reads the a2 fixtures of the lab folder"]
fn the_a2_image_shows_its_one_change_and_proves_the_rest() {
    let Some((verdict, _)) = check_a2_image(|_| {}) else {
        return;
    };
    println!("{}", verdict.render_text());
    // 9 842 rows, 517 descriptors that differ in bytes: the attribute added
    // is the one reason.
    assert_eq!(verdict.reasons.len(), 1, "{:#?}", verdict.reasons);
    let reason = &verdict.reasons[0];
    assert_eq!(reason.rule, RuleId::ColumnAddedOrDropped);
    assert_eq!(reason.kind, "Catalog");
    assert_eq!(reason.object, "Catalog._ДемоПартнеры");
    assert_eq!(
        reason.path_names(),
        ["ChildObjects", "Attribute"],
        "{}",
        reason.property
    );
    // The other 516: 396 that decode to the same XML, 119 that only decode in
    // the newer record format, the Configuration row of the `{68}` shape.
    assert_eq!(verdict.stats.format_upgrades, 516);
    assert_eq!(verdict.stats.descriptors_compared, 517);
    let class = s1::classify(&verdict);
    print!("{}", class.render_text());
    assert!(class.accepted(), "{:#?}", class.refusals);
    assert_eq!(
        class.operations,
        vec![S1Operation::AddAttribute {
            object: ObjectId {
                kind: "Catalog".to_string(),
                name: "_ДемоПартнеры".to_string(),
                row: "5eab8a1b-070f-4dcf-bdcc-a259c62c3693".to_string(),
            },
            attribute: "ДемоНовыйРеквизит".to_string(),
        }]
    );
}

/// The names of the descriptors of the image that differ in bytes but not in
/// meaning: the ones the proof has to carry.
fn noise_rows() -> Vec<String> {
    let (Some(staged), Some(active)) =
        (rows("nat_a2", "ConfigSave"), rows("nat_a2_base", "Config"))
    else {
        return Vec::new();
    };
    staged
        .iter()
        .filter(|(name, new)| {
            super::roles::is_uuid(name)
                && active.get(*name).is_some_and(|old| old != *new)
                && name.as_str() != "5eab8a1b-070f-4dcf-bdcc-a259c62c3693"
        })
        .map(|(name, _)| name.clone())
        .collect()
}

#[test]
#[ignore = "reads the a2 fixtures of the lab folder"]
fn a_noise_row_that_is_not_only_an_upgrade_is_refused_by_name() {
    let noise = noise_rows();
    if noise.is_empty() {
        return;
    }
    assert_eq!(noise.len(), 516);
    // The class id the importer adds to the property lists of an object,
    // changed by one digit: a record nobody has seen the platform write.
    const SEEN: &str = "3b10624f-1e3d-495d-8093-25225efc5313";
    const UNSEEN: &str = "3b10624f-1e3d-495d-8093-25225efc5314";
    let mut victim = String::new();
    let (verdict, _) = check_a2_image(|staged| {
        victim = noise
            .iter()
            .find(|name| {
                String::from_utf8_lossy(&staged[*name]).contains(SEEN)
                    && name.as_str() != "66193438-abc5-410b-a1f1-a204102d1a62"
            })
            .unwrap()
            .clone();
        let row = staged.get_mut(&victim).unwrap();
        let text = String::from_utf8(row.clone()).unwrap();
        *row = text.replacen(SEEN, UNSEEN, 1).into_bytes();
    })
    .unwrap();
    let unproven = verdict
        .reasons
        .iter()
        .filter(|reason| reason.rule == RuleId::RowFormatUpgradeUnproven)
        .collect::<Vec<_>>();
    assert_eq!(unproven.len(), 1, "{:#?}", verdict.reasons);
    assert_eq!(unproven[0].file_name, victim);
    assert!(
        unproven[0].change.contains(UNSEEN),
        "{}",
        unproven[0].change
    );
    assert_eq!(verdict.stats.format_upgrades, 515);
    let class = s1::classify(&verdict);
    assert!(!class.accepted());
    assert!(
        class
            .refusals
            .iter()
            .any(|refusal| refusal.code == RefusalCode::RowFormatUnproven)
    );
}

#[test]
#[ignore = "reads the a2 fixtures of the lab folder"]
fn a_deleted_row_that_lists_files_is_refused() {
    let listed = check_a2_image(|staged| {
        staged.insert(
            "deleted".to_string(),
            "\u{feff}{1,\"some.file\"}".as_bytes().to_vec(),
        );
    });
    let Some((verdict, _)) = listed else { return };
    assert!(
        verdict
            .reasons
            .iter()
            .any(|reason| reason.rule == RuleId::DeletedRowNotEmpty),
        "{:#?}",
        verdict.reasons
    );
    assert!(!s1::classify(&verdict).accepted());
}
