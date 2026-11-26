/*
 * Copyright (c) 1996, 2016, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */

// SPDX-License-Identifier: GPL-2.0-only WITH Classpath-exception-2.0
// Modified 2026-10-02: Rust translation of FloatingDecimal binary64 emission;
// independently implemented natural-number limbs; no JVM or original binary code.
// Original source SHA-256: 3bdf29124dd5f43fa01ea82a552d2ff9db1fc39bbc1e929888bb664fe4bf8d8b

//! Binary64 decimal rendering with the installed JDK17 stopping and rounding rules.
//! Uses exact unsigned arithmetic for the large branch; small branches retain
//! Java's signed wrapping arithmetic, which has distinct stopping behavior.
use std::cmp::Ordering;
mod parse;
pub use parse::{parse_binary32, parse_binary64};

#[derive(Clone)]
struct Natural(Vec<u32>);
impl Natural {
    fn new(value: u64) -> Self {
        let mut v = vec![value as u32];
        if value >> 32 != 0 {
            v.push((value >> 32) as u32);
        }
        Self(v)
    }
    fn trim(&mut self) {
        while self.0.len() > 1 && self.0.last() == Some(&0) {
            self.0.pop();
        }
    }
    fn mul(&mut self, value: u32) {
        let mut carry = 0u64;
        for limb in &mut self.0 {
            let x = u64::from(*limb) * u64::from(value) + carry;
            *limb = x as u32;
            carry = x >> 32;
        }
        if carry != 0 {
            self.0.push(carry as u32);
        }
    }
    fn shift(&mut self, bits: i32) {
        debug_assert!(bits >= 0);
        let words = (bits / 32) as usize;
        let rem = (bits % 32) as u32;
        let mut out = vec![0; words];
        let mut carry = 0u64;
        for &v in &self.0 {
            let x = (u64::from(v) << rem) | carry;
            out.push(x as u32);
            carry = x >> 32;
        }
        if carry != 0 {
            out.push(carry as u32);
        }
        self.0 = out;
        self.trim();
    }
    fn powered(value: u64, fives: i32, twos: i32) -> Self {
        let mut n = Self::new(value);
        for _ in 0..fives {
            n.mul(5);
        }
        n.shift(twos);
        n
    }
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .len()
            .cmp(&other.0.len())
            .then_with(|| self.0.iter().rev().cmp(other.0.iter().rev()))
    }
    fn sub(&mut self, other: &Self) {
        debug_assert!(self.cmp(other) != Ordering::Less);
        let mut borrow = 0u64;
        for (i, a) in self.0.iter_mut().enumerate() {
            let b = u64::from(other.0.get(i).copied().unwrap_or(0)) + borrow;
            let av = u64::from(*a);
            *a = av.wrapping_sub(b) as u32;
            borrow = u64::from(av < b);
        }
        debug_assert_eq!(borrow, 0);
        self.trim();
    }
    fn add(&self, other: &Self) -> Self {
        let mut v = Vec::with_capacity(self.0.len().max(other.0.len()) + 1);
        let mut carry = 0u64;
        for i in 0..self.0.len().max(other.0.len()) {
            let x = u64::from(self.0.get(i).copied().unwrap_or(0))
                + u64::from(other.0.get(i).copied().unwrap_or(0))
                + carry;
            v.push(x as u32);
            carry = x >> 32;
        }
        if carry != 0 {
            v.push(carry as u32);
        }
        Self(v)
    }
    fn digit(&mut self, divisor: &Self) -> u8 {
        let mut q = 0;
        while self.cmp(divisor) != Ordering::Less {
            self.sub(divisor);
            q += 1;
        }
        debug_assert!(q <= 9);
        self.mul(10);
        q
    }
}
const FIVE_BITS: [i32; 27] = [
    0, 3, 5, 7, 10, 12, 14, 17, 19, 21, 24, 26, 28, 31, 33, 35, 38, 40, 42, 45, 47, 49, 52, 54, 56,
    59, 61,
];
fn five_bits(n: i32) -> i32 {
    FIVE_BITS.get(n as usize).copied().unwrap_or(n * 3)
}
fn pow5(n: i32) -> i64 {
    let mut x = 1i64;
    for _ in 0..n {
        x = x.wrapping_mul(5);
    }
    x
}
fn signed(value: i64, width: u32) -> i64 {
    if width == 32 {
        i64::from(value as i32)
    } else {
        value
    }
}
fn rounded(digits: &mut [u8], exponent: &mut i32) {
    let mut i = digits.len() - 1;
    while digits[i] == 9 && i > 0 {
        digits[i] = 0;
        i -= 1;
    }
    if digits[i] == 9 {
        digits[i] = 1;
        *exponent += 1;
    } else {
        digits[i] += 1;
    }
}
fn render(negative: bool, digits: &[u8], exponent: i32) -> String {
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if exponent > 0 && exponent < 8 {
        let split = digits.len().min(exponent as usize);
        for &d in &digits[..split] {
            out.push(char::from(b'0' + d));
        }
        for _ in split..exponent as usize {
            out.push('0');
        }
        out.push('.');
        if split < digits.len() {
            for &d in &digits[split..] {
                out.push(char::from(b'0' + d));
            }
        } else {
            out.push('0');
        }
    } else if exponent <= 0 && exponent > -3 {
        out.push_str("0.");
        for _ in 0..-exponent {
            out.push('0');
        }
        for &d in digits {
            out.push(char::from(b'0' + d));
        }
    } else {
        out.push(char::from(b'0' + digits[0]));
        out.push('.');
        if digits.len() > 1 {
            for &d in &digits[1..] {
                out.push(char::from(b'0' + d));
            }
        } else {
            out.push('0');
        }
        out.push('E');
        out.push_str(&(exponent - 1).to_string());
    }
    out
}

