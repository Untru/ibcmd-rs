#![no_main]

use std::io::Cursor;

use ibcmd_cf::{
    payload::{PayloadEncoding, decode_payload},
    tree::{TraversalAction, traverse},
};
use ibcmd_core::limits::ResourceLimits;
use ibcmd_v8::reader::StreamingReader;
use libfuzzer_sys::fuzz_target;

const MAX_FUZZ_INPUT: usize = 1_048_576;

fn limits() -> ResourceLimits {
    ResourceLimits::new(8, 64, 1_048_576, 1_048_576, 200).expect("static fuzz limits are valid")
}

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_FUZZ_INPUT {
        return;
    }

    let budget = limits();
    if let Ok(mut reader) = StreamingReader::open(Cursor::new(data), budget) {
        for index in 0..reader.index().entries.len() {
            let _ = reader.read_entry_header(index);
            let _ = reader.read_entry_data(index);
        }
    }

    let payload_encoding = if data.first().is_some_and(|byte| byte & 1 == 0) {
        PayloadEncoding::Stored
    } else {
        PayloadEncoding::RawDeflate
    };
    let _ = decode_payload(payload_encoding, data, budget);

    let selector = data.first().copied().unwrap_or_default();
    let _ = traverse(
        Cursor::new(data.to_vec()),
        budget,
        |path, entry| match (selector as usize + path.len() + entry.name.len()) % 5 {
            0 => TraversalAction::Skip,
            1 => TraversalAction::Leaf(PayloadEncoding::Stored),
            2 => TraversalAction::Leaf(PayloadEncoding::RawDeflate),
            3 => TraversalAction::Container(PayloadEncoding::Stored),
            _ => TraversalAction::Container(PayloadEncoding::RawDeflate),
        },
        |_| {},
    );
});
