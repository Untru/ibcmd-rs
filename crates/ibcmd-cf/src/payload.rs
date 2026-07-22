//! Strict bounded decoding for CF entry payloads.
//!
//! Raw DEFLATE is decoded with the low-level streaming API so `StreamEnd`,
//! complete input consumption, aggregate byte budgets, and compression ratio
//! are all verified before bytes are accepted.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use flate2::{Compress, Compression, Decompress, FlushCompress, FlushDecompress, Status};
use ibcmd_core::limits::{
    ResourceBudget, ResourceLimitError, ResourceLimits, ensure_compression_ratio,
};

const CODEC_BUFFER_BYTES: usize = 16 * 1024;

/// Storage encoding declared by one CF payload entry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PayloadEncoding {
    /// Bytes are stored verbatim.
    Stored,
    /// Bytes are one complete raw RFC 1951 DEFLATE stream.
    RawDeflate,
}

/// Decoded payload bytes paired with their verified storage encoding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedPayload {
    encoding: PayloadEncoding,
    bytes: Vec<u8>,
}

impl DecodedPayload {
    pub const fn encoding(&self) -> PayloadEncoding {
        self.encoding
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Stateful decoder that enforces aggregate archive limits.
#[derive(Clone, Debug)]
pub struct PayloadDecoder {
    budget: ResourceBudget,
}

impl PayloadDecoder {
    pub const fn new(limits: ResourceLimits) -> Self {
        Self {
            budget: ResourceBudget::new(limits),
        }
    }

    pub const fn budget(&self) -> &ResourceBudget {
        &self.budget
    }

    pub fn enter_container(&mut self) -> Result<(), PayloadDecodeError> {
        self.budget.enter_container().map_err(Into::into)
    }

    pub fn leave_container(&mut self) -> Result<(), PayloadDecodeError> {
        self.budget.leave_container().map_err(Into::into)
    }

    /// Decodes one payload and commits accounting only after full validation.
    pub fn decode(
        &mut self,
        encoding: PayloadEncoding,
        input: &[u8],
    ) -> Result<DecodedPayload, PayloadDecodeError> {
        let encoded = u64::try_from(input.len()).map_err(|_| {
            PayloadDecodeError::Limit(ResourceLimitError::EncodedBytesExceeded {
                maximum: self.budget.limits().max_encoded_bytes(),
                actual: u64::MAX,
            })
        })?;

        // Check entry count and encoded aggregate before doing codec work. The
        // clone preserves atomic accounting if decoding later fails.
        let mut candidate = self.budget.clone();
        candidate.record_payload(encoded, 0, false)?;

        let remaining_decoded = self.budget.remaining_decoded_bytes();
        let bytes = match encoding {
            PayloadEncoding::Stored => {
                if encoded > remaining_decoded {
                    return Err(ResourceLimitError::DecodedBytesExceeded {
                        maximum: self.budget.limits().max_decoded_bytes(),
                        actual: self.budget.decoded_bytes().saturating_add(encoded),
                    }
                    .into());
                }
                input.to_vec()
            }
            PayloadEncoding::RawDeflate => decode_raw_deflate(
                input,
                remaining_decoded,
                self.budget.limits().max_compression_ratio(),
            )?,
        };
        let decoded = u64::try_from(bytes.len()).map_err(|_| {
            PayloadDecodeError::Limit(ResourceLimitError::DecodedBytesExceeded {
                maximum: self.budget.limits().max_decoded_bytes(),
                actual: u64::MAX,
            })
        })?;

        // Rebuild from the original budget because the preflight candidate
        // already counted the entry with zero decoded bytes.
        let mut committed = self.budget.clone();
        committed.record_payload(encoded, decoded, encoding == PayloadEncoding::RawDeflate)?;
        self.budget = committed;
        Ok(DecodedPayload { encoding, bytes })
    }
}

/// Decodes exactly one payload with a fresh budget.
pub fn decode_payload(
    encoding: PayloadEncoding,
    input: &[u8],
    limits: ResourceLimits,
) -> Result<DecodedPayload, PayloadDecodeError> {
    PayloadDecoder::new(limits).decode(encoding, input)
}

/// Encodes one payload without exceeding the same per-container byte limits.
pub fn encode_payload(
    encoding: PayloadEncoding,
    input: &[u8],
    limits: ResourceLimits,
) -> Result<Vec<u8>, PayloadEncodeError> {
    let decoded = u64::try_from(input.len()).map_err(|_| {
        PayloadEncodeError::Limit(ResourceLimitError::DecodedBytesExceeded {
            maximum: limits.max_decoded_bytes(),
            actual: u64::MAX,
        })
    })?;
    if decoded > limits.max_decoded_bytes() {
        return Err(ResourceLimitError::DecodedBytesExceeded {
            maximum: limits.max_decoded_bytes(),
            actual: decoded,
        }
        .into());
    }
    match encoding {
        PayloadEncoding::Stored => {
            if decoded > limits.max_encoded_bytes() {
                return Err(ResourceLimitError::EncodedBytesExceeded {
                    maximum: limits.max_encoded_bytes(),
                    actual: decoded,
                }
                .into());
            }
            Ok(input.to_vec())
        }
        PayloadEncoding::RawDeflate => encode_raw_deflate(input, limits.max_encoded_bytes()),
    }
}

fn decode_raw_deflate(
    input: &[u8],
    maximum_decoded: u64,
    maximum_ratio: u64,
) -> Result<Vec<u8>, PayloadDecodeError> {
    let encoded = u64::try_from(input.len()).unwrap_or(u64::MAX);
    let mut inflater = Decompress::new(false);
    let mut output = Vec::with_capacity(input.len().saturating_mul(2).min(CODEC_BUFFER_BYTES));
    let mut input_offset = 0usize;
    let mut buffer = [0_u8; CODEC_BUFFER_BYTES];

    loop {
        let before_in = inflater.total_in();
        let before_out = inflater.total_out();
        let status = inflater
            .decompress(&input[input_offset..], &mut buffer, FlushDecompress::Finish)
            .map_err(|error| PayloadDecodeError::InvalidDeflate(error.to_string()))?;
        let consumed = usize::try_from(inflater.total_in() - before_in)
            .map_err(|_| PayloadDecodeError::CounterOverflow)?;
        let produced = usize::try_from(inflater.total_out() - before_out)
            .map_err(|_| PayloadDecodeError::CounterOverflow)?;
        input_offset = input_offset
            .checked_add(consumed)
            .ok_or(PayloadDecodeError::CounterOverflow)?;

        let decoded_total = inflater.total_out();
        if decoded_total > maximum_decoded {
            return Err(ResourceLimitError::DecodedBytesExceeded {
                maximum: maximum_decoded,
                actual: decoded_total,
            }
            .into());
        }
        ensure_compression_ratio(encoded, decoded_total, maximum_ratio)?;
        output.extend_from_slice(&buffer[..produced]);

        match status {
            Status::StreamEnd => {
                if input_offset != input.len() {
                    return Err(PayloadDecodeError::TrailingBytes {
                        trailing: input.len() - input_offset,
                    });
                }
                return Ok(output);
            }
            Status::Ok | Status::BufError => {
                if consumed == 0 && produced == 0 {
                    return Err(PayloadDecodeError::TruncatedDeflate);
                }
            }
        }
    }
}

fn encode_raw_deflate(input: &[u8], maximum_encoded: u64) -> Result<Vec<u8>, PayloadEncodeError> {
    let mut deflater = Compress::new(Compression::default(), false);
    let mut output = Vec::with_capacity(input.len().min(CODEC_BUFFER_BYTES));
    let mut input_offset = 0usize;
    let mut buffer = [0_u8; CODEC_BUFFER_BYTES];

    loop {
        let before_in = deflater.total_in();
        let before_out = deflater.total_out();
        let status = deflater
            .compress(&input[input_offset..], &mut buffer, FlushCompress::Finish)
            .map_err(|error| PayloadEncodeError::Deflate(error.to_string()))?;
        let consumed = usize::try_from(deflater.total_in() - before_in)
            .map_err(|_| PayloadEncodeError::CounterOverflow)?;
        let produced = usize::try_from(deflater.total_out() - before_out)
            .map_err(|_| PayloadEncodeError::CounterOverflow)?;
        input_offset = input_offset
            .checked_add(consumed)
            .ok_or(PayloadEncodeError::CounterOverflow)?;

        if deflater.total_out() > maximum_encoded {
            return Err(ResourceLimitError::EncodedBytesExceeded {
                maximum: maximum_encoded,
                actual: deflater.total_out(),
            }
            .into());
        }
        output.extend_from_slice(&buffer[..produced]);
        match status {
            Status::StreamEnd => return Ok(output),
            Status::Ok | Status::BufError => {
                if consumed == 0 && produced == 0 {
                    return Err(PayloadEncodeError::StalledDeflate);
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum PayloadDecodeError {
    Limit(ResourceLimitError),
    InvalidDeflate(String),
    TruncatedDeflate,
    TrailingBytes { trailing: usize },
    CounterOverflow,
}

impl Display for PayloadDecodeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(source) => {
                write!(formatter, "payload resource limit rejected input: {source}")
            }
            Self::InvalidDeflate(reason) => {
                write!(formatter, "invalid raw DEFLATE payload: {reason}")
            }
            Self::TruncatedDeflate => {
                write!(formatter, "raw DEFLATE payload ended before StreamEnd")
            }
            Self::TrailingBytes { trailing } => write!(
                formatter,
                "raw DEFLATE payload has {trailing} trailing byte(s) after StreamEnd"
            ),
            Self::CounterOverflow => write!(formatter, "payload codec byte counter overflow"),
        }
    }
}

impl Error for PayloadDecodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Limit(source) => Some(source),
            _ => None,
        }
    }
}

impl From<ResourceLimitError> for PayloadDecodeError {
    fn from(source: ResourceLimitError) -> Self {
        Self::Limit(source)
    }
}

#[derive(Debug)]
pub enum PayloadEncodeError {
    Limit(ResourceLimitError),
    Deflate(String),
    StalledDeflate,
    CounterOverflow,
}

impl Display for PayloadEncodeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(source) => write!(
                formatter,
                "payload resource limit rejected output: {source}"
            ),
            Self::Deflate(reason) => write!(formatter, "failed to raw-deflate payload: {reason}"),
            Self::StalledDeflate => write!(formatter, "raw DEFLATE encoder made no progress"),
            Self::CounterOverflow => write!(formatter, "payload codec byte counter overflow"),
        }
    }
}

