//! Measured 8.3.27 file registry. Row-version bytes precede schema-ordered
//! fields physically; see the native #520 evidence, not SQL column offsets.
use std::collections::BTreeMap;
use std::ops::Range;

use crate::{
    blob,
    descr::{FieldKind, Table},
    error::{Error, Result, corrupt},
    object::Object,
    pager::Pager,
    record::decode_var_string,
};

const MAX_REGISTRY_ROWS: usize = 4096;
const MAX_REGISTRY_BYTES: u64 = 64 * 1024 * 1024;
const PROFILE: [(&str, FieldKind, u32, bool); 10] = [
    ("_IDRREF", FieldKind::Binary, 16, false),
    ("_EXTENSIONORDER", FieldKind::Numeric, 9, false),
    ("_EXTNAME", FieldKind::VarString, 255, true),
    ("_UPDATETIME", FieldKind::DateTime, 0, false),
    ("_EXTENSIONUSEPURPOSE", FieldKind::Numeric, 2, false),
    ("_EXTENSIONSCOPE", FieldKind::Numeric, 2, false),
    ("_EXTENSIONZIPPEDINFO", FieldKind::Image, 0, false),
    ("_MASTERNODE", FieldKind::Text, 0, true),
    ("_USEDINDISTRIBUTEDINFOBASE", FieldKind::Bool, 0, false),
    ("_VERSION", FieldKind::RowVersion, 0, false),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionRegistryRow {
    pub physical_id: [u8; 16],
    pub name: String,
    pub zipped_info: Vec<u8>,
}

struct Layout {
    fields: BTreeMap<&'static str, Range<usize>>,
    record_bytes: usize,
}
impl Layout {
    fn measured(table: &Table) -> Result<Self> {
        if table.fields.len() != PROFILE.len()
            || !table
                .fields
                .iter()
                .zip(PROFILE)
                .all(|(f, (name, kind, length, ci))| {
                    f.name == name
                        && f.kind == kind
                        && f.length == length
                        && f.precision == 0
                        && !f.nullable
                        && f.case_insensitive == ci
                })
        {
            return Err(Error::Unsupported(format!(
                "{}: unsupported extension registry layout",
                table.name
            )));
        }
        let mut fields = BTreeMap::new();
        let mut offset = 1; // physical record free flag
        for version in [true, false] {
            for (field, (name, _, _, _)) in table.fields.iter().zip(PROFILE) {
                if (field.kind == FieldKind::RowVersion) == version {
                    let end = offset + field.size();
                    fields.insert(name, offset..end);
                    offset = end;
                }
            }
        }
        Ok(Self {
            fields,
            record_bytes: offset,
        })
    }
    fn field<'a>(&self, record: &'a [u8], name: &str) -> Result<&'a [u8]> {
        let span = self
            .fields
            .get(name)
            .ok_or_else(|| corrupt(format!("registry has no field {name}")))?;
        record
            .get(span.clone())
            .ok_or_else(|| corrupt(format!("registry field {name} is truncated")))
    }
}

pub(crate) fn read_registry(
    pager: &mut Pager<'_>,
    tables: &BTreeMap<String, Table>,
) -> Result<Vec<ExtensionRegistryRow>> {
    // A next-generation registry has different selection/lifetime semantics.
    // Its presence must not silently turn a pending operation into active XML.
    let next = tables
        .get("_EXTENSIONSINFONGS")
        .ok_or_else(|| Error::Unsupported("missing _EXTENSIONSINFONGS registry".into()))?;
    if !read_table(pager, next)?.is_empty() {
        return Err(Error::Unsupported(
            "prepared next-generation extension registry is not supported".into(),
        ));
    }
    let table = tables
        .get("_EXTENSIONSINFO")
        .ok_or_else(|| Error::Unsupported("missing _EXTENSIONSINFO registry".into()))?;
    read_table(pager, table)
}

