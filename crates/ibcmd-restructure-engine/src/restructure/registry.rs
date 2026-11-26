//! The object registry (`Params` `1a621f0f-....si`, the "search information" of the configuration): a
//! pre-order list of every metadata object, `uuid, owner uuid, kind, "name", {synonyms}, flag, flag`.
//! The kind is the index of the object's class id in the row's own class list; the count in the header is
//! the number of records.
//!
//! A new attribute of a catalog or a document is one more record (kind 36 and 41 in the БСП, flags `0,0`)
//! among its owner's children: after the previous attribute in metadata order, or, when it is the first,
//! between the groups the other owners of the kind list before and after the attributes. The editing is
//! the apply track's [`crate::mssql_config_apply::si`] (measured there for new forms and templates); this
//! module only says where the attributes go. Measured on case a2 and on the native twin of the types case
//! (`docs/apply/restructuring.md`).

use std::collections::BTreeSet;

use anyhow::{Context, Result};

use crate::mssql_config_apply::si::{self, Insertion, NewRecord};
use crate::restructure::catalog::AttributeFacts;
use crate::restructure::object::ObjectKind;

/// The `Params` row of the registry.
pub const REGISTRY_ROW: &str = "1a621f0f-5568-4183-bd9f-f6ef670e7090.si";
/// The row of the versions of the `*.si` rows.
pub const VERSIONS_ROW: &str = "siVersions";

/// The registry text with the records of the added attributes of one object.
pub struct ObjectAdditions<'a> {
    pub kind: ObjectKind,
    pub owner: &'a str,
    /// All the attributes of the object, in metadata order, as the staged descriptor lists them.
    pub attributes: &'a [AttributeFacts],
    /// The uuids of the new ones.
    pub added: &'a BTreeSet<String>,
}

/// The records for the added attributes, at their places; the count follows. Nothing else changes.
pub fn add_attributes(text: &[u8], objects: &[ObjectAdditions<'_>]) -> Result<Vec<u8>> {
    let main = si::parse(text).context("the object registry does not parse")?;
    let mut insertions = Vec::new();
    for object in objects {
        let kind = main
            .kind_of_class(object.kind.attribute_class())
            .with_context(|| {
                format!(
                    "the registry has no class {} for the attributes of a {}",
                    object.kind.attribute_class(),
                    object.kind.label()
                )
            })?;
        // The new attributes that follow the same existing one go in together, in metadata order.
        let mut previous: Option<&AttributeFacts> = None;
        let mut pending: Vec<&AttributeFacts> = Vec::new();
        let mut flush = |previous: Option<&AttributeFacts>,
                         next: Option<&AttributeFacts>,
                         pending: &mut Vec<&AttributeFacts>|
         -> Result<()> {
            if pending.is_empty() {
                return Ok(());
            }
            let at = main.place(
                object.owner,
                kind,
                previous.map(|attribute| attribute.uuid.as_str()),
                next.map(|attribute| attribute.uuid.as_str()),
            )?;
            let records = pending
                .drain(..)
                .map(|attribute| NewRecord {
                    uuid: attribute.uuid.clone(),
                    parent: object.owner.to_owned(),
                    kind,
                    name: attribute.name.clone(),
                    synonyms: attribute.synonyms.clone(),
                    flags: (0, 0),
                })
                .collect();
            insertions.push(Insertion { at, records });
            Ok(())
        };
        for attribute in object.attributes {
            if object.added.contains(&attribute.uuid) {
                pending.push(attribute);
            } else {
                flush(previous, Some(attribute), &mut pending)?;
                previous = Some(attribute);
            }
        }
        flush(previous, None, &mut pending)?;
    }
    si::insert_records(text, &main, &insertions)
}

/// The registry text without the records of the removed attributes; the count follows. Nothing else changes.
/// A record spans its seven members; with it goes the separator after it (before it for the last record).
pub fn remove_attributes(text: &[u8], removed: &BTreeSet<String>) -> Result<Vec<u8>> {
    if removed.is_empty() {
        return Ok(text.to_vec());
    }
    let main = si::parse(text).context("the object registry does not parse")?;
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for uuid in removed {
        let index = main
            .index_of(uuid)
            .with_context(|| format!("the object registry does not list {uuid}"))?;
        anyhow::ensure!(
            main.subtree_end(index) == index + 1,
            "the registry record {} has descendants: it is no attribute",
            main.records[index].name
        );
        let range = if index + 1 < main.records.len() {
            (main.records[index].start, main.records[index + 1].start)
        } else {
            (main.records[index - 1].end, main.records[index].end)
        };
        ranges.push(range);
    }
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.0 <= last.1 => last.1 = last.1.max(range.1),
            _ => merged.push(range),
        }
    }
    let mut out = Vec::with_capacity(text.len());
    let mut from = 0;
    for (start, end) in merged {
        out.extend_from_slice(&text[from..start]);
        from = end;
    }
    out.extend_from_slice(&text[from..]);

    // The count: the number in front of the first record.
    let count = main.records.len();
    let first = main.records[0].start;
    let mut end = first;
    while end > 0 && text[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    anyhow::ensure!(
        end > 0 && text[end - 1] == b',',
        "the record count is not in front of the first record"
    );
    end -= 1;
    let mut start = end;
    while start > 0 && text[start - 1].is_ascii_digit() {
        start -= 1;
    }
    anyhow::ensure!(
        std::str::from_utf8(&text[start..end])
            .ok()
            .and_then(|digits| digits.parse::<usize>().ok())
            == Some(count),
        "the record count in front of the first record is not {count}"
    );
    // `start` is before every removed range (the first record is the front of the list), so the offsets
    // of the count are the same in the output.
    let updated = (count - removed.len()).to_string();
    out.splice(start..end, updated.bytes());

    let check = si::parse(&out).context("the registry without the attributes does not parse")?;
    anyhow::ensure!(
        check.records.len() == count - removed.len(),
        "the registry lost {} records, expected {}",
        count - check.records.len(),
        removed.len()
    );
    Ok(out)
}

