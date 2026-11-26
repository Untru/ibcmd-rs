//! Shared fixture helpers for the planner and its root apply gate tests.

use crate::restructure::caches::tests_corpus::Snap;
use crate::restructure::create::context_uuids;
use crate::restructure::plan::{Inputs, PlanOptions, StagedImage};

pub(super) const VERSION: &str = "00000000-0000-0000-0000-00000000cafe";

pub(super) fn bare(name: &str) -> bool {
    name.len() == 36 && !name.contains('.')
}

/// The plan's input from a snapshot that holds a staged image (`ConfigSave`).
pub(super) fn inputs_of(snap: &Snap) -> Inputs {
    let mut inputs = Inputs {
        schema: snap.schema().unwrap(),
        main_names: snap.stored_row("Params", "DBNames").unwrap(),
        root_row: snap.stored_row("Config", "root").unwrap(),
        ..Inputs::default()
    };
    for name in snap.names("Params") {
        if name.starts_with("DBNames-Ext-") {
            inputs
                .extension_names
                .push((name.clone(), snap.stored_row("Params", &name).unwrap()));
        }
        if name.ends_with(".si") || name == "siVersions" {
            inputs
                .cache_rows
                .push((name.clone(), snap.stored_row("Params", &name).unwrap()));
        }
    }
    let mut image = StagedImage {
        old_files: snap.names("Config").into_iter().collect(),
        new_files: snap.names("ConfigSave").into_iter().collect(),
        deleted: snap.stored_row("ConfigSave", "deleted"),
        ..StagedImage::default()
    };
    for name in snap.names("Config") {
        if bare(&name) {
            image
                .old_descriptors
                .insert(name.clone(), snap.stored_row("Config", &name).unwrap());
        }
    }
    for name in snap.names("ConfigSave") {
        if bare(&name) {
            image
                .new_descriptors
                .insert(name.clone(), snap.stored_row("ConfigSave", &name).unwrap());
        }
    }
    inputs.staged = image;
    inputs.objects = objects_of(snap, &inputs.root_row);
    inputs
}

/// `Inputs::objects`: the stored descriptors the reader would read.
pub(super) fn objects_of(
    snap: &Snap,
    root_row: &[u8],
) -> std::collections::BTreeMap<String, Vec<u8>> {
    let configuration = crate::restructure::plan::configuration_uuid(root_row).unwrap();
    let row = snap.stored_row("Config", &configuration).unwrap();
    context_uuids(&row)
        .unwrap()
        .into_iter()
        .map(|uuid| {
            let stored = snap.stored_row("Config", &uuid).unwrap();
            (uuid, stored)
        })
        .collect()
}

pub(super) fn options() -> PlanOptions {
    PlanOptions {
        names_version: Some(VERSION.to_owned()),
        ..PlanOptions::default()
    }
}
