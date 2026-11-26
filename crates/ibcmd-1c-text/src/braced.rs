//! Splitting a 1C braced value (`{field,field,...}`) into its top-level
//! fields, finding where a value ends, and the per-thread jump tables that
//! let both skip over the nested values of a registered text.
//!
//! There is one jump table per thread (`BRACE_JUMPS` below). The root
//! crate's readers register their texts with [`register_brace_jumps`]; every
//! splitter and scanner of this module, and [`registered_header_marker`],
//! read that table.

use std::collections::HashMap;

pub fn split_1c_braced_fields(text: &str, start: usize) -> Option<Vec<&str>> {
    split_1c_braced_fields_bounded(text, start, usize::MAX)
}

/// Top-level 1C fields with byte offsets into the original source text.
/// Offsets delimit the trimmed field value, so provenance points at the exact
/// raw token rather than a reconstructed/value-matched occurrence.
#[cfg(test)]
fn split_1c_braced_fields_with_spans(
    text: &str,
    start: usize,
) -> Option<Vec<(&str, usize, usize)>> {
    let mut fields = Vec::new();
    for_each_1c_braced_field(text, start, |value, value_start| {
        let trimmed = value.trim();
        let offset = value.find(trimmed).unwrap_or(0);
        fields.push((
            trimmed,
            start + value_start + offset,
            start + value_start + offset + trimmed.len(),
        ));
        true
    })?;
    Some(fields)
}

pub fn split_1c_braced_fields_bounded(
    text: &str,
    start: usize,
    max_fields: usize,
) -> Option<Vec<&str>> {
    if let Some(fields) = split_1c_braced_fields_jumped(&text[start..], max_fields) {
        return fields;
    }
    let mut fields = Vec::new();
    walk_1c_braced_fields(&text[start..], |value, _| {
        if fields.len() >= max_fields {
            return false;
        }
        fields.push(value.trim());
        true
    })?;
    Some(fields)
}

/// Walks the braced value at `start` once, handing every top-level field --
/// untrimmed, with its offset from `start` -- to `field`, which may stop the
/// walk by answering `false`. `None` when there is no complete braced value at
/// `start` or the walk was stopped.
///
/// Every delimiter is ASCII (`{`, `}`, `,`, `"`), and no byte of a multi-byte
/// UTF-8 sequence is, so the bytes are read directly: the answer is the one a
/// walk over the characters gives, in one pass instead of one to find the end
/// and another to split.
pub fn for_each_1c_braced_field<'t>(
    text: &'t str,
    start: usize,
    mut field: impl FnMut(&'t str, usize) -> bool,
) -> Option<()> {
    let rest = &text[start..];
    if let Some(walked) = for_each_1c_braced_field_jumped(rest, &mut field) {
        return walked;
    }
    walk_1c_braced_fields(rest, field)
}

/// [`for_each_1c_braced_field`] without the registered tables: the byte walk
/// over the value at `rest[0]`.
fn walk_1c_braced_fields<'t>(
    rest: &'t str,
    mut field: impl FnMut(&'t str, usize) -> bool,
) -> Option<()> {
    let bytes = rest.as_bytes();
    if bytes.first() != Some(&b'{') {
        return None;
    }
    let mut depth = 0usize;
    let mut field_start = 1usize;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index = skip_1c_string_body(bytes, index + 1)?;
                continue;
            }
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return field(&rest[field_start..index], field_start).then_some(());
                }
            }
            b',' if depth == 1 => {
                if !field(&rest[field_start..index], field_start) {
                    return None;
                }
                field_start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    None
}

/// The offset just past the quote that closes a 1C string whose body starts
/// at `from`; `""` inside the body is an escaped quote. `None` when the
/// string never closes.
fn skip_1c_string_body(bytes: &[u8], mut from: usize) -> Option<usize> {
    loop {
        let quote = from + bytes.get(from..)?.iter().position(|&byte| byte == b'"')?;
        if bytes.get(quote + 1) == Some(&b'"') {
            from = quote + 2;
            continue;
        }
        return Some(quote + 1);
    }
}

// ---------------------------------------------------------------------------
// Brace jumps: a form body's nesting, read once.
//
// The form readers split every braced value they meet, and a value's split
// walks all of it -- so each byte of a body was read once per enclosing value,
// by every pass over the item tree. A registered text records, in one walk,
// where each of its values and strings ends; splitting a value inside it then
// reads only the value's own top level and jumps over what is nested.
//
// A value read from a registered text starts outside any string, exactly as a
// walk that starts at that value does, so the two walks agree on every string
// and every value inside it: the answer is the one the plain walk gives. What
// the table does not know -- an offset that is no value start of the text, a
// value that runs past the slice handed in, a text not registered -- is walked
// as before.