/// `siVersions` with a new version for each named row.
pub fn bump_versions(text: &[u8], rows: &[&str]) -> Result<Vec<u8>> {
    let mut out = text.to_vec();
    for row in rows {
        out = si::set_si_version(&out, row, uuid::Uuid::new_v4())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NIL: &str = "00000000-0000-0000-0000-000000000000";
    const CONFIGURATION: &str = "66193438-abc5-410b-a1f1-a204102d1a62";
    const CATALOG: &str = "5eab8a1b-070f-4dcf-bdcc-a259c62c3693";

    fn attribute(index: usize) -> String {
        format!("a1a1a1a1-0000-4000-8000-00000000000{index}")
    }

    fn record(uuid: &str, parent: &str, kind: usize, name: &str) -> String {
        format!("{uuid},{parent},{kind},\"{name}\",\n{{1,1,\n{{\"ru\",\"{name}\"}}\n}},0,0")
    }

    /// A configuration, a catalog with four attributes, another catalog.
    fn sample() -> String {
        let mut records = vec![
            record(CONFIGURATION, NIL, 0, "Конфигурация"),
            record(CATALOG, CONFIGURATION, 1, "Справочник"),
        ];
        for index in 1..=4 {
            records.push(record(
                &attribute(index),
                CATALOG,
                2,
                &format!("Реквизит{index}"),
            ));
        }
        records.push(record(
            "b2b2b2b2-0000-4000-8000-000000000001",
            CONFIGURATION,
            1,
            "Другой",
        ));
        format!(
            "\u{feff}{{4,\r\n{{2,cf4abeab-37b2-11d4-940f-008048da11f9,cf4abea7-37b2-11d4-940f-008048da11f9}},\r\n{{{},{}}}\r\n}}",
            records.len(),
            records.join(",")
        )
        .replace('\n', "\r\n")
        .replace("\r\r\n", "\r\n")
    }

    fn names(text: &[u8]) -> Vec<String> {
        si::parse(text)
            .unwrap()
            .records
            .iter()
            .map(|record| record.name.clone())
            .collect()
    }

    #[test]
    fn removed_attributes_take_their_records_and_the_count_with_them() {
        let text = sample();
        let removed = |indexes: &[usize]| -> BTreeSet<String> {
            indexes.iter().map(|index| attribute(*index)).collect()
        };
        let all = names(text.as_bytes());
        assert_eq!(all.len(), 7);
        // The first, a middle, the last of the attributes, two in a row, and all of them.
        for (indexes, gone) in [
            (vec![1], vec!["Реквизит1"]),
            (vec![3], vec!["Реквизит3"]),
            (vec![4], vec!["Реквизит4"]),
            (vec![2, 3], vec!["Реквизит2", "Реквизит3"]),
            (
                vec![1, 2, 3, 4],
                vec!["Реквизит1", "Реквизит2", "Реквизит3", "Реквизит4"],
            ),
        ] {
            let out = remove_attributes(text.as_bytes(), &removed(&indexes)).unwrap();
            let kept: Vec<String> = all
                .iter()
                .filter(|name| !gone.contains(&name.as_str()))
                .cloned()
                .collect();
            assert_eq!(names(&out), kept, "{indexes:?}");
            // the header count is the number of records, the text before the first record is unchanged
            let out = String::from_utf8(out).unwrap();
            assert!(
                out.contains(&format!("{{{},{CONFIGURATION}", kept.len())),
                "{indexes:?}"
            );
        }
        // The record of the last attribute of the whole list goes with the separator before it.
        let mut last = text.clone();
        last = last.replace(
            "b2b2b2b2-0000-4000-8000-000000000001",
            "a1a1a1a1-0000-4000-8000-000000000009",
        );
        let out = remove_attributes(
            last.as_bytes(),
            &BTreeSet::from(["a1a1a1a1-0000-4000-8000-000000000009".to_owned()]),
        )
        .unwrap();
        assert_eq!(names(&out).len(), 6);
        // Refusals: an unknown record, an owner with descendants, nothing to remove is no change.
        assert!(remove_attributes(text.as_bytes(), &BTreeSet::from([NIL.to_owned()])).is_err());
        assert!(remove_attributes(text.as_bytes(), &BTreeSet::from([CATALOG.to_owned()])).is_err());
        assert_eq!(
            remove_attributes(text.as_bytes(), &BTreeSet::new()).unwrap(),
            text.as_bytes()
        );
    }
}
