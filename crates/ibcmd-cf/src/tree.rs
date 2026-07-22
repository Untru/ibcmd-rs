//! Bounded visitor-based traversal of nested V8 containers.

use std::{
    error::Error,
    fmt,
    io::{Cursor, Read, Seek},
};

use ibcmd_core::limits::{ResourceBudget, ResourceLimits};
use ibcmd_v8::reader::{EntryIndex, ReaderError, StreamingReader};

use crate::payload::{PayloadDecodeError, PayloadDecoder, PayloadEncoding};

pub trait ReadSeek: Read + Seek {}

impl<T: Read + Seek> ReadSeek for T {}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TraversalAction {
    Skip,
    Leaf(PayloadEncoding),
    Container(PayloadEncoding),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum VisitKind {
    Leaf,
    Container,
}

pub struct Visit<'a> {
    pub path: &'a [String],
    pub entry: &'a EntryIndex,
    pub kind: VisitKind,
    pub encoding: PayloadEncoding,
    pub bytes: &'a [u8],
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TraversalStats {
    pub containers: usize,
    pub visited_entries: usize,
    pub maximum_depth: usize,
    pub budget: ResourceBudget,
}

pub fn traverse<R, C, V>(
    source: R,
    limits: ResourceLimits,
    mut classify: C,
    mut visit: V,
) -> Result<TraversalStats, TreeError>
where
    R: Read + Seek + 'static,
    C: FnMut(&[String], &EntryIndex) -> TraversalAction,
    V: for<'a> FnMut(Visit<'a>),
{
    let boxed: Box<dyn ReadSeek> = Box::new(source);
    let mut reader = StreamingReader::open(boxed, limits).map_err(|source| TreeError::Reader {
        path: Vec::new(),
        source,
    })?;
    let mut decoder = PayloadDecoder::new(limits);
    decoder
        .enter_container()
        .map_err(|source| TreeError::Payload {
            path: Vec::new(),
            source,
        })?;
    let mut stats = TraversalStats {
        containers: 1,
        visited_entries: 0,
        maximum_depth: decoder.budget().depth(),
        budget: decoder.budget().clone(),
    };
    let result = walk_container(
        &mut reader,
        limits,
        &mut decoder,
        &mut Vec::new(),
        &mut stats,
        &mut classify,
        &mut visit,
    );
    let leave = decoder
        .leave_container()
        .map_err(|source| TreeError::Payload {
            path: Vec::new(),
            source,
        });
    result?;
    leave?;
    stats.budget = decoder.budget().clone();
    Ok(stats)
}

fn walk_container<C, V>(
    reader: &mut StreamingReader<Box<dyn ReadSeek>>,
    limits: ResourceLimits,
    decoder: &mut PayloadDecoder,
    path: &mut Vec<String>,
    stats: &mut TraversalStats,
    classify: &mut C,
    visit: &mut V,
) -> Result<(), TreeError>
where
    C: FnMut(&[String], &EntryIndex) -> TraversalAction,
    V: for<'a> FnMut(Visit<'a>),
{
    for index in 0..reader.index().entries.len() {
        let entry = reader.index().entries[index].clone();
        path.push(entry.name.clone());
        let action = classify(path, &entry);
        if action == TraversalAction::Skip {
            path.pop();
            continue;
        }

        let encoded = reader
            .read_entry_data(index)
            .map_err(|source| TreeError::Reader {
                path: path.clone(),
                source,
            })?
            .ok_or_else(|| TreeError::MissingData { path: path.clone() })?;
        let (encoding, kind) = match action {
            TraversalAction::Skip => unreachable!(),
            TraversalAction::Leaf(encoding) => (encoding, VisitKind::Leaf),
            TraversalAction::Container(encoding) => (encoding, VisitKind::Container),
        };
        let decoded = decoder
            .decode(encoding, &encoded)
            .map_err(|source| TreeError::Payload {
                path: path.clone(),
                source,
            })?;
        stats.visited_entries += 1;
        visit(Visit {
            path,
            entry: &entry,
            kind,
            encoding,
            bytes: decoded.bytes(),
        });

        if kind == VisitKind::Container {
            decoder
                .enter_container()
                .map_err(|source| TreeError::Payload {
                    path: path.clone(),
                    source,
                })?;
            stats.maximum_depth = stats.maximum_depth.max(decoder.budget().depth());
            let child_path = path.clone();
            let child_source: Box<dyn ReadSeek> = Box::new(Cursor::new(decoded.into_bytes()));
            let child_result = StreamingReader::open(child_source, limits)
                .map_err(|source| TreeError::Reader {
                    path: child_path,
                    source,
                })
                .and_then(|mut child| {
                    stats.containers += 1;
                    walk_container(&mut child, limits, decoder, path, stats, classify, visit)
                });
            let leave_result = decoder
                .leave_container()
                .map_err(|source| TreeError::Payload {
                    path: path.clone(),
                    source,
                });
            child_result?;
            leave_result?;
        }
        path.pop();
    }
    stats.budget = decoder.budget().clone();
    Ok(())
}

#[derive(Debug)]
pub enum TreeError {
    Reader {
        path: Vec<String>,
        source: ReaderError,
    },
    Payload {
        path: Vec<String>,
        source: PayloadDecodeError,
    },
    MissingData {
        path: Vec<String>,
    },
}

