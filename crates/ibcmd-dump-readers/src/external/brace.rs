//! Top-level fields of 1C brace lists (`{a,{b,c},"d"}`), for the few external
//! rows this adapter rewrites. Quotes are 1C strings with `""` escapes.

pub fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

// The fields of a brace list are the `ibcmd-1c-text` crate's; the adapter
// reaches them here, as before.
pub use ibcmd_1c_text::brace_list::fields;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_utf8_bom() {
        assert_eq!(strip_bom("\u{feff}{1}"), "{1}");
        assert_eq!(strip_bom("{1}"), "{1}");
    }
}
