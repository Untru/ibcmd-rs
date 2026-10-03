// SPDX-License-Identifier: GPL-2.0-only WITH Classpath-exception-2.0
//! Java floating-point lexical grammar used by the original EMF EFloat/EDouble
//! factory. Hex conversion rounds the current significand directly to the
//! requested IEEE primitive; it does not pass binary32 through binary64.

enum Token<'a> {
    NaN,
    Infinity(bool),
    Decimal(&'a str),
    Hex {
        negative: bool,
        mantissa: &'a str,
        exponent: &'a str,
    },
}

fn token(text: &str) -> Option<Token<'_>> {
    let text = text.trim_matches(|c: char| c <= '\u{20}');
    let (negative, unsigned) = match text.as_bytes().first()? {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    match unsigned {
        "NaN" => return Some(Token::NaN),
        "Infinity" => return Some(Token::Infinity(negative)),
        _ => (),
    }
    let value = text.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(text);
    let unsigned = value.strip_prefix(['-', '+']).unwrap_or(value);
    if let Some(hex) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
    {
        let p = hex.find(['p', 'P'])?;
        let (mantissa, rest) = hex.split_at(p);
        let exponent = &rest[1..];
        let digits = exponent.strip_prefix(['-', '+']).unwrap_or(exponent);
        if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let mut dots = 0;
        let mut count = 0;
        for c in mantissa.bytes() {
            if c == b'.' {
                dots += 1;
            } else if c.is_ascii_hexdigit() {
                count += 1;
            } else {
                return None;
            }
        }
        if dots > 1 || count == 0 {
            return None;
        }
        return Some(Token::Hex {
            negative,
            mantissa,
            exponent,
        });
    }
    let mut chars = unsigned.bytes().peekable();
    let mut digits = 0;
    while chars.peek().is_some_and(u8::is_ascii_digit) {
        digits += 1;
        chars.next();
    }
    if chars.peek() == Some(&b'.') {
        chars.next();
        while chars.peek().is_some_and(u8::is_ascii_digit) {
            digits += 1;
            chars.next();
        }
    }
    if digits == 0 {
        return None;
    }
    if chars.peek().is_some_and(|c| matches!(c, b'e' | b'E')) {
        chars.next();
        if chars.peek().is_some_and(|c| matches!(c, b'-' | b'+')) {
            chars.next();
        }
        let mut exponent_digits = 0;
        while chars.peek().is_some_and(u8::is_ascii_digit) {
            exponent_digits += 1;
            chars.next();
        }
        if exponent_digits == 0 {
            return None;
        }
    }
    if chars.next().is_some() {
        return None;
    }
    Some(Token::Decimal(value))
}

fn exponent(text: &str) -> i128 {
    let negative = text.starts_with('-');
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    let magnitude = digits.bytes().fold(0i128, |n, c| {
        n.saturating_mul(10).saturating_add(i128::from(c - b'0'))
    });
    if negative {
        -magnitude
    } else {
        magnitude
    }
}

fn hex_bits(negative: bool, mantissa: &str, power: &str, precision: u32, bias: i32) -> Option<u64> {
    let sign = u64::from(negative) << if precision == 24 { 31 } else { 63 };
    let infinity = if precision == 24 {
        0x7f800000
    } else {
        0x7ff0000000000000
    };
    let fraction_digits = mantissa.split_once('.').map_or(0, |(_, f)| f.len());
    let mut digits = Vec::new();
    digits.try_reserve_exact(mantissa.len()).ok()?;
    for c in mantissa.bytes().filter(|c| *c != b'.') {
        digits.push(match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return None,
        });
    }
    let Some(first) = digits.iter().position(|c| *c != 0) else {
        return Some(sign);
    };
    let leading_bit = 7 - digits[first].leading_zeros() as usize;
    let bit_count = (digits.len() - first - 1)
        .checked_mul(4)?
        .checked_add(leading_bit + 1)?;
    let scale =
        exponent(power).saturating_sub(i128::try_from(fraction_digits).ok()?.saturating_mul(4));
    let high = scale.saturating_add(i128::try_from(bit_count - 1).ok()?);
    if high > i128::from(bias) {
        return Some(sign | infinity);
    }
    let minimum_normal = 1 - bias;
    let minimum_bit = minimum_normal - (precision as i32 - 1);
    if high < i128::from(minimum_bit - 1) {
        return Some(sign);
    }
    let normal = high >= i128::from(minimum_normal);
    let wanted = if normal {
        precision as usize
    } else {
        usize::try_from(high - i128::from(minimum_bit) + 1).ok()?
    };
    let mut magnitude = 0u64;
    let mut guard = false;
    let mut sticky = false;
    let mut seen = 0usize;
    for (index, digit) in digits[first..].iter().enumerate() {
        let top = if index == 0 { leading_bit } else { 3 };
        for bit in (0..=top).rev() {
            let set = digit & (1 << bit) != 0;
            if seen < wanted {
                magnitude = (magnitude << 1) | u64::from(set);
            } else if seen == wanted {
                guard = set;
            } else {
                sticky |= set;
            }
            seen += 1;
        }
    }
    if seen < wanted {
        magnitude <<= wanted - seen;
    }
    if guard && (sticky || magnitude & 1 != 0) {
        magnitude += 1;
    }
    if !normal {
        return Some(sign | magnitude);
    }
    let mut high = i32::try_from(high).ok()?;
    if magnitude == 1u64 << precision {
        magnitude >>= 1;
        high += 1;
    }
    if high > bias {
        return Some(sign | infinity);
    }
    let fraction_mask = (1u64 << (precision - 1)) - 1;
    Some(sign | (u64::try_from(high + bias).ok()? << (precision - 1)) | (magnitude & fraction_mask))
}

