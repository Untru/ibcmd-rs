//! A live full-tree import may retain the target's representation of the same
//! root or business-process flowchart. Compilation itself still uses only XML.

use anyhow::Result;

use super::{EmptyStage, EmptyStageRow, SqlExec, inflate_raw};
use crate::mssql::{fetch_effective_candidates, quote_string};
use crate::mssql_effective_row::alias_pattern;

fn counter(plain: &[u8]) -> Option<(&[u8], u32)> {
    let body = plain.strip_suffix(b"}")?;
    let comma = body.iter().rposition(|&byte| byte == b',')?;
    let digits = &body[comma + 1..];
    if !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some((
        &body[..comma],
        std::str::from_utf8(digits).ok()?.parse().ok()?,
    ))
}

// A stored row is eligible only when the entire raw DEFLATE stream accounts
// for its bytes. The source provides an upper bound on the decoded size.
fn stored_plain(blob: &[u8], limit: usize) -> Option<Vec<u8>> {
    let mut plain = Vec::with_capacity(limit.checked_add(1)?);
    let mut decoder = flate2::Decompress::new(false);
    let status = decoder
        .decompress_vec(blob, &mut plain, flate2::FlushDecompress::Finish)
        .ok()?;
    (status == flate2::Status::StreamEnd
        && decoder.total_in() == blob.len() as u64
        && plain.len() <= limit)
        .then_some(plain)
}

fn same_flowchart(compiled: &[u8], stored: &[u8]) -> bool {
    compiled == stored
        || matches!((counter(compiled), counter(stored)),
        (Some((left, items)), Some((right, high_water))) if left == right && items <= high_water)
}

fn retain_row(
    sql: &SqlExec,
    database: &str,
    row: &mut EmptyStageRow,
    flowchart: bool,
) -> Result<bool> {
    let mut candidates = fetch_effective_candidates(sql, database, &row.file_name)?;
    let Some((name, size, blob)) = candidates.pop() else {
        return Ok(false);
    };
    // An alias or multipart body needs its generation/assembly context.
    if !candidates.is_empty()
        || !name.eq_ignore_ascii_case(&row.file_name)
        || blob.len() > crate::mssql::CONFIG_ROW_PART_BYTES
        || usize::try_from(size).ok() != Some(blob.len())
    {
        return Ok(false);
    }
    let compiled = inflate_raw(&row.blob)?;
    // A u32 counter can grow from one digit to ten.
    let limit = compiled.len() + if flowchart { 9 } else { 0 };
    let Some(active) = stored_plain(&blob, limit) else {
        return Ok(false);
    };
    let equivalent = if flowchart {
        same_flowchart(&compiled, &active)
    } else {
        compiled == active
    };
    if !equivalent {
        return Ok(false);
    }
    row.blob = blob;
    row.plain = Some(active);
    Ok(true)
}

pub(super) fn retain(stage: &mut EmptyStage, sql: &SqlExec, database: &str) -> Result<Vec<String>> {
    let mut retained = Vec::new();
    let Some(root) = stage.service.iter_mut().find(|row| row.file_name == "root") else {
        return Ok(retained);
    };
    if !retain_row(sql, database, root, false)? {
        return Ok(retained);
    }
    retained.push(root.file_name.clone());
    let Some(suffix) = crate::metadata_model::bodies_rows::body_row_suffix("BusinessProcess")
    else {
        return Ok(retained);
    };
    for object in stage
        .objects
        .iter_mut()
        .filter(|object| object.kind == "BusinessProcess")
    {
        let name = format!("{}.{suffix}", object.uuid);
        if let Some(row) = object.rows.iter_mut().find(|row| row.file_name == name)
            && retain_row(sql, database, row, true)?
        {
            retained.push(name);
        }
    }
    Ok(retained)
}

/// Recheck every borrowed value inside the ConfigSave transaction, before
/// replacing the previous stage. Range locks also cover absent aliases/parts.
pub(super) fn guard_sql(names: &[String], stage: &str) -> String {
    if names.is_empty() {
        return String::new();
    }
    let rows = names
        .iter()
        .map(|name| {
            format!(
                "(N'{}',N'{}')",
                quote_string(name),
                quote_string(&alias_pattern(name))
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "DECLARE @retained TABLE (name nvarchar(128) PRIMARY KEY, alias_pattern nvarchar(200));
        INSERT INTO @retained (name,alias_pattern) SELECT name,pattern FROM (VALUES {rows}) v(name,pattern);
        IF EXISTS (SELECT 1 FROM @retained n
        LEFT JOIN {stage} s ON s.FileName = n.name COLLATE DATABASE_DEFAULT AND s.PartNo = 0
        LEFT JOIN dbo.Config c WITH (UPDLOCK,HOLDLOCK) ON c.FileName = n.name COLLATE DATABASE_DEFAULT AND c.PartNo = 0
        WHERE c.FileName IS NULL OR s.FileName IS NULL OR c.BinaryData IS NULL
           OR c.DataSize IS NULL OR c.DataSize <> s.DataSize
           OR DATALENGTH(c.BinaryData) <> s.DataSize OR c.BinaryData <> s.BinaryData
           OR EXISTS (SELECT 1 FROM dbo.Config a WITH (UPDLOCK,HOLDLOCK)
               WHERE a.FileName LIKE n.alias_pattern COLLATE DATABASE_DEFAULT ESCAPE N'\\'
                  OR (a.FileName = n.name COLLATE DATABASE_DEFAULT AND a.PartNo <> 0)))
        THROW 55006, 'An active root or flowchart changed during import. Retry the import.', 1;\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuse_requires_a_complete_bounded_stream() {
        let plain = b"native row";
        let mut blob = crate::module_blob::deflate_raw(plain).unwrap();
        assert_eq!(stored_plain(&blob, plain.len()).unwrap(), plain);
        assert!(stored_plain(&blob, plain.len() - 1).is_none());
        assert!(stored_plain(&blob[..blob.len() - 1], plain.len()).is_none());
        blob.push(0);
        assert!(stored_plain(&blob, plain.len()).is_none());
    }

    #[test]
    fn only_a_non_decreasing_hidden_counter_can_be_retained() {
        let generated = b"{5,{1},2,1,{10},2,{20},2}";
        assert!(same_flowchart(generated, b"{5,{1},2,1,{10},2,{20},8}"));
        assert!(!same_flowchart(generated, b"{5,{1},2,1,{11},2,{20},8}"));
        assert!(!same_flowchart(generated, b"{5,{1},2,1,{10},2,{20},1}"));
        assert!(!same_flowchart(generated, b"{5,{1},2,1,{10},2,{20},8,2}"));
        assert!(!same_flowchart(generated, b"{5,{1},2,1,{10},2,{20},x}"));
    }
}
