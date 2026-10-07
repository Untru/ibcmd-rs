//! The `versions` row, the dynamic-update markers and `MobileVersions.dat`.

use std::io::Read;

use anyhow::{Result, anyhow, bail};
use flate2::read::DeflateDecoder;
use uuid::Uuid;

const MAX_INFLATED: usize = 64 * 1024 * 1024;

/// Inflates a raw-deflate row (the storage form of `versions`, descriptors
/// and most bodies), refusing a value that inflates to more than 64 MiB.
pub fn inflate_row(blob: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = DeflateDecoder::new(blob).take((MAX_INFLATED + 1) as u64);
    let mut plain = Vec::new();
    decoder
        .read_to_end(&mut plain)
        .map_err(|error| anyhow!("raw deflate failed: {error}"))?;
    if plain.len() > MAX_INFLATED {
        bail!("a row inflates to more than {MAX_INFLATED} bytes");
    }
    Ok(plain)
}

/// The storage form of a row: raw deflate (the inverse of [`inflate_row`]).
pub fn deflate_row(plain: &[u8]) -> Result<Vec<u8>> {
    use std::io::Write;

    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(plain)
        .map_err(|error| anyhow!("raw deflate failed: {error}"))?;
    encoder
        .finish()
        .map_err(|error| anyhow!("raw deflate failed: {error}"))
}

pub fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)
}

/// `{1,<count>,"",<generation>,"<name>",<version>,...}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionsRow {
    pub generation: Uuid,
    /// Declared entry count.
    pub count: usize,
    /// Row name -> version, in stored order.
    pub entries: Vec<(String, Uuid)>,
}

pub fn parse_versions(blob: &[u8]) -> Result<VersionsRow> {
    let plain = inflate_row(blob).map_err(|error| anyhow!("versions: {error}"))?;
    parse_versions_plain(&plain)
}

/// Parse an already fully validated/inflated ordinary version map. Storage
/// callers can derive allocation from the actual stream without decoding twice.
pub(super) fn parse_versions_plain(plain: &[u8]) -> Result<VersionsRow> {
    let text = std::str::from_utf8(strip_bom(plain))
        .map_err(|_| anyhow!("versions: the row is not UTF-8"))?;
    let inner = text
        .trim()
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .ok_or_else(|| anyhow!("versions: the row is not braced"))?;
    let mut fields = inner.splitn(5, ',');
    if fields.next().map(str::trim) != Some("1") {
        bail!("versions: unsupported header tag");
    }
    let count = fields
        .next()
        .map(str::trim)
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| anyhow!("versions: invalid entry count"))?;
    if fields.next().map(str::trim) != Some("\"\"") {
        bail!("versions: unsupported header shape");
    }
    let generation = fields
        .next()
        .and_then(|value| Uuid::parse_str(value.trim()).ok())
        .ok_or_else(|| anyhow!("versions: invalid generation"))?;
    let rest = fields.next().unwrap_or("");
    let mut entries = Vec::with_capacity(count);
    let mut remaining = rest.trim();
    while !remaining.is_empty() {
        let after_quote = remaining
            .strip_prefix('"')
            .ok_or_else(|| anyhow!("versions: an entry does not start with a quoted name"))?;
        let end = after_quote
            .find('"')
            .ok_or_else(|| anyhow!("versions: an entry name is not terminated"))?;
        let name = &after_quote[..end];
        let after_name = after_quote[end + 1..]
            .strip_prefix(',')
            .ok_or_else(|| anyhow!("versions: an entry has no version"))?;
        let (version_text, tail) = match after_name.find(',') {
            Some(position) => (
                &after_name[..position],
                after_name[position + 1..].trim_start(),
            ),
            None => (after_name, ""),
        };
        let version = Uuid::parse_str(version_text.trim())
            .map_err(|_| anyhow!("versions: entry {name} has an invalid version"))?;
        entries.push((name.to_owned(), version));
        remaining = tail;
    }
    // The header counts the empty-named entry that carries the generation.
    if entries.len() + 1 != count {
        bail!(
            "versions: {} entries (and the generation) but the header says {count}",
            entries.len()
        );
    }
    Ok(VersionsRow {
        generation,
        count,
        entries,
    })
}

