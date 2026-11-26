//! The versions blob: the raw-deflated map from each stored file name to its
//! generation UUID, patched with fresh UUIDs for the names a load changed (and
//! appended for the names it adds, where the caller allows that).

use anyhow::{Context, Result, anyhow};
use ibcmd_1c_text::is_uuid_like_ascii;
use serde::Serialize;
use uuid::Uuid;

use crate::raw::{deflate_raw, hex_sha256, inflate_raw};

#[derive(Debug, Clone)]
pub struct PatchedVersionsBlob {
    pub blob: Vec<u8>,
    pub plain_bytes: usize,
    pub output_sha256: String,
    pub replacements: Vec<VersionReplacement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionReplacement {
    pub name: String,
    pub old_uuid: String,
    pub new_uuid: String,
}

pub fn patch_versions_blob_bytes(
    input: &[u8],
    changes: &[String],
    include_standard_entries: bool,
) -> Result<PatchedVersionsBlob> {
    patch_versions_blob_bytes_inner(input, changes, include_standard_entries, false)
}

pub fn patch_versions_blob_bytes_allowing_additions(
    input: &[u8],
    changes: &[String],
    include_standard_entries: bool,
) -> Result<PatchedVersionsBlob> {
    patch_versions_blob_bytes_inner(input, changes, include_standard_entries, true)
}

pub fn versions_base_free_blockers(selected_config_rows: usize) -> Vec<String> {
    vec![
        format!(
            "source tree does not contain a ConfigSave versions asset; staging currently fetches the active versions row and patches generation UUIDs for {selected_config_rows} selected Config row(s) plus the standard root/version/versions entries"
        ),
        "the versions blob is a deflated native FileName-to-generation-UUID map; source XML and body assets provide row names and payloads, but not the active generation UUID map, entry order, unchanged row set, or platform-owned standard entries".to_string(),
        "base-free versions row generation is blocked until a validated versions compiler can synthesize the complete native row for the target compatible infobase without dropping stable Config entries or relying on database-specific GUIDs".to_string(),
    ]
}

fn patch_versions_blob_bytes_inner(
    input: &[u8],
    changes: &[String],
    include_standard_entries: bool,
    allow_missing_changes: bool,
) -> Result<PatchedVersionsBlob> {
    let plain = inflate_raw(input).context("failed to inflate versions blob")?;
    let mut text = String::from_utf8(plain).context("versions blob is not valid UTF-8")?;

    let mut replacements = Vec::new();
    replacements.push(replace_header_uuid(&mut text)?);

    if include_standard_entries {
        for name in ["root", "version", "versions"] {
            if let Some(replacement) = replace_named_uuid_optional(&mut text, name)? {
                replacements.push(replacement);
            }
        }
    }
    let mut names = Vec::new();
    names.extend(changes.iter().cloned());
    names.sort();
    names.dedup();

    for name in names {
        match replace_named_uuid_optional(&mut text, &name)? {
            Some(replacement) => replacements.push(replacement),
            None if allow_missing_changes => {
                replacements.push(append_named_uuid(&mut text, &name)?)
            }
            None => return Err(anyhow!("versions entry not found: {name}")),
        }
    }

    let plain = text.into_bytes();
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);

    Ok(PatchedVersionsBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
        replacements,
    })
}

fn replace_header_uuid(text: &mut String) -> Result<VersionReplacement> {
    let marker = ",\"\",";
    let marker_start = text
        .find(marker)
        .ok_or_else(|| anyhow!("versions header marker not found"))?;
    let uuid_start = marker_start + marker.len();
    replace_uuid_at(text, uuid_start, "<generation>")
}

#[cfg(test)]
fn replace_named_uuid(text: &mut String, name: &str) -> Result<VersionReplacement> {
    let marker = format!("\"{name}\",");
    let marker_start = text
        .find(&marker)
        .ok_or_else(|| anyhow!("versions entry not found: {name}"))?;
    let uuid_start = marker_start + marker.len();
    replace_uuid_at(text, uuid_start, name)
}

fn replace_named_uuid_optional(
    text: &mut String,
    name: &str,
) -> Result<Option<VersionReplacement>> {
    let marker = format!("\"{name}\",");
    let Some(marker_start) = text.find(&marker) else {
        return Ok(None);
    };
    let uuid_start = marker_start + marker.len();
    replace_uuid_at(text, uuid_start, name).map(Some)
}

fn append_named_uuid(text: &mut String, name: &str) -> Result<VersionReplacement> {
    if name.contains('"') || name.contains('\r') || name.contains('\n') {
        return Err(anyhow!("unsupported versions entry name: {name}"));
    }
    increment_versions_entry_count(text)?;
    let insert_at = text
        .rfind('}')
        .ok_or_else(|| anyhow!("versions list closing brace not found"))?;
    let new_uuid = Uuid::new_v4().hyphenated().to_string();
    text.insert_str(insert_at, &format!(",\"{name}\",{new_uuid}"));
    Ok(VersionReplacement {
        name: name.to_string(),
        old_uuid: String::new(),
        new_uuid,
    })
}

