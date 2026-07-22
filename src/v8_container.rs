use anyhow::{Result, anyhow};

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
    let document = ibcmd_v8::writer::Format15Document::new(
        1,
        elements
            .iter()
            .map(|element| {
                ibcmd_v8::writer::Format15Element::preserved(
                    element.header.clone(),
                    Some(element.data.clone()),
                )
            })
            .collect(),
    );
    ibcmd_v8::writer::write_format15_to_vec(&document).map_err(anyhow::Error::new)
}

pub(crate) fn make_v8_element_header(name: &str) -> Vec<u8> {
    ibcmd_v8::writer::make_element_header(name)
}

#[cfg(test)]
mod tests {
    use super::{V8Element, build_v8_container, make_v8_element_header, parse_v8_container};

    const V8_MAGIC_NUMBER: u32 = ibcmd_v8::format15::SENTINEL;
    const BLOCK_HEADER_SIZE: usize = ibcmd_v8::format15::BLOCK_HEADER_SIZE;

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