/// The dynamic generations a database carries: `Config.DynamicallyUpdated`
/// `{1,N,g1..gN}` and `Params.DynamicallyUpdated` `{0,N+1,ordinary,g1..gN}`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DynamicHistory {
    /// The generation the ordinary `versions` row carries.
    pub ordinary: Option<Uuid>,
    /// Oldest first.
    pub generations: Vec<Uuid>,
}

fn marker_fields(blob: &[u8]) -> Result<Vec<String>> {
    let text = std::str::from_utf8(strip_bom(blob))
        .map_err(|_| anyhow!("a dynamic marker is not UTF-8"))?;
    let inner = text
        .trim()
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .ok_or_else(|| anyhow!("a dynamic marker is not braced"))?;
    Ok(inner
        .split(',')
        .map(|field| field.trim().to_owned())
        .collect())
}

/// Reads the two markers (each `None` when the row is absent). Both present
/// and consistent, or both absent, or -- after a native apply of body rows
/// alone, which clears the `Config` marker and leaves the `Params` one -- only
/// the `Params` marker, which then names no overlay; anything else fails
/// closed.
pub fn parse_dynamic_history(
    config_marker: Option<&[u8]>,
    params_marker: Option<&[u8]>,
) -> Result<DynamicHistory> {
    match (config_marker, params_marker) {
        (None, None) => Ok(DynamicHistory::default()),
        (Some(config), Some(params)) => {
            let config = marker_fields(config)?;
            let params = marker_fields(params)?;
            let config_count = config
                .get(1)
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| anyhow!("Config.DynamicallyUpdated has no count"))?;
            let params_count = params
                .get(1)
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| anyhow!("Params.DynamicallyUpdated has no count"))?;
            if config.first().map(String::as_str) != Some("1")
                || config_count == 0
                || config.len() != config_count + 2
            {
                bail!("unsupported Config.DynamicallyUpdated marker");
            }
            if params.first().map(String::as_str) != Some("0")
                || params_count != config_count + 1
                || params.len() != params_count + 2
            {
                bail!("unsupported Params.DynamicallyUpdated marker");
            }
            if config_count > 4096 {
                bail!("the dynamic generation history exceeds 4096 entries");
            }
            let parse = |values: &[String]| -> Result<Vec<Uuid>> {
                values
                    .iter()
                    .map(|value| {
                        Uuid::parse_str(value)
                            .map_err(|_| anyhow!("a dynamic marker holds an invalid generation"))
                    })
                    .collect()
            };
            let generations = parse(&config[2..])?;
            let params_generations = parse(&params[3..])?;
            if generations != params_generations {
                bail!("the Config and Params dynamic generation histories disagree");
            }
            let ordinary = Uuid::parse_str(&params[2]).map_err(|_| {
                anyhow!("Params.DynamicallyUpdated has an invalid ordinary generation")
            })?;
            Ok(DynamicHistory {
                ordinary: Some(ordinary),
                generations,
            })
        }
        (None, Some(params)) => {
            let params = marker_fields(params)?;
            let count = params
                .get(1)
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| anyhow!("Params.DynamicallyUpdated has no count"))?;
            if params.first().map(String::as_str) != Some("0")
                || count == 0
                || params.len() != count + 2
            {
                bail!("unsupported Params.DynamicallyUpdated marker");
            }
            let ordinary = Uuid::parse_str(&params[2]).map_err(|_| {
                anyhow!("Params.DynamicallyUpdated has an invalid ordinary generation")
            })?;
            Ok(DynamicHistory {
                ordinary: Some(ordinary),
                generations: Vec::new(),
            })
        }
        (Some(_), None) => {
            bail!("Config.DynamicallyUpdated is present but Params.DynamicallyUpdated is not")
        }
    }
}

