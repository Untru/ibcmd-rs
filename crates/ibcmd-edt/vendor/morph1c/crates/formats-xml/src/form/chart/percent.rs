//! Exact installed SDK percentage projection and Java17 binary64 rendering.
use super::*;

fn parts(text: &str) -> Result<(bool, String, i64), FormError> {
    let normalized = super::semantic::normalize_big_decimal(text)
        .ok_or_else(|| frame("chart: invalid current BigDecimal percentage".into()))?;
    let text = normalized.as_str();
    let negative = text.starts_with('-');
    let unsigned = text.strip_prefix(['-', '+']).unwrap_or(text);
    let (mantissa, exponent) = match unsigned.split_once(['e', 'E']) {
        Some((m, e)) => (
            m,
            e.parse::<i32>()
                .map_err(|_| frame("chart: invalid decimal exponent".into()))? as i64,
        ),
        None => (unsigned, 0),
    };
    let fractional = mantissa.split_once('.').map_or(0, |(_, f)| f.len());
    let fractional = i64::try_from(fractional)
        .map_err(|_| frame("chart: decimal scale length overflow".into()))?;
    let mut exponent = exponent
        .checked_sub(fractional)
        .ok_or_else(|| frame("chart: decimal scale overflow".into()))?;
    if i32::try_from(-exponent).is_err() {
        return Err(frame(
            "chart: BigDecimal scale outside signed32 model domain".into(),
        ));
    }
    let mut digits = mantissa.chars().filter(|c| *c != '.').collect::<String>();
    let first = digits
        .bytes()
        .position(|c| c != b'0')
        .unwrap_or(digits.len());
    digits.drain(..first);
    if digits.is_empty() {
        return Ok((false, "0".into(), 0));
    }
    while digits.ends_with('0') {
        digits.pop();
        exponent = exponent
            .checked_add(1)
            .ok_or_else(|| frame("chart: decimal scale overflow".into()))?;
    }
    Ok((negative, digits, exponent))
}

/// Exact numeric comparison, without expanding arbitrarily large decimal exponents.
#[doc(hidden)]
pub fn percentage_numeric_equal(left: &str, right: &str) -> Result<bool, FormError> {
    Ok(parts(left)? == parts(right)?)
}

/// The actual SDK integer branch returns BigDecimal.intValue(), including low32-bit wrapping.
/// None means the original decimal is fractional; selection never depends on rounded f64.
#[doc(hidden)]
pub fn native_percentage_integer(text: &str) -> Result<Option<(String, bool)>, FormError> {
    let (negative, digits, exponent) = parts(text)?;
    if exponent < 0 {
        return Ok(None);
    }
    let mut value = 0u32;
    for digit in digits.bytes() {
        value = value.wrapping_mul(10).wrapping_add(u32::from(digit - b'0'));
    }
    let (mut factor, mut power, mut e) = (1u32, 10u32, exponent as u64);
    while e > 0 {
        if e & 1 == 1 {
            factor = factor.wrapping_mul(power);
        }
        power = power.wrapping_mul(power);
        e >>= 1;
    }
    value = value.wrapping_mul(factor);
    if negative {
        value = 0u32.wrapping_sub(value);
    }
    let plain = (value as i32).to_string();
    let equal = percentage_numeric_equal(text, &plain)?;
    Ok(Some((plain, equal)))
}

/// Original SDK plain-number projection and whether it retains the current
/// decimal value. Call exactly once: a fractional double output can be integral.
/// Nonfinite overflow output is retained here; its preservation flag is false.
#[doc(hidden)]
pub fn native_percentage_projection(text: &str) -> Result<(String, bool), FormError> {
    if let Some(integer) = native_percentage_integer(text)? {
        return Ok(integer);
    }
    let value = decimal_double(text)?;
    let output = ibcmd_number_format::format_binary64(value);
    let equal = value.is_finite() && percentage_numeric_equal(text, &output)?;
    Ok((output, equal))
}
fn decimal_double(text: &str) -> Result<f64, FormError> {
    let (negative, digits, exponent) = parts(text)?;
    let normalized = super::semantic::normalize_big_decimal(text)
        .ok_or_else(|| frame("chart: invalid current BigDecimal percentage".into()))?;
    let text = normalized.as_str();
    let unsigned = text.strip_prefix(['-', '+']).unwrap_or(text);
    let (mantissa, raw_exponent) = unsigned
        .split_once(['e', 'E'])
        .map_or((unsigned, 0), |(m, e)| {
            (m, e.parse::<i32>().expect("validated decimal exponent"))
        });
    let fractional = mantissa.split_once('.').map_or(0, |(_, f)| f.len());
    let scale = i64::try_from(fractional)
        .map_err(|_| frame("chart: decimal scale overflow".into()))?
        - i64::from(raw_exponent);
    let coefficient = mantissa
        .bytes()
        .filter(|b| *b != b'.')
        .try_fold(0i64, |n, b| {
            n.checked_mul(10)?.checked_add(i64::from(b - b'0'))
        });
    if let Some(coefficient) = coefficient {
        let coefficient = if negative { -coefficient } else { coefficient };
        if scale == 0 {
            return Ok(coefficient as f64);
        }
        const POWERS: [f64; 23] = [
            1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15,
            1e16, 1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
        ];
        if coefficient.abs() < (1i64 << 52) {
            if scale > 0 && scale < 23 {
                return Ok(coefficient as f64 / POWERS[scale as usize]);
            }
            if scale < 0 && scale > -23 {
                return Ok(coefficient as f64 * POWERS[(-scale) as usize]);
            }
        }
    }
    let normalized = format!("{}{}e{}", if negative { "-" } else { "" }, digits, exponent);
    normalized
        .parse::<f64>()
        .map_err(|_| frame("chart: invalid decimal-to-binary percentage".into()))
}
