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

pub(crate) fn parse_v8_container(bytes: &[u8]) -> Result<Vec<V8Element>> {
    let container = ibcmd_v8::format15::parse(bytes).map_err(anyhow::Error::new)?;
    container
        .elements
        .into_iter()
        .map(|element| {
            let data = element
                .data
                .ok_or_else(|| anyhow!("element {} has an absent data address", element.name))?;
            Ok(V8Element {
                name: element.name,
                header: element.raw_header,
                data,
            })
        })
        .collect()
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
        BLOCK_HEADER_SIZE, V8_MAGIC_NUMBER, V8Element, build_v8_container, make_v8_element_header,
        parse_v8_container,
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
    fn parses_storage_version_two_container() {
        let mut inner = build_v8_container(&[V8Element {
            name: "image".to_string(),
            header: make_v8_element_header("image"),
            data: b"compiled-image".to_vec(),
        }])
        .unwrap();
        inner[8..12].copy_from_slice(&2_u32.to_le_bytes());

        let elements = parse_v8_container(&inner).unwrap();

        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].name, "image");
        assert_eq!(elements[0].data, b"compiled-image");
    }

    #[test]
    fn parses_multi_page_block_through_portable_reader() {
        let mut inner = build_v8_container(&[V8Element {
            name: "text".to_string(),
            header: make_v8_element_header("text"),
            data: b"payload".to_vec(),
        }])
        .unwrap();
        let data_addr = u32::from_le_bytes(inner[51..55].try_into().unwrap()) as usize;
        let original =
            inner[data_addr + BLOCK_HEADER_SIZE..data_addr + BLOCK_HEADER_SIZE + 7].to_vec();
        let split = 3_u32;
        let next_page = inner.len();
        inner[data_addr + 11..data_addr + 19].copy_from_slice(format!("{split:08x}").as_bytes());
        inner[data_addr + 20..data_addr + 28]
            .copy_from_slice(format!("{:08x}", next_page).as_bytes());
        inner.extend_from_slice(
            format!("\r\n00000000 00000004 {V8_MAGIC_NUMBER:08x} \r\n").as_bytes(),
        );
        inner.extend_from_slice(&original[split as usize..]);

        let elements = parse_v8_container(&inner).unwrap();

        assert_eq!(elements[0].name, "text");
        assert_eq!(elements[0].data, original);
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