/// `Files.MobileVersions.dat`: a UTF-8 BOM and `{<count>,<guid>,...}`, newest
/// first, at most 1000 entries. Every apply puts a fresh random GUID at the
/// head and drops the last when the list is full (measured on 8.3.27.2214).
pub fn mobile_versions_prepend(current: &[u8], fresh: Uuid) -> Result<Vec<u8>> {
    let fields = marker_fields(current)?;
    let declared = fields
        .first()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| anyhow!("MobileVersions.dat has no count"))?;
    let existing = &fields[1..];
    if existing.len() > declared || declared > 1000 {
        bail!(
            "MobileVersions.dat holds {} entries for a count of {declared}",
            existing.len()
        );
    }
    let mut guids = Vec::with_capacity(existing.len() + 1);
    guids.push(fresh.hyphenated().to_string());
    for value in existing {
        let parsed = Uuid::parse_str(value)
            .map_err(|_| anyhow!("MobileVersions.dat holds an invalid GUID"))?;
        guids.push(parsed.hyphenated().to_string());
    }
    let capacity = declared.max(existing.len() + 1).min(1000);
    guids.truncate(capacity);
    let mut out = vec![0xef, 0xbb, 0xbf];
    out.extend_from_slice(format!("{{{},{}}}", capacity, guids.join(",")).as_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::DeflateEncoder};
    use std::io::Write;

    fn deflate(text: &str) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(text.as_bytes()).unwrap();
        encoder.finish().unwrap()
    }

    const G0: &str = "719baa18-69ed-439a-8962-1de53d98e05e";
    const G1: &str = "8c2ac6ff-7309-4025-93f6-264cbb068d62";
    const V: &str = "d607f458-d2c5-43ff-9ccc-fd8078f3b18b";

    #[test]
    fn versions_rows_parse_with_their_entries() {
        let text = format!(
            "\u{feff}{{1,3,\"\",{G0},\"root\",{V},\"0012c33d-4e24-47a2-887e-6164d9a5076f.0\",{G1}}}"
        );
        let row = parse_versions(&deflate(&text)).unwrap();
        assert_eq!(row.generation, Uuid::parse_str(G0).unwrap());
        assert_eq!(row.count, 3);
        assert_eq!(row.entries[0].0, "root");
        assert_eq!(row.entries[1].1, Uuid::parse_str(G1).unwrap());
        // a wrong count is refused
        let bad = format!("{{1,5,\"\",{G0},\"root\",{V}}}");
        assert!(parse_versions(&deflate(&bad)).is_err());
        assert!(parse_versions(b"not deflate").is_err());
    }

    #[test]
    fn dynamic_markers_give_the_history_or_fail_closed() {
        let config = format!("\u{feff}{{1,2,{G0},{G1}}}");
        let params = format!("\u{feff}{{0,3,{V},{G0},{G1}}}");
        let history =
            parse_dynamic_history(Some(config.as_bytes()), Some(params.as_bytes())).unwrap();
        assert_eq!(history.ordinary, Some(Uuid::parse_str(V).unwrap()));
        assert_eq!(history.generations.len(), 2);
        assert_eq!(history.generations[1], Uuid::parse_str(G1).unwrap());
        assert_eq!(
            parse_dynamic_history(None, None).unwrap(),
            DynamicHistory::default()
        );
        assert!(parse_dynamic_history(Some(config.as_bytes()), None).is_err());
        // a native apply of body rows alone clears the Config marker and leaves the
        // Params one: no overlay any more
        let left_over = parse_dynamic_history(None, Some(params.as_bytes())).unwrap();
        assert_eq!(left_over.ordinary, Some(Uuid::parse_str(V).unwrap()));
        assert!(left_over.generations.is_empty());
        assert!(parse_dynamic_history(None, Some(b"{1,3,x}")).is_err());
        let mismatched = format!("{{0,3,{V},{G1},{G0}}}");
        assert!(
            parse_dynamic_history(Some(config.as_bytes()), Some(mismatched.as_bytes())).is_err()
        );
    }

    #[test]
    fn mobile_versions_gain_a_head_and_stay_capped() {
        let a = Uuid::parse_str(G0).unwrap();
        let b = Uuid::parse_str(G1).unwrap();
        let fresh = Uuid::parse_str(V).unwrap();
        let current = format!("\u{feff}{{2,{a},{b}}}");
        let next = mobile_versions_prepend(current.as_bytes(), fresh).unwrap();
        assert_eq!(
            strip_bom(&next),
            format!("{{3,{fresh},{a},{b}}}").as_bytes()
        );
        // a full list of 1000 drops its last entry
        let full = (0..1000)
            .map(|index| Uuid::from_u128(index as u128 + 1).to_string())
            .collect::<Vec<_>>();
        let current = format!("\u{feff}{{1000,{}}}", full.join(","));
        let next = mobile_versions_prepend(current.as_bytes(), fresh).unwrap();
        let text = std::str::from_utf8(strip_bom(&next)).unwrap();
        assert!(text.starts_with(&format!("{{1000,{fresh},{}", full[0])));
        assert!(!text.contains(&full[999]));
        assert_eq!(next.len(), current.len());
    }
}