impl fmt::Display for TreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reader { path, source } => {
                write!(
                    formatter,
                    "nested reader failed at {}: {source}",
                    display_path(path)
                )
            }
            Self::Payload { path, source } => write!(
                formatter,
                "nested payload failed at {}: {source}",
                display_path(path)
            ),
            Self::MissingData { path } => write!(
                formatter,
                "nested traversal expected data at {} but found an absent sentinel",
                display_path(path)
            ),
        }
    }
}

impl Error for TreeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Reader { source, .. } => Some(source),
            Self::Payload { source, .. } => Some(source),
            Self::MissingData { .. } => None,
        }
    }
}

fn display_path(path: &[String]) -> String {
    if path.is_empty() {
        return "/".to_string();
    }
    format!("/{}", path.join("/"))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use ibcmd_core::limits::{ResourceLimitError, ResourceLimits};
    use ibcmd_v8::format15;

    use crate::payload::{PayloadDecodeError, PayloadEncoding, encode_payload};

    use super::{TraversalAction, TreeError, VisitKind, traverse};

    fn limits(depth: usize) -> ResourceLimits {
        ResourceLimits::new(depth, 64, 1_048_576, 1_048_576, 200).unwrap()
    }

    #[test]
    fn visits_stored_nested_container_without_retaining_a_tree() {
        let child = container("leaf", b"done");
        let root = container("nested", &child);
        let mut visits = Vec::new();

        let stats = traverse(
            Cursor::new(root),
            limits(4),
            |_, entry| {
                if entry.name == "nested" {
                    TraversalAction::Container(PayloadEncoding::Stored)
                } else {
                    TraversalAction::Leaf(PayloadEncoding::Stored)
                }
            },
            |visit| {
                visits.push((visit.path.join("/"), visit.kind, visit.bytes.to_vec()));
            },
        )
        .unwrap();

        assert_eq!(stats.containers, 2);
        assert_eq!(stats.visited_entries, 2);
        assert_eq!(stats.maximum_depth, 2);
        assert_eq!(stats.budget.depth(), 0);
        assert_eq!(stats.budget.entries(), 2);
        assert_eq!(visits[0].0, "nested");
        assert_eq!(visits[0].1, VisitKind::Container);
        assert_eq!(
            visits[1],
            ("nested/leaf".to_string(), VisitKind::Leaf, b"done".to_vec())
        );
    }

    #[test]
    fn recursive_container_bomb_returns_exact_shared_depth_limit() {
        let leaf = container("leaf", b"done");
        let level_two = container("nested", &leaf);
        let root = container("nested", &level_two);

        let error = traverse(
            Cursor::new(root),
            limits(2),
            |_, entry| {
                if entry.name == "nested" {
                    TraversalAction::Container(PayloadEncoding::Stored)
                } else {
                    TraversalAction::Leaf(PayloadEncoding::Stored)
                }
            },
            |_| {},
        )
        .unwrap_err();

        assert!(matches!(
            error,
            TreeError::Payload {
                path,
                source: PayloadDecodeError::Limit(ResourceLimitError::DepthExceeded {
                    maximum: 2,
                    actual: 3,
                }),
            } if path == ["nested", "nested"]
        ));
    }

    #[test]
    fn raw_deflate_leaf_uses_the_same_aggregate_decoder() {
        let encoded = encode_payload(PayloadEncoding::RawDeflate, b"decoded", limits(2)).unwrap();
        let root = container("leaf", &encoded);
        let mut payload = Vec::new();

        let stats = traverse(
            Cursor::new(root),
            limits(2),
            |_, _| TraversalAction::Leaf(PayloadEncoding::RawDeflate),
            |visit| payload.extend_from_slice(visit.bytes),
        )
        .unwrap();

        assert_eq!(payload, b"decoded");
        assert_eq!(stats.budget.entries(), 1);
        assert_eq!(stats.budget.encoded_bytes(), encoded.len() as u64);
        assert_eq!(stats.budget.decoded_bytes(), 7);
    }

    fn container(name: &str, data: &[u8]) -> Vec<u8> {
        let header = element_header(name);
        let toc_size = 12_usize;
        let header_address = 16 + 31 + toc_size;
        let data_address = header_address + 31 + header.len();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&format15::SENTINEL.to_le_bytes());
        bytes.extend_from_slice(&512_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&block_header(toc_size));
        bytes.extend_from_slice(&(header_address as u32).to_le_bytes());
        bytes.extend_from_slice(&(data_address as u32).to_le_bytes());
        bytes.extend_from_slice(&format15::SENTINEL.to_le_bytes());
        bytes.extend_from_slice(&block_header(header.len()));
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(&block_header(data.len()));
        bytes.extend_from_slice(data);
        bytes
    }

    fn block_header(size: usize) -> Vec<u8> {
        format!(
            "\r\n{size:08x} {size:08x} {sentinel:08x} \r\n",
            sentinel = format15::SENTINEL
        )
        .into_bytes()
    }

    fn element_header(name: &str) -> Vec<u8> {
        let mut bytes = vec![0; 20];
        bytes.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
        bytes.extend_from_slice(&[0; 4]);
        bytes
    }
}