struct BraceJumps {
    base: usize,
    len: usize,
    /// For each offset that opens a value (`{`) or a string (`"`) outside
    /// any string, the offset just past its end; 0 elsewhere.
    ends: Vec<u32>,
    /// Where each uuid's header marker first stands in the text, built on
    /// first use (see [`registered_header_marker`]).
    header_markers: std::cell::OnceCell<HeaderMarkers>,
}

/// The first offset of `{1,0,<uuid>},` and of `{0,0,<uuid>},` in a text, by
/// the 36 characters between (whatever they are).
#[derive(Default)]
struct HeaderMarkers {
    modern: HashMap<String, usize>,
    legacy: HashMap<String, usize>,
}

fn header_markers(text: &str) -> HeaderMarkers {
    let mut markers = HeaderMarkers::default();
    for (prefix, map) in [
        ("{1,0,", &mut markers.modern),
        ("{0,0,", &mut markers.legacy),
    ] {
        // The prefix cannot overlap itself, so every occurrence is found.
        for (start, _) in text.match_indices(prefix) {
            let uuid_start = start + prefix.len();
            let Some(uuid) = text.get(uuid_start..uuid_start + 36) else {
                continue;
            };
            if text.get(uuid_start + 36..uuid_start + 38) == Some("},") {
                map.entry(uuid.to_string()).or_insert(start);
            }
        }
    }
    markers
}

/// For a registered text (the whole text, not a part of it): the offset just
/// past `{1,0,<uuid>},`'s first occurrence, else past `{0,0,<uuid>},`'s --
/// what searching the text for each marker in turn finds. `None` when no
/// table knows the text; `Some(None)` when it holds neither marker.
pub fn registered_header_marker(text: &str, uuid: &str) -> Option<Option<usize>> {
    if uuid.len() != 36 {
        return None;
    }
    let base = text.as_ptr() as usize;
    BRACE_JUMPS.with(|tables| {
        let tables = tables.borrow();
        let table = tables
            .iter()
            .find(|table| table.base == base && table.len == text.len())?;
        let markers = table.header_markers.get_or_init(|| header_markers(text));
        let marker_len = "{1,0,".len() + 36 + "},".len();
        Some(
            markers
                .modern
                .get(uuid)
                .or_else(|| markers.legacy.get(uuid))
                .map(|start| start + marker_len),
        )
    })
}

thread_local! {
    static BRACE_JUMPS: std::cell::RefCell<Vec<BraceJumps>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Registers texts for the jump table while the guard lives. Registering
/// holds a shared borrow of each text, so none of them can change or go away
/// while its table is in use.
pub struct BraceJumpsGuard<'t> {
    registered: usize,
    _texts: std::marker::PhantomData<&'t str>,
}

pub fn register_brace_jumps<'t>(texts: impl IntoIterator<Item = &'t str>) -> BraceJumpsGuard<'t> {
    let mut registered = 0;
    BRACE_JUMPS.with(|tables| {
        let mut tables = tables.borrow_mut();
        for text in texts {
            if text.len() < BRACE_JUMPS_MIN_TEXT || text.len() >= u32::MAX as usize {
                continue;
            }
            tables.push(build_brace_jumps(text));
            registered += 1;
        }
    });
    BraceJumpsGuard {
        registered,
        _texts: std::marker::PhantomData,
    }
}

impl Drop for BraceJumpsGuard<'_> {
    fn drop(&mut self) {
        BRACE_JUMPS.with(|tables| {
            let mut tables = tables.borrow_mut();
            let keep = tables.len().saturating_sub(self.registered);
            tables.truncate(keep);
        });
    }
}

/// Texts shorter than this are walked directly: their table would cost more
/// than it saves.
pub const BRACE_JUMPS_MIN_TEXT: usize = 4096;

fn build_brace_jumps(text: &str) -> BraceJumps {
    let bytes = text.as_bytes();
    let mut ends = vec![0u32; bytes.len()];
    let mut open = Vec::<u32>::new();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => match skip_1c_string_body(bytes, index + 1) {
                Some(end) => {
                    ends[index] = end as u32;
                    index = end;
                    continue;
                }
                // A string that never closes: nothing after it is outside one.
                None => break,
            },
            b'{' => open.push(index as u32),
            b'}' => {
                if let Some(start) = open.pop() {
                    ends[start as usize] = (index + 1) as u32;
                }
            }
            _ => {}
        }
        index += 1;
    }
    BraceJumps {
        base: text.as_ptr() as usize,
        len: bytes.len(),
        ends,
        header_markers: std::cell::OnceCell::new(),
    }
}

