//! Blob objects: 256-byte blocks of `u32` next block, `u16` used length and
//! 250 data bytes. Block 0 is the header; its first `u32` heads the chain of
//! free blocks, which link through their own first `u32`. A value is the chain
//! from its first block to a block whose next is 0.

use crate::error::{Result, corrupt};

pub(crate) const BLOCK: usize = 256;
pub(crate) const DATA: usize = 250;

/// Reads the chain starting at `start` from a whole blob object. With
/// `expected`, the chain must hold exactly that many bytes.
#[cfg(test)]
pub(crate) fn read_chain(object: &[u8], start: u32, expected: Option<u64>) -> Result<Vec<u8>> {
    read_chain_checked(object, start, expected, &|| Ok(()))
}

pub(crate) fn read_chain_checked(
    object: &[u8],
    start: u32,
    expected: Option<u64>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    read_chain_accounted(object, start, expected, &mut |_| check())
}

/// Charge/check every copied block before growing the destination buffer.
pub(crate) fn read_chain_accounted(
    object: &[u8],
    start: u32,
    expected: Option<u64>,
    before_copy: &mut dyn FnMut(u64) -> Result<()>,
) -> Result<Vec<u8>> {
    before_copy(0)?;
    let blocks = object.len() / BLOCK;
    let mut out = Vec::with_capacity(expected.unwrap_or(0).min(object.len() as u64) as usize);
    let mut block = start as usize;
    let mut steps = 0usize;
    while block != 0 {
        if block >= blocks {
            return Err(corrupt(format!(
                "Blob: блок {block} вне объекта из {blocks} блоков"
            )));
        }
        steps += 1;
        if steps > blocks {
            return Err(corrupt("Blob: цепочка блоков зациклена"));
        }
        let at = block * BLOCK;
        let next = u32::from_le_bytes(object[at..at + 4].try_into().unwrap()) as usize;
        let used = u16::from_le_bytes([object[at + 4], object[at + 5]]) as usize;
        if used > DATA {
            return Err(corrupt(format!("Blob: блок {block} заявляет {used} байт")));
        }
        if let Some(limit) = expected
            && (out.len() as u64).saturating_add(used as u64) > limit
        {
            return Err(corrupt("Blob: цепочка длиннее значения"));
        }
        before_copy(used as u64)?;
        out.extend_from_slice(&object[at + 6..at + 6 + used]);
        block = next;
    }
    if let Some(limit) = expected
        && out.len() as u64 != limit
    {
        return Err(corrupt(format!(
            "Blob: цепочка с блока {start} даёт {} байт вместо {limit}",
            out.len()
        )));
    }
    Ok(out)
}

/// Builds a blob object: values written as consecutive chains from block 1,
/// then the remaining blocks (up to `min_blocks`, the old object size) chained
/// as free. Returns the content and, per value, `(first block, length)`.
/// `(first block, length)` of a value in a blob object; `(0, 0)` for none.
pub(crate) type BlobRef = (u32, u32);

pub(crate) fn build(values: &[&[u8]], min_blocks: usize) -> Result<(Vec<u8>, Vec<BlobRef>)> {
    let used: usize = values.iter().map(|v| v.len().div_ceil(DATA)).sum();
    let total = (used + 1).max(min_blocks).max(1);
    if total > u32::MAX as usize {
        return Err(crate::error::Error::Refused(
            "Blob: значения не помещаются в объект".into(),
        ));
    }
    let mut out = vec![0u8; total * BLOCK];
    let mut refs = Vec::with_capacity(values.len());
    let mut block = 1usize;
    for value in values {
        let len = u32::try_from(value.len())
            .map_err(|_| crate::error::Error::Refused("Blob: значение длиннее 4 ГБ".into()))?;
        if value.is_empty() {
            refs.push((0, 0));
            continue;
        }
        refs.push((block as u32, len));
        let chunks: Vec<&[u8]> = value.chunks(DATA).collect();
        for (i, chunk) in chunks.iter().enumerate() {
            let at = block * BLOCK;
            let next = if i + 1 < chunks.len() { block + 1 } else { 0 };
            out[at..at + 4].copy_from_slice(&(next as u32).to_le_bytes());
            out[at + 4..at + 6].copy_from_slice(&(chunk.len() as u16).to_le_bytes());
            out[at + 6..at + 6 + chunk.len()].copy_from_slice(chunk);
            block += 1;
        }
    }
    // Free chain over the unused tail, in ascending order.
    let first_free = if block < total { block as u32 } else { 0 };
    out[0..4].copy_from_slice(&first_free.to_le_bytes());
    for b in block..total {
        let next = if b + 1 < total { (b + 1) as u32 } else { 0 };
        out[b * BLOCK..b * BLOCK + 4].copy_from_slice(&next.to_le_bytes());
    }
    Ok((out, refs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip_and_free_tail_is_chained() {
        let a = vec![7u8; 600];
        let b: Vec<u8> = (0..250u32).map(|i| i as u8).collect();
        let (object, refs) = build(&[&a, &[], &b], 10).unwrap();
        assert_eq!(object.len(), 10 * BLOCK);
        assert_eq!(refs, vec![(1, 600), (0, 0), (4, 250)]);
        assert_eq!(read_chain(&object, 1, Some(600)).unwrap(), a);
        assert_eq!(read_chain(&object, 4, Some(250)).unwrap(), b);
        // free head 5 → 6 → … → 9 → 0
        assert_eq!(u32::from_le_bytes(object[0..4].try_into().unwrap()), 5);
        assert_eq!(
            u32::from_le_bytes(object[9 * BLOCK..9 * BLOCK + 4].try_into().unwrap()),
            0
        );
    }

    #[test]
    fn broken_chains_fail_closed() {
        let mut object = vec![0u8; 3 * BLOCK];
        object[BLOCK..BLOCK + 4].copy_from_slice(&1u32.to_le_bytes()); // loops to itself
        object[BLOCK + 4..BLOCK + 6].copy_from_slice(&1u16.to_le_bytes());
        assert!(read_chain(&object, 1, None).is_err());
        assert!(read_chain(&object, 9, None).is_err());
        object[BLOCK..BLOCK + 4].copy_from_slice(&0u32.to_le_bytes());
        object[BLOCK + 4..BLOCK + 6].copy_from_slice(&251u16.to_le_bytes());
        assert!(read_chain(&object, 1, None).is_err());
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    #[test]
    fn cancellation_interrupts_a_chain_between_blocks() {
        let (object, refs) = build(&[&[7u8; 1000]], 0).unwrap();
        let calls = std::cell::Cell::new(0);
        let error = read_chain_checked(&object, refs[0].0, Some(1000), &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                Err(crate::Error::Refused("cancelled".into()))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert_eq!(calls.get(), 3);
    }
}