impl Error for PayloadEncodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Limit(source) => Some(source),
            _ => None,
        }
    }
}

impl From<ResourceLimitError> for PayloadEncodeError {
    fn from(source: ResourceLimitError) -> Self {
        Self::Limit(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(decoded: u64, ratio: u64) -> ResourceLimits {
        ResourceLimits::new(2, 2, 1_048_576, decoded, ratio).unwrap()
    }

    #[test]
    fn stored_and_raw_deflate_roundtrip() {
        let input = b"standalone CF payload\r\nwith binary \0 bytes";
        for encoding in [PayloadEncoding::Stored, PayloadEncoding::RawDeflate] {
            let encoded = encode_payload(encoding, input, limits(1_048_576, 200)).unwrap();
            let decoded = decode_payload(encoding, &encoded, limits(1_048_576, 200)).unwrap();
            assert_eq!(decoded.encoding(), encoding);
            assert_eq!(decoded.bytes(), input);
        }
    }

    #[test]
    fn raw_deflate_requires_stream_end_and_full_input_consumption() {
        let encoded = encode_payload(
            PayloadEncoding::RawDeflate,
            b"complete",
            limits(1_048_576, 200),
        )
        .unwrap();

        let truncated = &encoded[..encoded.len() - 1];
        assert!(matches!(
            decode_payload(
                PayloadEncoding::RawDeflate,
                truncated,
                limits(1_048_576, 200)
            ),
            Err(PayloadDecodeError::TruncatedDeflate) | Err(PayloadDecodeError::InvalidDeflate(_))
        ));

        let mut trailing = encoded;
        trailing.extend_from_slice(b"junk");
        assert!(matches!(
            decode_payload(
                PayloadEncoding::RawDeflate,
                &trailing,
                limits(1_048_576, 200)
            ),
            Err(PayloadDecodeError::TrailingBytes { trailing: 4 })
        ));
    }

    #[test]
    fn decompression_bomb_is_rejected_by_bytes_or_ratio() {
        let expanded = vec![b'A'; 256 * 1024];
        let encoded = encode_payload(
            PayloadEncoding::RawDeflate,
            &expanded,
            limits(512 * 1024, 10_000),
        )
        .unwrap();
        let error =
            decode_payload(PayloadEncoding::RawDeflate, &encoded, limits(4 * 1024, 8)).unwrap_err();
        assert!(matches!(
            error,
            PayloadDecodeError::Limit(ResourceLimitError::DecodedBytesExceeded { .. })
                | PayloadDecodeError::Limit(ResourceLimitError::CompressionRatioExceeded { .. })
        ));
    }

    #[test]
    fn aggregate_entry_and_depth_limits_are_enforced() {
        let mut decoder = PayloadDecoder::new(limits(1_048_576, 200));
        decoder.enter_container().unwrap();
        decoder.enter_container().unwrap();
        assert!(matches!(
            decoder.enter_container(),
            Err(PayloadDecodeError::Limit(
                ResourceLimitError::DepthExceeded { .. }
            ))
        ));
        decoder.decode(PayloadEncoding::Stored, b"one").unwrap();
        decoder.decode(PayloadEncoding::Stored, b"two").unwrap();
        assert!(matches!(
            decoder.decode(PayloadEncoding::Stored, b"three"),
            Err(PayloadDecodeError::Limit(
                ResourceLimitError::EntryCountExceeded { .. }
            ))
        ));
    }
}
