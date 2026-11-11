//! DefinedType payload coordinates shared by descriptor decoding and indexing.
//! Native 8.3.27.2214 and 8.5.1.1150 corpus-defined-type-resource records use
//! `{0,<type id>,<value id>,<metadata header>,<type pattern>}`.

pub struct DefinedTypeLayout;

impl DefinedTypeLayout {
    pub const REVISION: &'static str = "0";
    pub const TYPE_ID: usize = 1;
    pub const VALUE_ID: usize = 2;
    pub const HEADER: usize = 3;
    pub const PATTERN: usize = 4;
}