pub fn parse_binary32(text: &str) -> Option<f32> {
    match token(text)? {
        Token::NaN => Some(f32::NAN),
        Token::Infinity(negative) => Some(if negative {
            f32::NEG_INFINITY
        } else {
            f32::INFINITY
        }),
        Token::Decimal(text) => text.parse().ok(),
        Token::Hex {
            negative,
            mantissa,
            exponent,
        } => Some(f32::from_bits(
            u32::try_from(hex_bits(negative, mantissa, exponent, 24, 127)?).ok()?,
        )),
    }
}

pub fn parse_binary64(text: &str) -> Option<f64> {
    match token(text)? {
        Token::NaN => Some(f64::NAN),
        Token::Infinity(negative) => Some(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }),
        Token::Decimal(text) => text.parse().ok(),
        Token::Hex {
            negative,
            mantissa,
            exponent,
        } => Some(f64::from_bits(hex_bits(
            negative, mantissa, exponent, 53, 1023,
        )?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emf_java_lexical_domain_and_hex_rounding() {
        for text in ["  1.25\t", "1.25f", "1.25D"] {
            assert_eq!(parse_binary32(text), Some(1.25));
            assert_eq!(parse_binary64(text), Some(1.25));
        }
        for text in [
            "inf",
            "NaNf",
            "InfinityD",
            "1.2.3",
            "１２.５",
            "0x1",
            "0x.p1",
            "1e",
            "1_0",
            "0x1p1p2",
        ] {
            assert!(parse_binary32(text).is_none(), "{text}");
            assert!(parse_binary64(text).is_none(), "{text}");
        }
        for text in ["NaN", "+NaN", "-NaN"] {
            assert!(parse_binary32(text).unwrap().is_nan());
            assert!(parse_binary64(text).unwrap().is_nan());
        }
        assert_eq!(parse_binary32("0x1.8p1"), Some(3.0));
        assert_eq!(parse_binary64("0x1.8p1"), Some(3.0));
        for (text, bits) in [
            ("0x1p-149", 1),
            ("0x1p-150", 0),
            ("0x1.000001p-150", 1),
            ("-0x1p-500", 0x80000000),
            ("0x1.000001p0", 0x3f800000),
            ("0x1.000003p0", 0x3f800002),
            ("0x1.fffffep127", 0x7f7fffff),
            ("0x1p2147483648", 0x7f800000),
        ] {
            assert_eq!(parse_binary32(text).unwrap().to_bits(), bits, "{text}");
        }
        for (text, bits) in [
            ("0x1p-1074", 1),
            ("0x1p-1075", 0),
            ("0x1.0000000000001p-1075", 1),
            ("0x1.00000000000008p0", 0x3ff0000000000000),
            ("0x1.00000000000018p0", 0x3ff0000000000002),
            ("0x1.fffffffffffffp1023", 0x7fefffffffffffff),
        ] {
            assert_eq!(parse_binary64(text).unwrap().to_bits(), bits, "{text}");
        }
    }
}
