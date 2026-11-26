//! The file-name segment the source tree writes for a name: a character no
//! file system takes (a control character, or one of `<>:"/\|?*`) becomes
//! `_`, and a name left blank becomes `Unnamed`.
//!
//! It moved here verbatim, but for visibility, from the root's
//! `mssql_dump`, which reaches it by its old path.

pub fn sanitize_source_path_segment(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            output.push('_');
        } else {
            output.push(ch);
        }
    }
    if output.trim().is_empty() {
        "Unnamed".to_string()
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_and_control_characters_become_underscores() {
        assert_eq!(
            sanitize_source_path_segment("a<b>c:d\"e/f\\g|h?i*j\tk\u{7f}l"),
            "a_b_c_d_e_f_g_h_i_j_k_l"
        );
        assert_eq!(
            sanitize_source_path_segment("Справочник.Имя 1"),
            "Справочник.Имя 1"
        );
        assert_eq!(sanitize_source_path_segment(""), "Unnamed");
        assert_eq!(sanitize_source_path_segment(" \u{3000} "), "Unnamed");
        assert_eq!(sanitize_source_path_segment("\n"), "_");
    }
}