fn read_table(pager: &mut Pager<'_>, table: &Table) -> Result<Vec<ExtensionRegistryRow>> {
    let layout = Layout::measured(table)?;
    let records = Object::open(pager, table.records_page)?;
    if records.len > ((MAX_REGISTRY_ROWS + 1) * layout.record_bytes) as u64 {
        return Err(Error::Refused(
            "extension registry exceeds its row budget".into(),
        ));
    }
    let records = records.read_all(pager)?;
    if !records.len().is_multiple_of(layout.record_bytes) {
        return Err(corrupt("extension registry record object is truncated"));
    }
    if records.is_empty() {
        return Ok(Vec::new());
    }
    if records.first() != Some(&1) {
        return Err(corrupt("invalid extension registry record-zero header"));
    }
    let blobs = Object::open(pager, table.blob_page)?;
    if blobs.len > MAX_REGISTRY_BYTES {
        return Err(Error::Refused(
            "extension registry exceeds its blob budget".into(),
        ));
    }
    let blobs = blobs.read_all(pager)?;
    decode_records_checked(&layout, &records, &blobs, &|| pager.check_read())
}

fn binary_value(
    layout: &Layout,
    record: &[u8],
    blobs: &[u8],
    name: &str,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let value = layout.field(record, name)?;
    let (head, len) = value.split_at(4);
    let head = u32::from_le_bytes(
        head.try_into()
            .map_err(|_| corrupt("invalid registry blob pointer"))?,
    );
    let len = u32::from_le_bytes(
        len.try_into()
            .map_err(|_| corrupt("invalid registry blob length"))?,
    );
    if u64::from(len) > MAX_REGISTRY_BYTES {
        return Err(Error::Refused(
            "extension registry value exceeds its budget".into(),
        ));
    }
    blob::read_chain_checked(blobs, head, Some(u64::from(len)), check)
}

#[cfg(test)]
fn decode_records(
    layout: &Layout,
    records: &[u8],
    blobs: &[u8],
) -> Result<Vec<ExtensionRegistryRow>> {
    decode_records_checked(layout, records, blobs, &|| Ok(()))
}