/// The registered table and the value it knows at `rest[0]`: (table, the
/// value's offset in it, the value's length), when the value lies within
/// `rest`.
fn brace_jump_value<'a>(
    tables: &'a [BraceJumps],
    rest: &str,
) -> Option<(&'a BraceJumps, usize, usize)> {
    if rest.as_bytes().first() != Some(&b'{') {
        return None;
    }
    let at = rest.as_ptr() as usize;
    let table = tables
        .iter()
        .find(|table| at >= table.base && at < table.base + table.len)?;
    let offset = at - table.base;
    let end = table.ends[offset] as usize;
    // `then`, not `then_some`: with `end == 0` the subtraction must not run.
    (end != 0 && end - offset <= rest.len()).then(|| (table, offset, end - offset))
}

/// [`split_1c_braced_fields_bounded`] through the registered table: the
/// value's top level is walked once to count its fields, and once more to
/// collect them into a vector of that size. `None` when no table knows the
/// value; `Some(None)` is the split's own `None` (more than `max_fields`).
fn split_1c_braced_fields_jumped(rest: &str, max_fields: usize) -> Option<Option<Vec<&str>>> {
    BRACE_JUMPS.with(|tables| {
        let tables = tables.borrow();
        let (table, offset, len) = brace_jump_value(&tables, rest)?;
        let value = &rest[..len];
        let bytes = value.as_bytes();
        let last = len - 1;
        // Every value and string inside a closed value is closed too, so each
        // has its end; checked while counting, before anything is collected.
        let mut count = 1usize;
        let mut index = 1usize;
        while index < last {
            match bytes[index] {
                b'"' | b'{' => {
                    let jump = table.ends[offset + index] as usize;
                    if jump <= offset + index || jump > offset + len {
                        return None;
                    }
                    index = jump - offset;
                    continue;
                }
                b',' => count += 1,
                _ => {}
            }
            index += 1;
        }
        if count > max_fields {
            return Some(None);
        }
        let mut fields = Vec::with_capacity(count);
        let mut field_start = 1usize;
        let mut index = 1usize;
        while index < last {
            match bytes[index] {
                b'"' | b'{' => {
                    index = table.ends[offset + index] as usize - offset;
                    continue;
                }
                b',' => {
                    fields.push(value[field_start..index].trim());
                    field_start = index + 1;
                }
                _ => {}
            }
            index += 1;
        }
        fields.push(value[field_start..last].trim());
        Some(Some(fields))
    })
}

/// The fields of the value at `rest[0]` through the registered table, or
/// `None` when no table knows that value within `rest` (walk it instead).
/// `Some(None)` is the walk's own `None`: `field` stopped it.
fn for_each_1c_braced_field_jumped<'t>(
    rest: &'t str,
    field: &mut impl FnMut(&'t str, usize) -> bool,
) -> Option<Option<()>> {
    let at = rest.as_ptr() as usize;
    BRACE_JUMPS.with(|tables| {
        let tables = tables.borrow();
        let table = tables
            .iter()
            .find(|table| at >= table.base && at < table.base + table.len)?;
        let offset = at - table.base;
        let end = table.ends[offset] as usize;
        if end == 0 || end - offset > rest.len() {
            return None;
        }
        let value = &rest[..end - offset];
        let bytes = value.as_bytes();
        if bytes.first() != Some(&b'{') {
            return None;
        }
        let last = bytes.len() - 1;
        // Every value and string inside a closed value is closed too, so each
        // has its end; the walk below relies on it, and it is checked before
        // any field is handed out.
        let mut index = 1usize;
        while index < last {
            match bytes[index] {
                b'"' | b'{' => {
                    let jump = table.ends[offset + index] as usize;
                    if jump <= offset + index || jump > end {
                        return None;
                    }
                    index = jump - offset;
                }
                _ => index += 1,
            }
        }
        let mut field_start = 1usize;
        let mut index = 1usize;
        while index < last {
            match bytes[index] {
                b'"' | b'{' => {
                    index = table.ends[offset + index] as usize - offset;
                    continue;
                }
                b',' => {
                    if !field(&value[field_start..index], field_start) {
                        return Some(None);
                    }
                    field_start = index + 1;
                }
                _ => {}
            }
            index += 1;
        }
        Some(field(&value[field_start..last], field_start).then_some(()))
    })
}

