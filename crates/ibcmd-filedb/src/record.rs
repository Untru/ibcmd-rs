//! Field codecs used by the CONFIG-family tables.

use crate::error::{Result, corrupt};

/// Packed decimal of `digits` digits: a sign nibble (1 = non-negative), the
/// digits most significant first, a zero pad nibble when the count is even.
pub(crate) fn pack_decimal(value: u64, digits: usize) -> Vec<u8> {
    let text = format!("{value:0digits$}");
    let mut nibbles = Vec::with_capacity(digits + 2);
    nibbles.push(1u8);
    nibbles.extend(text.bytes().map(|b| b - b'0'));
    if nibbles.len() % 2 == 1 {
        nibbles.push(0);
    }
    nibbles.chunks(2).map(|p| (p[0] << 4) | p[1]).collect()
}

pub(crate) fn unpack_decimal(bytes: &[u8], digits: usize) -> Result<u64> {
    let mut value: u64 = 0;
    let sign = bytes.first().map(|b| b >> 4).unwrap_or(0);
    if sign != 1 {
        return Err(corrupt("Отрицательное или неверное число в записи"));
    }
    for i in 1..=digits {
        let byte = *bytes
            .get(i / 2)
            .ok_or_else(|| corrupt("Число короче описания"))?;
        let nibble = if i % 2 == 0 { byte >> 4 } else { byte & 0x0F };
        if nibble > 9 {
            return Err(corrupt("Неверная цифра в числе"));
        }
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(nibble as u64))
            .ok_or_else(|| corrupt("Число вне диапазона"))?;
    }
    Ok(value)
}

/// `DT`: 7 bytes of packed `yyyyMMddhhmmss`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub [u8; 7]);

impl Timestamp {
    /// From calendar parts; out-of-range parts are refused.
    pub fn new(
        year: u32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> Option<Self> {
        if year > 9999
            || !(1..=12).contains(&month)
            || !(1..=31).contains(&day)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return None;
        }
        let digits = format!("{year:04}{month:02}{day:02}{hour:02}{minute:02}{second:02}");
        let d = digits.as_bytes();
        let mut out = [0u8; 7];
        for (i, pair) in d.chunks(2).enumerate() {
            out[i] = ((pair[0] - b'0') << 4) | (pair[1] - b'0');
        }
        Some(Timestamp(out))
    }

    /// The current UTC time, as the platform stamps rows it writes.
    pub fn now_utc() -> Self {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let days = (secs / 86_400) as i64;
        let rem = secs % 86_400;
        let (y, m, d) = civil_from_days(days);
        Timestamp::new(
            y as u32,
            m,
            d,
            (rem / 3600) as u32,
            (rem % 3600 / 60) as u32,
            (rem % 60) as u32,
        )
        .unwrap_or(Timestamp([0; 7]))
    }

    pub fn digits(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

// Howard Hinnant's days-to-civil conversion.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `NVC(length)`: `u16` character count, then UTF-16LE padded with spaces
/// (`20 00`), as the platform writes it.
pub(crate) fn decode_var_string(bytes: &[u8], length: usize) -> Result<String> {
    let count = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
    if count > length {
        return Err(corrupt("Строка длиннее описания поля"));
    }
    let units: Vec<u16> = bytes[2..2 + count * 2]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    String::from_utf16(&units).map_err(|_| corrupt("Строка не в UTF-16"))
}

pub(crate) fn encode_var_string(value: &str, length: usize, out: &mut [u8]) -> Result<()> {
    let units: Vec<u16> = value.encode_utf16().collect();
    if units.len() > length {
        return Err(crate::error::Error::Refused(format!(
            "Имя «{value}» длиннее {length} символов"
        )));
    }
    out[0..2].copy_from_slice(&(units.len() as u16).to_le_bytes());
    for pad in out[2..].as_chunks_mut::<2>().0 {
        *pad = [0x20, 0x00];
    }
    for (i, u) in units.iter().enumerate() {
        out[2 + i * 2..4 + i * 2].copy_from_slice(&u.to_le_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_round_trip_like_native_fields() {
        // PARTNO N(10) = 1 is stored as 10 00 00 00 00 10 in native files.
        assert_eq!(pack_decimal(1, 10), [0x10, 0, 0, 0, 0, 0x10]);
        assert_eq!(pack_decimal(0, 10), [0x10, 0, 0, 0, 0, 0]);
        // ATTRIBUTES N(5) takes 3 bytes, DATASIZE N(20) 11.
        assert_eq!(pack_decimal(12345, 5), [0x11, 0x23, 0x45]);
        assert_eq!(pack_decimal(172_797_914, 20).len(), 11);
        for (v, n) in [
            (0u64, 5usize),
            (99_999, 5),
            (172_797_914, 20),
            (4_294_967_295, 10),
        ] {
            assert_eq!(unpack_decimal(&pack_decimal(v, n), n).unwrap(), v);
        }
        assert!(unpack_decimal(&[0x00, 0x00, 0x01], 5).is_err());
        assert!(unpack_decimal(&[0x1A, 0x00, 0x00], 5).is_err());
    }

    #[test]
    fn timestamps_pack_as_digits() {
        let t = Timestamp::new(2026, 10, 21, 20, 18, 56).unwrap();
        assert_eq!(t.digits(), "20261021201856");
        assert!(Timestamp::new(2026, 13, 1, 0, 0, 0).is_none());
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_747), (2026, 10, 21));
    }

    #[test]
    fn var_strings_round_trip() {
        let mut buf = vec![0u8; 258];
        encode_var_string("versions", 128, &mut buf).unwrap();
        assert_eq!(decode_var_string(&buf, 128).unwrap(), "versions");
        assert_eq!(&buf[18..22], &[0x20, 0x00, 0x20, 0x00]);
        assert!(encode_var_string(&"x".repeat(129), 128, &mut buf).is_err());
    }
}