fn increment_versions_entry_count(text: &mut String) -> Result<()> {
    let header_start = text
        .find("{1,")
        .ok_or_else(|| anyhow!("versions header count marker not found"))?;
    let count_start = header_start + "{1,".len();
    let count_end = text[count_start..]
        .find(',')
        .map(|offset| count_start + offset)
        .ok_or_else(|| anyhow!("versions header count end not found"))?;
    let count = text[count_start..count_end]
        .parse::<usize>()
        .with_context(|| {
            format!(
                "invalid versions entry count: {}",
                &text[count_start..count_end]
            )
        })?;
    text.replace_range(count_start..count_end, &(count + 1).to_string());
    Ok(())
}

fn replace_uuid_at(text: &mut String, uuid_start: usize, name: &str) -> Result<VersionReplacement> {
    let uuid_end = uuid_start + 36;
    let old_uuid = text
        .get(uuid_start..uuid_end)
        .ok_or_else(|| anyhow!("UUID for {name} exceeds text length"))?
        .to_string();
    if !is_uuid_like_ascii(old_uuid.as_bytes()) {
        return Err(anyhow!("invalid UUID for {name}: {old_uuid}"));
    }
    let new_uuid = Uuid::new_v4().hyphenated().to_string();
    text.replace_range(uuid_start..uuid_end, &new_uuid);
    Ok(VersionReplacement {
        name: name.to_string(),
        old_uuid,
        new_uuid,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn patches_versions_uuids_without_changing_text_length() {
        let mut text = "\u{feff}{1,2,\"\",aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,\"root\",bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,\"file.0\",cccccccc-cccc-4ccc-cccc-cccccccccccc}".to_string();
        let original_len = text.len();

        let header = super::replace_header_uuid(&mut text).unwrap();
        let root = super::replace_named_uuid(&mut text, "root").unwrap();
        let file = super::replace_named_uuid(&mut text, "file.0").unwrap();

        assert_eq!(text.len(), original_len);
        assert_eq!(header.old_uuid, "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
        assert_eq!(root.old_uuid, "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb");
        assert_eq!(file.old_uuid, "cccccccc-cccc-4ccc-cccc-cccccccccccc");
        assert!(super::is_uuid_like_ascii(header.new_uuid.as_bytes()));
        assert!(super::is_uuid_like_ascii(root.new_uuid.as_bytes()));
        assert!(super::is_uuid_like_ascii(file.new_uuid.as_bytes()));
    }

    #[test]
    fn patches_versions_blob_without_standard_entries() -> anyhow::Result<()> {
        let text = "\u{feff}{1,2,\"\",aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,\"file.0\",bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,\"file.1\",cccccccc-cccc-4ccc-cccc-cccccccccccc}";
        let input = super::deflate_raw(text.as_bytes())?;

        let patched = super::patch_versions_blob_bytes(&input, &["file.0".to_string()], true)?;
        let plain = super::inflate_raw(&patched.blob)?;
        let output = String::from_utf8(plain)?;

        assert_eq!(output.len(), text.len());
        assert_eq!(
            patched
                .replacements
                .iter()
                .map(|replacement| replacement.name.as_str())
                .collect::<Vec<_>>(),
            vec!["<generation>", "file.0"]
        );
        assert!(output.contains("\"file.1\",cccccccc-cccc-4ccc-cccc-cccccccccccc"));

        Ok(())
    }

    #[test]
    fn versions_blob_patch_still_requires_changed_entries() -> anyhow::Result<()> {
        let text = "\u{feff}{1,1,\"\",aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,\"file.0\",bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb}";
        let input = super::deflate_raw(text.as_bytes())?;

        let error =
            super::patch_versions_blob_bytes(&input, &["missing.0".to_string()], true).unwrap_err();

        assert_eq!(error.to_string(), "versions entry not found: missing.0");

        Ok(())
    }

    #[test]
    fn versions_blob_patch_can_add_missing_changed_entries() -> anyhow::Result<()> {
        let text = "\u{feff}{1,1,\"\",aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,\"file.0\",bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb}";
        let input = super::deflate_raw(text.as_bytes())?;

        let patched = super::patch_versions_blob_bytes_allowing_additions(
            &input,
            &["file.5".to_string()],
            true,
        )?;
        let plain = super::inflate_raw(&patched.blob)?;
        let output = String::from_utf8(plain)?;
        let addition = patched
            .replacements
            .iter()
            .find(|replacement| replacement.name == "file.5")
            .expect("missing added version replacement");

        assert!(output.starts_with("\u{feff}{1,2,"));
        assert_eq!(addition.old_uuid, "");
        assert!(super::is_uuid_like_ascii(addition.new_uuid.as_bytes()));
        assert!(output.contains(&format!("\"file.5\",{}", addition.new_uuid)));

        Ok(())
    }

    #[test]
    fn reports_versions_base_free_blockers() {
        let blockers = super::versions_base_free_blockers(6);

        assert_eq!(blockers.len(), 3);
        assert!(blockers[0].contains("6 selected Config row(s)"));
        assert!(blockers[0].contains("standard root/version/versions entries"));
        assert!(blockers[1].contains("FileName-to-generation-UUID map"));
        assert!(blockers[1].contains("unchanged row set"));
        assert!(blockers[2].contains("without dropping stable Config entries"));
        assert!(blockers[2].contains("database-specific GUIDs"));
    }
}