/// The end of the value at `rest[0]` through the registered table, relative
/// to `rest`; `None` when no table knows that value within `rest`.
fn brace_jump_end(rest: &str) -> Option<usize> {
    if rest.as_bytes().first() != Some(&b'{') {
        return None;
    }
    let at = rest.as_ptr() as usize;
    BRACE_JUMPS.with(|tables| {
        let tables = tables.borrow();
        let table = tables
            .iter()
            .find(|table| at >= table.base && at < table.base + table.len)?;
        let offset = at - table.base;
        let end = table.ends[offset] as usize;
        // `then`, not `then_some`: with `end == 0` the subtraction must not run.
        (end != 0 && end - offset <= rest.len()).then(|| end - offset)
    })
}

pub fn scan_1c_braced_value(text: &str, start: usize) -> Option<usize> {
    if let Some(end) = brace_jump_end(&text[start..]) {
        return Some(start + end);
    }
    let bytes = &text.as_bytes()[start..];
    if bytes.first() != Some(&b'{') {
        return None;
    }
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index = skip_1c_string_body(bytes, index + 1)?;
                continue;
            }
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(start + index + 1);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

pub fn skip_ascii_ws_at(text: &str, mut offset: usize) -> usize {
    while let Some(byte) = text.as_bytes().get(offset)
        && byte.is_ascii_whitespace()
    {
        offset += 1;
    }
    offset
}

pub fn expect_comma_at(text: &str, offset: usize) -> Option<usize> {
    let offset = skip_ascii_ws_at(text, offset);
    if text.as_bytes().get(offset) == Some(&b',') {
        Some(offset + 1)
    } else {
        None
    }
}

pub fn split_1c_braced_sequence(text: &str) -> Option<Vec<&str>> {
    let text = text.trim();
    if !text.starts_with('{') || !text.ends_with('}') {
        return None;
    }
    let mut fields = Vec::new();
    let mut offset = 1usize;
    let end = text.len().checked_sub(1)?;
    while offset < end {
        offset = skip_ascii_ws_at(text, offset);
        while text[offset..].starts_with(',') {
            offset += 1;
            offset = skip_ascii_ws_at(text, offset);
        }
        if offset >= end {
            break;
        }
        if !text[offset..].starts_with('{') {
            return None;
        }
        let item_end = scan_1c_braced_value(text, offset)?;
        fields.push(text[offset..item_end].trim());
        offset = item_end;
    }
    Some(fields)
}

#[cfg(test)]
mod brace_walk_tests {
    use super::*;

