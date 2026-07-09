#[derive(Debug, Clone)]
pub(super) struct ConfigRow {
    pub(super) file_name: String,
    pub(super) part_no: i32,
    pub(super) data_size: i64,
    pub(super) binary_hex: String,
}

#[derive(Debug, Clone)]
pub(super) struct ConfigRowHeader {
    pub(super) file_name: String,
    pub(super) part_no: i32,
    pub(super) data_size: i64,
}

#[derive(Debug)]
pub(super) struct ConfigChunkRow {
    pub(super) file_name: String,
    pub(super) part_no: i32,
    pub(super) data_size: i64,
    pub(super) chunk_index: i32,
    pub(super) binary_hex: String,
}

#[derive(Debug)]
pub(super) struct BinaryConfigRow {
    pub(super) file_name: String,
    pub(super) part_no: i32,
    pub(super) data_size: i64,
    pub(super) binary: Vec<u8>,
}