fn decode_records_checked(
    layout: &Layout,
    records: &[u8],
    blobs: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<ExtensionRegistryRow>> {
    check()?;
    let mut rows = Vec::new();
    let mut retained_bytes = 0u64;
    for record in records.chunks_exact(layout.record_bytes).skip(1) {
        check()?;
        match record.first() {
            Some(1) => continue,
            Some(0) => {}
            _ => return Err(corrupt("invalid extension registry free flag")),
        }
        let name = decode_var_string(layout.field(record, "_EXTNAME")?, 255)?;
        if name.is_empty() {
            return Err(corrupt("empty extension registry name"));
        }
        let physical_id = layout
            .field(record, "_IDRREF")?
            .try_into()
            .map_err(|_| corrupt("invalid registry identity"))?;
        if physical_id == [0; 16] {
            return Err(corrupt("empty extension registry identity"));
        }
        for (field, digits, maximum) in [
            ("_EXTENSIONORDER", 9, 999_999_999),
            ("_EXTENSIONUSEPURPOSE", 2, 2),
            ("_EXTENSIONSCOPE", 2, 1),
        ] {
            if crate::record::unpack_decimal(layout.field(record, field)?, digits)? > maximum {
                return Err(Error::Unsupported(format!(
                    "unsupported extension registry value {field}"
                )));
            }
        }
        if layout.field(record, "_USEDINDISTRIBUTEDINFOBASE")? != [0] {
            return Err(Error::Unsupported(
                "distributed file extensions are not supported".into(),
            ));
        }
        let master_ref = layout.field(record, "_MASTERNODE")?;
        if u32::from_le_bytes(master_ref[4..].try_into().unwrap()) != 68 {
            return Err(Error::Unsupported(
                "unsupported file extension master-node profile".into(),
            ));
        }
        let expected_master: Vec<_> = format!("0:{}", "0".repeat(32))
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        if binary_value(layout, record, blobs, "_MASTERNODE", check)? != expected_master {
            return Err(Error::Unsupported(
                "distributed file extensions are not supported".into(),
            ));
        }
        let zipped_info = binary_value(layout, record, blobs, "_EXTENSIONZIPPEDINFO", check)?;
        retained_bytes = retained_bytes
            .checked_add(zipped_info.len() as u64)
            .filter(|n| *n <= MAX_REGISTRY_BYTES)
            .ok_or_else(|| {
                Error::Refused("extension registry exceeds its retained-value budget".into())
            })?;
        rows.push(ExtensionRegistryRow {
            physical_id,
            name,
            zipped_info,
        });
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Field;
    #[test]
    fn a_live_next_generation_registry_is_never_read_as_active() {
        use std::{fs, io::Read, path::Path};
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/file-infobase/extensions-8327/applied.1CD.gz");
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(fs::File::open(fixture).unwrap())
            .read_to_end(&mut bytes)
            .unwrap();
        let path =
            std::env::temp_dir().join(format!("filedb-next-registry-{}.1CD", std::process::id()));
        let _scratch = crate::test_scratch::TestScratch::adopt(&path);
        fs::write(&path, bytes).unwrap();
        let mut db = crate::FileDb::open(&path).unwrap();
        let mut next = db.tables["_EXTENSIONSINFO"].clone();
        next.name = "_EXTENSIONSINFONGS".into();
        db.tables.insert(next.name.clone(), next);
        assert!(
            matches!(db.extension_registry(),Err(Error::Unsupported(message)) if message.contains("next-generation"))
        );
        drop(db);
    }
    fn table() -> Table {
        Table {
            name: "_EXTENSIONSINFO".into(),
            fields: PROFILE
                .into_iter()
                .map(|(name, kind, length, case_insensitive)| Field {
                    name: name.into(),
                    kind,
                    length,
                    precision: 0,
                    nullable: false,
                    case_insensitive,
                })
                .collect(),
            index_names: vec![],
            records_page: 1,
            blob_page: 2,
            index_page: 3,
        }
    }
    #[test]
    fn version_precedes_named_fields_and_unknown_profiles_are_refused() {
        let mut table = table();
        let layout = Layout::measured(&table).unwrap();
        assert_eq!(layout.record_bytes, 578);
        assert_eq!(layout.fields["_VERSION"], 1..17);
        assert_eq!(layout.fields["_IDRREF"], 17..33);
        assert_eq!(layout.fields["_EXTNAME"], 38..550);
        table.fields[0].nullable = true;
        assert!(Layout::measured(&table).is_err());
        table = self::table();
        table.fields.swap(0, 1);
        assert!(Layout::measured(&table).is_err());
    }
    #[test]
    fn registry_rows_decode_full_values_and_refuse_corrupt_names_and_blobs() {
        let layout = Layout::measured(&table()).unwrap();
        let mut record = vec![0; layout.record_bytes];
        record[layout.fields["_IDRREF"].clone()].copy_from_slice(&[7; 16]);
        crate::record::encode_var_string(
            "Расширение",
            255,
            &mut record[layout.fields["_EXTNAME"].clone()],
        )
        .unwrap();
        for (name, digits) in [
            ("_EXTENSIONORDER", 9),
            ("_EXTENSIONUSEPURPOSE", 2),
            ("_EXTENSIONSCOPE", 2),
        ] {
            record[layout.fields[name].clone()]
                .copy_from_slice(&crate::record::pack_decimal(0, digits));
        }
        let master: Vec<_> = format!("0:{}", "0".repeat(32))
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let (blobs, refs) = blob::build(&[b"payload", &master], 0).unwrap();
        let master_span = layout.fields["_MASTERNODE"].clone();
        record[master_span.start..master_span.start + 4].copy_from_slice(&refs[1].0.to_le_bytes());
        record[master_span.start + 4..master_span.end].copy_from_slice(&refs[1].1.to_le_bytes());
        let span = layout.fields["_EXTENSIONZIPPEDINFO"].clone();
        record[span.start..span.start + 4].copy_from_slice(&refs[0].0.to_le_bytes());
        record[span.start + 4..span.end].copy_from_slice(&refs[0].1.to_le_bytes());
        let mut records = vec![0; layout.record_bytes];
        records.extend_from_slice(&record);
        assert_eq!(
            decode_records(&layout, &records, &blobs).unwrap(),
            vec![ExtensionRegistryRow {
                physical_id: [7; 16],
                name: "Расширение".into(),
                zipped_info: b"payload".to_vec()
            }]
        );
        let refusal = decode_records_checked(&layout, &records, &blobs, &|| {
            Err(Error::Refused("registry read cancelled".into()))
        });
        assert!(
            matches!(refusal, Err(Error::Refused(message)) if message == "registry read cancelled")
        );
        let checks = std::cell::Cell::new(0);
        let refusal = decode_records_checked(&layout, &records, &blobs, &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 4 {
                Err(Error::Refused("registry block cancelled".into()))
            } else {
                Ok(())
            }
        });
        assert!(
            matches!(refusal, Err(Error::Refused(message)) if message == "registry block cancelled")
        );
        assert_eq!(checks.get(), 4);
        records[layout.record_bytes + layout.fields["_EXTNAME"].start..][..2]
            .copy_from_slice(&256u16.to_le_bytes());
        assert!(decode_records(&layout, &records, &blobs).is_err());
        records[layout.record_bytes..].copy_from_slice(&record);
        records[layout.record_bytes + span.start..][..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_records(&layout, &records, &blobs).is_err());
    }
    #[test]
    fn registry_refuses_unmeasured_scope_and_distribution() {
        let layout = Layout::measured(&table()).unwrap();
        let mut record = vec![0; layout.record_bytes];
        record[layout.fields["_IDRREF"].clone()].copy_from_slice(&[7; 16]);
        crate::record::encode_var_string(
            "CleanRoom",
            255,
            &mut record[layout.fields["_EXTNAME"].clone()],
        )
        .unwrap();
        for (name, digits) in [
            ("_EXTENSIONORDER", 9),
            ("_EXTENSIONUSEPURPOSE", 2),
            ("_EXTENSIONSCOPE", 2),
        ] {
            record[layout.fields[name].clone()]
                .copy_from_slice(&crate::record::pack_decimal(0, digits));
        }
        let master: Vec<_> = format!("0:{}", "0".repeat(32))
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let (blobs, refs) = blob::build(&[b"payload", &master], 0).unwrap();
        for (name, index) in [("_EXTENSIONZIPPEDINFO", 0), ("_MASTERNODE", 1)] {
            let span = layout.fields[name].clone();
            record[span.start..span.start + 4].copy_from_slice(&refs[index].0.to_le_bytes());
            record[span.start + 4..span.end].copy_from_slice(&refs[index].1.to_le_bytes());
        }
        let mut records = vec![0; layout.record_bytes];
        records.extend_from_slice(&record);
        assert!(decode_records(&layout, &records, &blobs).is_ok());
        let mut accepted = Vec::new();
        for (name, value) in [
            ("_EXTENSIONSCOPE", crate::record::pack_decimal(2, 2)),
            ("_EXTENSIONUSEPURPOSE", crate::record::pack_decimal(3, 2)),
            ("_USEDINDISTRIBUTEDINFOBASE", vec![1]),
        ] {
            records[layout.record_bytes..].copy_from_slice(&record);
            let span = layout.fields[name].clone();
            records[layout.record_bytes + span.start..layout.record_bytes + span.end]
                .copy_from_slice(&value);
            if decode_records(&layout, &records, &blobs).is_ok() {
                accepted.push(name);
            }
        }
        assert!(
            accepted.is_empty(),
            "unmeasured registry values were accepted: {accepted:?}"
        );
    }
}
