//! Трансляция режима совместимости между Designer-кодировкой (`Version8_3_20`) и
//! каноном (EDT-dotted `8.3.20`) — обратимо и только для распознанного версионного шаблона.

/// Транслировать Designer-кодировку режима совместимости в КАНОН (EDT-dotted). Designer несёт
/// `Version8_3_20`; канон — `8.3.20`. Правило: снять префикс `Version` и заменить `_`→`.`, но
/// ТОЛЬКО если остаток версионный (`\d+(_\d+)*`). Не-версионное / уже-dotted значение остаётся
/// БЕЗ изменений — определённое обратимое преобразование применяется лишь к РАСПОЗНАННОМУ
/// версионному шаблону (не угадываем структуру произвольного значения).
pub(crate) fn compat_designer_to_canonical(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("Version") {
        if is_dotless_version(rest) {
            return rest.replace('_', ".");
        }
    }
    s.to_string()
}

/// Обратное к [`compat_designer_to_canonical`]: канон (`8.3.20`) → Designer (`Version8_3_20`).
/// Версионное значение (`\d+(\.\d+)*`) → префикс `Version` + `.`→`_`; иначе без изменений.
pub(crate) fn compat_canonical_to_designer(s: &str) -> String {
    if is_dotted_version(s) {
        return format!("Version{}", s.replace('.', "_"));
    }
    s.to_string()
}

/// `8_3_20`-подобная строка: непустые группы цифр, разделённые `_`.
fn is_dotless_version(s: &str) -> bool {
    !s.is_empty()
        && s.split('_')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// `8.3.20`-подобная строка: непустые группы цифр, разделённые `.`.
fn is_dotted_version(s: &str) -> bool {
    !s.is_empty()
        && s.split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(any())]
mod compat_mode_tests {
    use super::{compat_canonical_to_designer, compat_designer_to_canonical};

    #[test]
    fn designer_prefixed_round_trips_to_canonical_dotted() {
        assert_eq!(compat_designer_to_canonical("Version8_3_20"), "8.3.20");
        assert_eq!(compat_canonical_to_designer("8.3.20"), "Version8_3_20");
        // Inverse over the witnessed values.
        for (des, canon) in [
            ("Version8_3_20", "8.3.20"),
            ("Version8_5_1", "8.5.1"),
            ("Version8_1", "8.1"),
        ] {
            assert_eq!(compat_designer_to_canonical(des), canon);
            assert_eq!(compat_canonical_to_designer(canon), des);
        }
    }

    #[test]
    fn non_version_values_pass_through_both_ways() {
        // A non-versioned sentinel (defensive) is left untouched — no mangling.
        assert_eq!(compat_designer_to_canonical("DontUse"), "DontUse");
        assert_eq!(compat_canonical_to_designer("DontUse"), "DontUse");
        // An already-dotted value read from a non-Version source stays as-is.
        assert_eq!(compat_designer_to_canonical("8.3.20"), "8.3.20");
    }
}