pub fn format_binary64(value: f64) -> String {
    let raw = value.to_bits();
    let negative = raw >> 63 != 0;
    let mut fraction = raw & ((1u64 << 52) - 1);
    let mut bin = ((raw >> 52) & 0x7ff) as i32;
    if bin == 0x7ff {
        return if fraction != 0 {
            "NaN".into()
        } else if negative {
            "-Infinity".into()
        } else {
            "Infinity".into()
        };
    }
    if bin == 0 && fraction == 0 {
        return if negative {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    let significant;
    if bin == 0 {
        let leading = fraction.leading_zeros() as i32;
        let shift = leading - 11;
        fraction <<= shift;
        bin = 1 - shift;
        significant = 64 - leading;
    } else {
        fraction |= 1u64 << 52;
        significant = 53;
    }
    bin -= 1023;
    format_unpacked(negative, fraction, bin, significant)
}

/// Java 17 Float.toString: unpack binary32 and use FloatingDecimal's shared
/// compatible-format dtoa with the original 24-bit significance.
pub fn format_binary32(value: f32) -> String {
    let raw = value.to_bits();
    let negative = raw >> 31 != 0;
    let mut fraction = raw & ((1u32 << 23) - 1);
    let mut bin = ((raw >> 23) & 0xff) as i32;
    if bin == 0xff {
        return if fraction != 0 {
            "NaN".into()
        } else if negative {
            "-Infinity".into()
        } else {
            "Infinity".into()
        };
    }
    if bin == 0 && fraction == 0 {
        return if negative {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    let significant;
    if bin == 0 {
        let leading = fraction.leading_zeros() as i32;
        let shift = leading - 8;
        fraction <<= shift;
        bin = 1 - shift;
        significant = 32 - leading;
    } else {
        fraction |= 1u32 << 23;
        significant = 24;
    }
    format_unpacked(negative, u64::from(fraction) << 29, bin - 127, significant)
}

fn format_unpacked(negative: bool, mut fraction: u64, bin: i32, significant: i32) -> String {
    let tail = fraction.trailing_zeros() as i32;
    let nfract = 53 - tail;
    let tiny = (nfract - bin - 1).max(0);
    if (-21..=62).contains(&bin) && tiny < 27 && nfract + five_bits(tiny) < 64 && tiny == 0 {
        let insignificant = if bin > significant {
            let p = bin - significant - 1;
            if p > 1 && p < 64 {
                (1u64 << p).to_string().len() - 1
            } else {
                0
            }
        } else {
            0
        };
        let mut integer = if bin >= 52 {
            fraction << (bin - 52)
        } else {
            fraction >> (52 - bin)
        };
        let mut exp = insignificant as i32;
        if insignificant != 0 {
            let power = 10u64.pow(insignificant as u32);
            let residue = integer % power;
            integer /= power;
            if residue >= power / 2 {
                integer += 1;
            }
        }
        while integer % 10 == 0 {
            integer /= 10;
            exp += 1;
        }
        let digits: Vec<u8> = integer.to_string().bytes().map(|b| b - b'0').collect();
        exp += digits.len() as i32;
        return render(negative, &digits, exp);
    }
    let d2 = f64::from_bits((1023u64 << 52) | (fraction & ((1u64 << 52) - 1)));
    // Retain the original FloatingDecimal coefficient: replacing this rounded
    // estimate with LOG10_2 changes the source algorithm's boundary decisions.
    #[allow(clippy::approx_constant)]
    let estimate = (d2 - 1.5) * 0.289529654 + 0.176091259 + f64::from(bin) * 0.301029995663981;
    let mut exp = estimate.floor() as i32;
    let b5 = (-exp).max(0);
    let mut b2 = b5 + tiny + bin;
    let s5 = exp.max(0);
    let mut s2 = s5 + tiny;
    let m5 = b5;
    let mut m2 = b2 - significant;
    fraction >>= tail;
    b2 -= nfract - 1;
    let common = b2.min(s2);
    b2 -= common;
    s2 -= common;
    m2 -= common;
    if nfract == 1 {
        m2 -= 1;
    }
    if m2 < 0 {
        b2 -= m2;
        s2 -= m2;
        m2 = 0;
    }
    let bbits = nfract + b2 + five_bits(b5);
    let tenbits = s2 + 1 + five_bits(s5 + 1);
    let mut digits = Vec::with_capacity(20);
    let (mut low, mut high, difference);
    if bbits < 64 && tenbits < 64 {
        let width = if bbits < 32 && tenbits < 32 { 32 } else { 64 };
        let mut b = signed(
            (fraction as i64)
                .wrapping_mul(pow5(b5))
                .wrapping_shl(b2 as u32),
            width,
        );
        let s = signed(pow5(s5).wrapping_shl(s2 as u32), width);
        let mut m = signed(pow5(m5).wrapping_shl(m2 as u32), width);
        let ten = signed(s.wrapping_mul(10), width);
        let q = (b / s) as u8;
        b = signed((b % s).wrapping_mul(10), width);
        m = signed(m.wrapping_mul(10), width);
        low = b < m;
        high = signed(b.wrapping_add(m), width) > ten;
        if q == 0 && !high {
            exp -= 1;
        } else {
            digits.push(q);
        }
        if !(-3..8).contains(&exp) {
            low = false;
            high = false;
        }
        while !low && !high {
            let q = (b / s) as u8;
            b = signed((b % s).wrapping_mul(10), width);
            m = signed(m.wrapping_mul(10), width);
            if m > 0 {
                low = b < m;
                high = signed(b.wrapping_add(m), width) > ten;
            } else {
                low = true;
                high = true;
            }
            digits.push(q);
        }
        difference = signed(signed(b.wrapping_shl(1), width).wrapping_sub(ten), width).cmp(&0);
    } else {
        let s = Natural::powered(1, s5, s2);
        let mut b = Natural::powered(fraction, b5, b2);
        let mut m = Natural::powered(1, m5 + 1, m2 + 1);
        let ten = Natural::powered(1, s5 + 1, s2 + 1);
        let q = b.digit(&s);
        low = b.cmp(&m) == Ordering::Less;
        high = ten.cmp(&b.add(&m)) != Ordering::Greater;
        if q == 0 && !high {
            exp -= 1;
        } else {
            digits.push(q);
        }
        if !(-3..8).contains(&exp) {
            low = false;
            high = false;
        }
        while !low && !high {
            let q = b.digit(&s);
            m.mul(10);
            low = b.cmp(&m) == Ordering::Less;
            high = ten.cmp(&b.add(&m)) != Ordering::Greater;
            digits.push(q);
        }
        b.mul(2);
        difference = b.cmp(&ten);
    }
    exp += 1;
    if high
        && (!low
            || difference == Ordering::Greater
            || (difference == Ordering::Equal && digits[digits.len() - 1] & 1 != 0))
    {
        rounded(&mut digits, &mut exp);
    }
    render(negative, &digits, exp)
}

#[cfg(test)]
mod tests {
    use super::{format_binary32, format_binary64};
    #[test]
    fn binary32_java17_boundary_rendering() {
        for (bits, expected) in [
            (0, "0.0"),
            (0x80000000, "-0.0"),
            (1, "1.4E-45"),
            (0x00800000, "1.17549435E-38"),
            (0x7f7fffff, "3.4028235E38"),
            (0x3dcccccd, "0.1"),
            (0x7f800000, "Infinity"),
            (0xff800000, "-Infinity"),
            (0x7fc00000, "NaN"),
            (0x4b800000, "1.6777216E7"),
            (0x4b800001, "1.6777218E7"),
        ] {
            assert_eq!(
                format_binary32(f32::from_bits(bits)),
                expected,
                "{bits:08x}"
            );
        }
        assert_eq!(format_binary32("16777217".parse().unwrap()), "1.6777216E7");
        assert_eq!(format_binary32("16777219".parse().unwrap()), "1.677722E7");
    }
    #[test]
    fn binary64_java17_boundary_rendering() {
        for (bits, expected) in [
            (0, "0.0"),
            (1, "4.9E-324"),
            (2, "1.0E-323"),
            (0x8000000000000000, "-0.0"),
            (0x44b52d02c7e14af6, "9.999999999999999E22"),
            (0x0010000000000000, "2.2250738585072014E-308"),
            (0x7fefffffffffffff, "1.7976931348623157E308"),
            (0x7ff0000000000000, "Infinity"),
            (0xfff0000000000000, "-Infinity"),
            (0x7ff8000000000000, "NaN"),
        ] {
            assert_eq!(
                format_binary64(f64::from_bits(bits)),
                expected,
                "{bits:016x}"
            );
        }
    }
}
