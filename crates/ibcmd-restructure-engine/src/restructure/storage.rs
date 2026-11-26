//! `SchemaStorage`: the state machine of a restructure, and `NewGenCreated`.
//!
//! `SchemaStorage(SchemaID int, Status int, CurrentSchema, NewGenCreated, NewGenDropped varbinary(max))`;
//! `SchemaID 0` is the main configuration, `1` the extensions (their tables carry the `X1` suffix). The
//! platform rebuilds a changed table through a "new generation" (`NG`) copy and records how far it got:
//!
//! | `Status` | meaning (measured on `config apply`) |
//! |---|---|
//! | 100 | idle: `CurrentSchema` is the schema of the tables that exist |
//! | 200 | the `NG` tables listed in `NewGenCreated` exist (and are being filled) |
//! | 400 | the `NG` tables are filled; the old tables are being dropped |
//! | 500 | the old tables are dropped; the `NG` tables are being renamed |
//!
//! When it is done the platform writes `Status 100`, the new `CurrentSchema` (the same text as
//! `DBSchema.SerializedData`) and the empty marker `{0,{0}}` into `NewGenCreated` and `NewGenDropped`.
//! `NewGenCreated` is a mini `DBSchema` (`{0,{<n>,<table entry>...}}`) with the new definitions of the
//! `NG` tables created so far. `NewGenDropped` was never anything but the marker in the traced cases.

use anyhow::Result;

use crate::metadata_model::brace::Brace;
use crate::restructure::schema::DbSchema;

pub const STATUS_IDLE: i64 = 100;
pub const STATUS_CREATED: i64 = 200;
pub const STATUS_LOADED: i64 = 400;
pub const STATUS_DROPPED: i64 = 500;

/// `NewGenCreated` / `NewGenDropped` when nothing is in flight: BOM + `{0,` CRLF `{0}` CRLF `}`.
pub const EMPTY_GENERATION: &[u8] = b"\xEF\xBB\xBF{0,\r\n{0}\r\n}";

/// A `SchemaStorage` row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaStorageRow {
    pub schema_id: i64,
    pub status: i64,
    pub current_schema: Vec<u8>,
    pub new_gen_created: Vec<u8>,
    pub new_gen_dropped: Vec<u8>,
}

impl SchemaStorageRow {
    /// Idle and with nothing left over from an earlier restructure: the only state a
    /// restructure may start from.
    pub fn is_idle(&self) -> bool {
        self.status == STATUS_IDLE
            && self.new_gen_created == EMPTY_GENERATION
            && self.new_gen_dropped == EMPTY_GENERATION
    }
}

/// `NewGenCreated` after the tables in `created` (their new definitions, in creation order) exist.
pub fn new_gen_created(created: &[Brace]) -> Vec<u8> {
    DbSchema::from_tables(created.to_vec()).to_text()
}

/// The table entries of a `NewGenCreated` text.
pub fn parse_new_gen_created(text: &[u8]) -> Result<DbSchema> {
    DbSchema::parse(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_empty_marker_is_the_text_of_an_empty_schema() {
        let empty = DbSchema::parse(EMPTY_GENERATION).unwrap();
        assert!(empty.is_empty());
        assert_eq!(empty.to_text(), EMPTY_GENERATION);
        assert_eq!(EMPTY_GENERATION.len(), 14);
    }

    #[test]
    fn only_an_idle_row_with_empty_generations_may_start_a_restructure() {
        let row = SchemaStorageRow {
            schema_id: 0,
            status: STATUS_IDLE,
            current_schema: b"x".to_vec(),
            new_gen_created: EMPTY_GENERATION.to_vec(),
            new_gen_dropped: EMPTY_GENERATION.to_vec(),
        };
        assert!(row.is_idle());
        assert!(
            !SchemaStorageRow {
                status: STATUS_CREATED,
                ..row.clone()
            }
            .is_idle()
        );
        assert!(
            !SchemaStorageRow {
                new_gen_created: b"{0,{1}}".to_vec(),
                ..row
            }
            .is_idle()
        );
    }
}