    /// The character walk the byte walk and the jump table replaced.
    fn reference_scan(text: &str, start: usize) -> Option<usize> {
        if text[start..].chars().next()? != '{' {
            return None;
        }
        let mut depth = 0usize;
        let mut in_string = false;
        let mut chars = text[start..].char_indices().peekable();
        while let Some((relative, ch)) = chars.next() {
            if in_string {
                if ch == '"' {
                    if let Some((_, next)) = chars.peek()
                        && *next == '"'
                    {
                        let _ = chars.next();
                        continue;
                    }
                    in_string = false;
                }
                continue;
            }
            match ch {
                '"' => in_string = true,
                '{' => depth += 1,
                '}' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(start + relative + ch.len_utf8());
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// The character walk's fields: (trimmed value, start, end).
    fn reference_split(
        text: &str,
        start: usize,
        max_fields: usize,
    ) -> Option<Vec<(&str, usize, usize)>> {
        let end = reference_scan(text, start)?;
        let inner_start = start + text[start..].chars().next()?.len_utf8();
        let inner_end = end.checked_sub(1)?;
        let inner = &text[inner_start..inner_end];
        let mut fields = Vec::new();
        let mut field_start = 0usize;
        let mut depth = 0usize;
        let mut in_string = false;
        let mut chars = inner.char_indices().peekable();
        let field = |value: &'_ str, at: usize| {
            let trimmed = value.trim();
            let offset = value.find(trimmed).unwrap_or(0);
            (at + offset, at + offset + trimmed.len())
        };
        while let Some((index, ch)) = chars.next() {
            if in_string {
                if ch == '"' {
                    if let Some((_, next)) = chars.peek()
                        && *next == '"'
                    {
                        let _ = chars.next();
                        continue;
                    }
                    in_string = false;
                }
                continue;
            }
            match ch {
                '"' => in_string = true,
                '{' => depth += 1,
                '}' => depth = depth.checked_sub(1)?,
                ',' if depth == 0 => {
                    if fields.len() >= max_fields {
                        return None;
                    }
                    let value = &inner[field_start..index];
                    let (from, to) = field(value, inner_start + field_start);
                    fields.push((value.trim(), from, to));
                    field_start = index + ch.len_utf8();
                }
                _ => {}
            }
        }
        if fields.len() >= max_fields {
            return None;
        }
        let value = &inner[field_start..];
        let (from, to) = field(value, inner_start + field_start);
        fields.push((value.trim(), from, to));
        Some(fields)
    }

    fn values(fields: Option<Vec<(&str, usize, usize)>>) -> Option<Vec<&str>> {
        fields.map(|fields| fields.into_iter().map(|(value, _, _)| value).collect())
    }

    struct Random(u64);

    impl Random {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    const PIECES: &[&str] = &[
        "{", "}", ",", "\"", " ", "a", "1", "я", "Ж", "\r\n", "{\"", "\",", "\"\"",
    ];

    fn random_text(random: &mut Random, len: usize) -> String {
        (0..len)
            .map(|_| PIECES[random.below(PIECES.len() as u64) as usize])
            .collect()
    }

    /// A well-formed value: nested lists, strings holding braces, commas and
    /// doubled quotes, Cyrillic scalars.
    fn random_value(random: &mut Random, depth: usize, out: &mut String) {
        match random.below(if depth > 5 { 3 } else { 5 }) {
            0 => out.push_str(["0", "1", "-1", "12345", "Ж", "abc"][random.below(6) as usize]),
            1 => {
                out.push('"');
                for _ in 0..random.below(6) {
                    out.push_str(["{", "}", ",", "\"\"", "я", " ", "x"][random.below(7) as usize]);
                }
                out.push('"');
            }
            2 => out.push_str(" \r\n"),
            _ => {
                out.push('{');
                let count = random.below(5);
                for index in 0..=count {
                    if index > 0 {
                        out.push(',');
                        if random.below(3) == 0 {
                            out.push_str("\r\n");
                        }
                    }
                    random_value(random, depth + 1, out);
                }
                out.push('}');
            }
        }
    }

    fn check_starts(text: &str, step: usize) {
        for start in (0..=text.len())
            .step_by(step)
            .filter(|start| text.is_char_boundary(*start))
        {
            assert_eq!(
                scan_1c_braced_value(text, start),
                reference_scan(text, start),
                "scan at {start} of {text:?}"
            );
            assert_eq!(
                split_1c_braced_fields_with_spans(text, start),
                reference_split(text, start, usize::MAX),
                "spans at {start} of {text:?}"
            );
            for max_fields in [0, 1, 2, 3, usize::MAX] {
                assert_eq!(
                    split_1c_braced_fields_bounded(text, start, max_fields),
                    values(reference_split(text, start, max_fields)),
                    "split at {start} ({max_fields}) of {text:?}"
                );
            }
        }
    }

    #[test]
    fn the_byte_walk_answers_as_the_character_walk() {
        let mut random = Random(0x9e37_79b9_7f4a_7c15);
        for _ in 0..3000 {
            let len = random.below(40) as usize;
            check_starts(&random_text(&mut random, len), 1);
        }
        for _ in 0..300 {
            let mut text = String::new();
            random_value(&mut random, 0, &mut text);
            check_starts(&text, 1);
        }
    }

    #[test]
    fn a_registered_text_answers_as_the_walk() {
        let mut random = Random(0x0123_4567_89ab_cdef);
        for round in 0..9 {
            let mut text = String::new();
            while text.len() < BRACE_JUMPS_MIN_TEXT + 500 {
                if round % 3 == 2 {
                    // Not well formed: stray braces, open strings.
                    text.push_str(&random_text(&mut random, 50));
                } else {
                    random_value(&mut random, 0, &mut text);
                    text.push(',');
                }
            }
            let text = if round % 3 == 0 {
                format!("{{{text}0}}")
            } else {
                text
            };
            let jumps = register_brace_jumps([text.as_str()]);
            assert_eq!(jumps.registered, 1);
            check_starts(&text, 1);
            // A slice cut inside a value is not answered from the table.
            let mut half = text.len() / 2;
            while !text.is_char_boundary(half) {
                half -= 1;
            }
            check_starts(&text[..half], 3);
            check_starts(&text[half..], 3);
        }
    }
}
