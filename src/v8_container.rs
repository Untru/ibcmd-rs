use anyhow::{Context, Result, anyhow};

const V8_MAGIC_NUMBER: u32 = 0x7fff_ffff;
const V8_PAGE_SIZE: u32 = 512;
const FILE_HEADER_SIZE: usize = 16;
const BLOCK_HEADER_SIZE: usize = 31;
const ELEM_ADDR_SIZE: usize = 12;
const ELEM_HEADER_PREFIX_SIZE: usize = 20;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct V8Element {
    pub(crate) name: String,
    pub(crate) header: Vec<u8>,
    pub(crate) data: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
struct BlockHeader {
    data_size: usize,
    page_size: usize,
    next_page_addr: u32,
}

pub(crate) fn parse_v8_container(bytes: &[u8]) -> Result<Vec<V8Element>> {
    if bytes.len() < FILE_HEADER_SIZE + BLOCK_HEADER_SIZE {
        return Err(anyhow!("container is too short"));
    }
    if read_u32(bytes, 0)? != V8_MAGIC_NUMBER {
        return Err(anyhow!("unexpected file header next page marker"));
    }
    if read_u32(bytes, 8)? != 1 {
        return Err(anyhow!("unsupported module container storage version"));
    }

    let toc_header = read_block_header(bytes, FILE_HEADER_SIZE)?;
    let toc_start = FILE_HEADER_SIZE + BLOCK_HEADER_SIZE;
    let toc_end = toc_start
        .checked_add(toc_header.data_size)
        .ok_or_else(|| anyhow!("TOC block size overflows container length"))?;
    if toc_end > bytes.len() {
        return Err(anyhow!("TOC block exceeds container length"));
    }
    if toc_header.data_size % ELEM_ADDR_SIZE != 0 {
        return Err(anyhow!("TOC size is not divisible by element address size"));
    }

    let mut result = Vec::new();
    for (index, entry) in bytes[toc_start..toc_end]
        .chunks_exact(ELEM_ADDR_SIZE)
        .enumerate()
    {
        let header_addr = read_u32(entry, 0)? as usize;
        let data_addr = read_u32(entry, 4)? as usize;
        let marker = read_u32(entry, 8)?;
        if marker != V8_MAGIC_NUMBER {
            return Err(anyhow!(
                "invalid address table marker at entry {}: 0x{:08x}",
                index,
                marker
            ));
        }
        validate_block_address(bytes, header_addr, index, "header")?;
        validate_block_address(bytes, data_addr, index, "data")?;

        let header = read_block_payload(bytes, header_addr)
            .with_context(|| format!("failed to read element {index} header block"))?;
        let data = read_block_payload(bytes, data_addr)
            .with_context(|| format!("failed to read element {index} data block"))?;
        let name = element_name(&header)?;
        result.push(V8Element { name, header, data });
    }
    Ok(result)
}

pub(crate) fn read_v8_element_data(bytes: &[u8], target_name: &str) -> Result<Option<Vec<u8>>> {
    Ok(parse_v8_container(bytes)?
        .into_iter()
        .find(|element| element.name == target_name)
        .map(|element| element.data))
}

pub(crate) fn build_v8_container(elements: &[V8Element]) -> Result<Vec<u8>> {
    let toc_len = elements
        .len()
        .checked_mul(ELEM_ADDR_SIZE)
        .ok_or_else(|| anyhow!("TOC size overflows usize"))?;
    let toc_block_total = BLOCK_HEADER_SIZE
        .checked_add(toc_len)
        .ok_or_else(|| anyhow!("TOC block size overflows usize"))?;
    let mut offset = FILE_HEADER_SIZE
        .checked_add(toc_block_total)
        .ok_or_else(|| anyhow!("container offset overflows usize"))?;

    let mut addresses = Vec::with_capacity(elements.len());
    for element in elements {
        let header_addr = offset;
        offset = offset
            .checked_add(BLOCK_HEADER_SIZE)
            .and_then(|value| value.checked_add(element.header.len()))
            .ok_or_else(|| anyhow!("header block offset overflows usize"))?;

        let data_addr = offset;
        let data_page = page_size_for_data(element.data.len());
        offset = offset
            .checked_add(BLOCK_HEADER_SIZE)
            .and_then(|value| value.checked_add(data_page))
            .ok_or_else(|| anyhow!("data block offset overflows usize"))?;

        addresses.push((header_addr, data_addr));
    }

    let mut bytes = Vec::with_capacity(offset);
    write_u32(&mut bytes, V8_MAGIC_NUMBER);
    write_u32(&mut bytes, V8_PAGE_SIZE);
    write_u32(&mut bytes, 1);
    write_u32(&mut bytes, 0);

    let mut toc = Vec::with_capacity(toc_len);
    for (header_addr, data_addr) in addresses {
        write_u32(&mut toc, checked_u32(header_addr, "header address")?);
        write_u32(&mut toc, checked_u32(data_addr, "data address")?);
        write_u32(&mut toc, V8_MAGIC_NUMBER);
    }
    write_block(&mut bytes, &toc, toc.len())?;

    for element in elements {
        write_block(&mut bytes, &element.header, element.header.len())?;
        write_block(
            &mut bytes,
            &element.data,
            page_size_for_data(element.data.len()),
        )?;
    }

    Ok(bytes)
}

pub(crate) fn make_v8_element_header(name: &str) -> Vec<u8> {
    let mut header = vec![0; ELEM_HEADER_PREFIX_SIZE];
    for unit in name.encode_utf16() {
        header.extend_from_slice(&unit.to_le_bytes());
    }
    header.extend_from_slice(&[0, 0, 0, 0]);
    header
}

fn validate_block_address(
    bytes: &[u8],
    offset: usize,
    element_index: usize,
    kind: &str,
) -> Result<()> {
    if offset >= bytes.len() {
        return Err(anyhow!(
            "element {} {} address {} exceeds container length {}",
            element_index,
            kind,
            offset,
            bytes.len()
        ));
    }
    Ok(())
}

fn read_block_payload(bytes: &[u8], offset: usize) -> Result<Vec<u8>> {
    let header = read_block_header(bytes, offset)?;
    let start = offset
        .checked_add(BLOCK_HEADER_SIZE)
        .ok_or_else(|| anyhow!("block at {} payload offset overflows usize", offset))?;
    let data_end = start
        .checked_add(header.data_size)
        .ok_or_else(|| anyhow!("block at {} data end overflows usize", offset))?;
    let page_end = start
        .checked_add(header.page_size)
        .ok_or_else(|| anyhow!("block at {} page end overflows usize", offset))?;
    if data_end > bytes.len() || page_end > bytes.len() {
        return Err(anyhow!("block at {} exceeds container length", offset));
    }
    if header.next_page_addr != V8_MAGIC_NUMBER {
        return Err(anyhow!(
            "multi-page V8 blocks are not supported yet: block at {} next page address 0x{:08x}",
            offset,
            header.next_page_addr
        ));
    }
    Ok(bytes[start..data_end].to_vec())
}

fn read_block_header(bytes: &[u8], offset: usize) -> Result<BlockHeader> {
    let end = offset
        .checked_add(BLOCK_HEADER_SIZE)
        .ok_or_else(|| anyhow!("block header at {} overflows input length", offset))?;
    if end > bytes.len() {
        return Err(anyhow!("block header at {} exceeds input length", offset));
    }
    let raw = &bytes[offset..end];
    if raw[0] != b'\r'
        || raw[1] != b'\n'
        || raw[10] != b' '
        || raw[19] != b' '
        || raw[28] != b' '
        || raw[29] != b'\r'
        || raw[30] != b'\n'
    {
        return Err(anyhow!("invalid block header at {}", offset));
    }
    let header = BlockHeader {
        data_size: parse_hex_u32(&raw[2..10])? as usize,
        page_size: parse_hex_u32(&raw[11..19])? as usize,
        next_page_addr: parse_hex_u32(&raw[20..28])?,
    };
    if header.page_size < header.data_size {
        return Err(anyhow!(
            "block at {} page size {} is smaller than data size {}",
            offset,
            header.page_size,
            header.data_size
        ));
    }
    Ok(header)
}

fn write_block(target: &mut Vec<u8>, data: &[u8], page_size: usize) -> Result<()> {
    if page_size < data.len() {
        return Err(anyhow!(
            "page size {} is less than data size {}",
            page_size,
            data.len()
        ));
    }
    let header = format!(
        "\r\n{:08x} {:08x} {:08x} \r\n",
        data.len(),
        page_size,
        V8_MAGIC_NUMBER
    );
    if header.len() != BLOCK_HEADER_SIZE {
        return Err(anyhow!("invalid block header length {}", header.len()));
    }
    target.extend_from_slice(header.as_bytes());
    target.extend_from_slice(data);
    target.resize(target.len() + (page_size - data.len()), 0);
    Ok(())
}

fn page_size_for_data(len: usize) -> usize {
    if len < V8_PAGE_SIZE as usize {
        V8_PAGE_SIZE as usize
    } else {
        len
    }
}

fn element_name(header: &[u8]) -> Result<String> {
    if header.len() < ELEM_HEADER_PREFIX_SIZE {
        return Err(anyhow!("element header is too short"));
    }
    let raw = &header[ELEM_HEADER_PREFIX_SIZE..];
    let mut units = Vec::new();
    for pair in raw.chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    String::from_utf16(&units).context("element name is not valid UTF-16LE")
}

fn parse_hex_u32(bytes: &[u8]) -> Result<u32> {
    let text = std::str::from_utf8(bytes)?;
    Ok(u32::from_str_radix(text, 16)?)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| anyhow!("u32 at {} overflows input length", offset))?;
    if end > bytes.len() {
        return Err(anyhow!("u32 at {} exceeds input length", offset));
    }
    Ok(u32::from_le_bytes(bytes[offset..end].try_into()?))
}

