//! Nonloaded .debug_info is metadata. Keep .debug_str and every other byte.
pub fn exclude_debug_info(bytes: &mut [u8]) -> Result<(), &'static str> {
    if !bytes.starts_with(b"\x7fELF\x02\x01") {
        return Ok(());
    }
    fn read<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], &'static str> {
        bytes
            .get(offset..offset.checked_add(N).ok_or("ELF offset overflow")?)
            .ok_or("truncated ELF header")?
            .try_into()
            .map_err(|_| "ELF field width")
    }
    fn word(bytes: &[u8], offset: usize) -> Result<usize, &'static str> {
        usize::try_from(u64::from_le_bytes(read(bytes, offset)?)).map_err(|_| "ELF offset overflow")
    }
    fn range(bytes: &[u8], offset: usize, size: usize) -> Result<(usize, usize), &'static str> {
        let end = offset.checked_add(size).ok_or("ELF range overflow")?;
        if end > bytes.len() {
            return Err("ELF range outside file");
        }
        Ok((offset, end))
    }
    fn overlaps(a: (usize, usize), b: (usize, usize)) -> bool {
        a.0 < b.1 && b.0 < a.1
    }
    let shoff = word(bytes, 40)?;
    let stride = usize::from(u16::from_le_bytes(read(bytes, 58)?));
    let mut count = usize::from(u16::from_le_bytes(read(bytes, 60)?));
    let mut names_index = usize::from(u16::from_le_bytes(read(bytes, 62)?));
    if shoff == 0 && count == 0 {
        return Ok(());
    }
    if stride < 64 {
        return Err("ELF section header width");
    }
    range(bytes, shoff, stride)?;
    if count == 0 {
        count = word(bytes, shoff + 32)?;
    }
    if names_index == 0xffff {
        names_index = u32::from_le_bytes(read(bytes, shoff + 40)?) as usize;
    }
    let section_table = range(
        bytes,
        shoff,
        count.checked_mul(stride).ok_or("ELF table overflow")?,
    )?;
    if names_index >= count {
        return Err("ELF section name table index");
    }
    let names_header = shoff + names_index * stride;
    if u32::from_le_bytes(read(bytes, names_header + 4)?) != 3 {
        return Err("ELF section name table type");
    }
    let names_range = range(
        bytes,
        word(bytes, names_header + 24)?,
        word(bytes, names_header + 32)?,
    )?;
    let names = &bytes[names_range.0..names_range.1];
    let mut allocated = Vec::new();
    let mut debug = Vec::new();
    let mut protected = vec![(0, 64), section_table, names_range];
    for index in 0..count {
        let header = shoff + index * stride;
        let name_offset = u32::from_le_bytes(read(bytes, header)?) as usize;
        let name = names.get(name_offset..).ok_or("ELF section name offset")?;
        let end = name
            .iter()
            .position(|&b| b == 0)
            .ok_or("unterminated ELF section name")?;
        let kind = u32::from_le_bytes(read(bytes, header + 4)?);
        let flags = u64::from_le_bytes(read(bytes, header + 8)?);
        if kind == 8 {
            continue;
        } // NOBITS has no on-disk payload.
        let span = range(bytes, word(bytes, header + 24)?, word(bytes, header + 32)?)?;
        if flags & 2 != 0 {
            allocated.push(span);
        }
        if &name[..end] == b".debug_info" && kind == 1 && flags == 0 {
            debug.push(span);
        } else {
            protected.push(span);
        }
    }
    let phoff = word(bytes, 32)?;
    let phstride = usize::from(u16::from_le_bytes(read(bytes, 54)?));
    let mut phcount = usize::from(u16::from_le_bytes(read(bytes, 56)?));
    if phcount == 0xffff {
        phcount = u32::from_le_bytes(read(bytes, shoff + 44)?) as usize;
    }
    if phcount != 0 {
        if phstride < 56 {
            return Err("ELF program header width");
        }
        protected.push(range(
            bytes,
            phoff,
            phcount
                .checked_mul(phstride)
                .ok_or("ELF program table overflow")?,
        )?);
        for index in 0..phcount {
            let header = phoff + index * phstride;
            let kind = u32::from_le_bytes(read(bytes, header)?);
            if kind != 0 {
                let segment = range(bytes, word(bytes, header + 8)?, word(bytes, header + 32)?)?;
                protected.push(segment);
                if kind == 1 {
                    allocated.push(segment);
                }
            }
        }
    }
    if debug
        .iter()
        .any(|&d| allocated.iter().any(|&a| overlaps(d, a)))
    {
        return Err("debug_info overlaps loadable ELF bytes");
    }
    if debug
        .iter()
        .any(|&d| protected.iter().any(|&a| overlaps(d, a)))
    {
        return Err("debug_info overlaps ELF metadata or another section");
    }
    for (start, end) in debug {
        bytes[start..end].fill(0);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::exclude_debug_info;
    use std::sync::LazyLock;
    static IMAGE: LazyLock<Vec<u8>> = LazyLock::new(|| {
        include_str!("../fixtures/elf-debug-numeric.hex")
            .split_ascii_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).expect("authored ELF fixture hex byte"))
            .collect()
    });
    fn has_jar(bytes: &[u8]) -> bool {
        bytes.windows(4).any(|b| b == b".jar")
    }

    #[test]
    fn numeric_debug_reference_is_distinct_from_loaded_bytes_and_strings() {
        assert!(has_jar(&IMAGE));
        let mut numeric = IMAGE.to_vec();
        exclude_debug_info(&mut numeric).unwrap();
        assert!(!has_jar(&numeric));
        for offset in [384, 416] {
            let mut bytes = IMAGE.to_vec();
            bytes[offset..offset + 4].copy_from_slice(b".jar");
            exclude_debug_info(&mut bytes).unwrap();
            assert!(has_jar(&bytes));
        }
    }

    #[test]
    fn allocated_debug_info_is_scanned_and_load_segment_overlap_refuses() {
        let mut allocated = IMAGE.to_vec();
        allocated[200..208].copy_from_slice(&2u64.to_le_bytes());
        exclude_debug_info(&mut allocated).unwrap();
        assert!(has_jar(&allocated));
        let mut overlap = IMAGE.to_vec();
        overlap[592..600].copy_from_slice(&408u64.to_le_bytes());
        let before = overlap.clone();
        assert_eq!(
            exclude_debug_info(&mut overlap),
            Err("debug_info overlaps loadable ELF bytes")
        );
        assert_eq!(overlap, before);
    }

    #[test]
    fn malformed_ranges_and_names_do_not_mask_payloads() {
        for (offset, replacement) in [
            (216, (1u64 << 63).to_le_bytes()),
            (320, u64::MAX.to_le_bytes()),
        ] {
            let mut bytes = IMAGE.to_vec();
            bytes[offset..offset + 8].copy_from_slice(&replacement);
            let before = bytes.clone();
            assert!(exclude_debug_info(&mut bytes).is_err());
            assert_eq!(bytes, before);
        }
    }

    #[test]
    fn extended_header_counts_use_the_actual_section_zero_fields() {
        let mut bytes = IMAGE.to_vec();
        bytes[60..62].copy_from_slice(&0u16.to_le_bytes());
        bytes[62..64].copy_from_slice(&0xffffu16.to_le_bytes());
        bytes[96..104].copy_from_slice(&5u64.to_le_bytes());
        bytes[104..108].copy_from_slice(&4u32.to_le_bytes());
        bytes[56..58].copy_from_slice(&0xffffu16.to_le_bytes());
        bytes[108..112].copy_from_slice(&1u32.to_le_bytes());
        exclude_debug_info(&mut bytes).unwrap();
        assert!(!has_jar(&bytes));
    }

    #[test]
    fn debug_cannot_hide_headers_string_table_or_other_section_payloads() {
        for offset in [64u64, 416, 432, 560] {
            let mut bytes = IMAGE.to_vec();
            bytes[216..224].copy_from_slice(&offset.to_le_bytes());
            let before = bytes.clone();
            assert!(exclude_debug_info(&mut bytes).is_err());
            assert_eq!(bytes, before);
        }
    }

    #[test]
    fn nonnull_program_payloads_cannot_alias_debug_or_escape_file_bounds() {
        // INTERP, DYNAMIC, NOTE, TLS and an unknown non-null segment.
        for kind in [3u32, 2, 4, 7, 0x6fff_ffff] {
            for offset in [400u64, u64::MAX] {
                let mut bytes = IMAGE.to_vec();
                bytes[560..564].copy_from_slice(&kind.to_le_bytes());
                bytes[568..576].copy_from_slice(&offset.to_le_bytes());
                bytes[592..600].copy_from_slice(&8u64.to_le_bytes());
                let before = bytes.clone();
                assert!(exclude_debug_info(&mut bytes).is_err());
                assert_eq!(bytes, before);
            }
        }
    }
}