fn write_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn checked_u32(value: usize, name: &str) -> Result<u32> {
    value
        .try_into()
        .with_context(|| format!("{name} does not fit into u32: {value}"))
}

#[cfg(test)]
mod tests {
    use super::{
        V8_MAGIC_NUMBER, V8Element, build_v8_container, make_v8_element_header, parse_v8_container,
    };

    #[test]
    fn parses_synthetic_container_with_two_elements() {
        let inner = build_v8_container(&[
            V8Element {
                name: "alpha".to_string(),
                header: make_v8_element_header("alpha"),
                data: b"first".to_vec(),
            },
            V8Element {
                name: "beta".to_string(),
                header: make_v8_element_header("beta"),
                data: b"second".to_vec(),
            },
        ])
        .unwrap();

        let elements = parse_v8_container(&inner).unwrap();

        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].name, "alpha");
        assert_eq!(elements[0].header, make_v8_element_header("alpha"));
        assert_eq!(elements[0].data, b"first");
        assert_eq!(elements[1].name, "beta");
        assert_eq!(elements[1].data, b"second");
    }

    #[test]
    fn builds_container_and_parses_byte_identical_data() {
        let source = vec![
            V8Element {
                name: "one".to_string(),
                header: make_v8_element_header("one"),
                data: vec![0, 1, 2, 3, 255],
            },
            V8Element {
                name: "two".to_string(),
                header: make_v8_element_header("two"),
                data: (0..=255).collect(),
            },
        ];

        let inner = build_v8_container(&source).unwrap();
        let parsed = parse_v8_container(&inner).unwrap();

        assert_eq!(parsed.len(), source.len());
        assert_eq!(parsed[0].data, source[0].data);
        assert_eq!(parsed[1].data, source[1].data);
    }

    #[test]
    fn rejects_multi_page_block_with_precise_error() {
        let mut inner = build_v8_container(&[V8Element {
            name: "text".to_string(),
            header: make_v8_element_header("text"),
            data: b"payload".to_vec(),
        }])
        .unwrap();
        let data_addr = u32::from_le_bytes(inner[51..55].try_into().unwrap()) as usize;
        let next_page = data_addr + 128;
        inner[data_addr + 20..data_addr + 28]
            .copy_from_slice(format!("{:08x}", next_page).as_bytes());

        let error = parse_v8_container(&inner).unwrap_err();
        let error_chain = format!("{error:#}");

        assert!(
            error_chain.contains(&format!(
                "multi-page V8 blocks are not supported yet: block at {} next page address 0x{:08x}",
                data_addr, next_page
            )),
            "{error_chain}"
        );
    }

    #[test]
    fn rejects_invalid_address_table_marker() {
        let mut inner = build_v8_container(&[V8Element {
            name: "text".to_string(),
            header: make_v8_element_header("text"),
            data: b"payload".to_vec(),
        }])
        .unwrap();
        inner[55..59].copy_from_slice(&0_u32.to_le_bytes());

        let error = parse_v8_container(&inner).unwrap_err().to_string();

        assert_eq!(error, "invalid address table marker at entry 0: 0x00000000");
    }

    #[test]
    fn writes_expected_file_header() {
        let inner = build_v8_container(&[]).unwrap();

        assert_eq!(&inner[0..4], &V8_MAGIC_NUMBER.to_le_bytes());
        assert_eq!(&inner[8..12], &1_u32.to_le_bytes());
    }
}
