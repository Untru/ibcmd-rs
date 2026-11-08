//! Information-register enum values and native effective-property rules.

pub const PERIODICITY: &[(&str, i64)] = &[
    ("Nonperiodical", 0),
    ("Year", 1),
    ("Quarter", 2),
    ("Month", 3),
    ("Day", 4),
    ("Second", 5),
    ("RecorderPosition", 6),
];
pub const WRITE_MODE: &[(&str, i64)] = &[("Independent", 0), ("RecorderSubordinate", 1)];

/// Native export disables period filtering when there is no period or the
/// recorder owns writing. Unknown decoded enum spellings are not inferred.
pub fn main_filter_on_period(stored: bool, periodicity: &str, write_mode: &str) -> Option<bool> {
    let periodicity = PERIODICITY.iter().find(|(name, _)| *name == periodicity)?.1;
    let write_mode = WRITE_MODE.iter().find(|(name, _)| *name == write_mode)?.1;
    Some(stored && periodicity != 0 && write_mode == 0)
}

#[cfg(test)]
mod tests {
    use super::main_filter_on_period;

    #[test]
    fn filtering_needs_a_period_and_independent_writing() {
        for stored in [false, true] {
            assert_eq!(
                main_filter_on_period(stored, "Nonperiodical", "Independent"),
                Some(false)
            );
            assert_eq!(
                main_filter_on_period(stored, "Nonperiodical", "RecorderSubordinate"),
                Some(false)
            );
            for period in [
                "Year",
                "Quarter",
                "Month",
                "Day",
                "Second",
                "RecorderPosition",
            ] {
                assert_eq!(
                    main_filter_on_period(stored, period, "Independent"),
                    Some(stored)
                );
                assert_eq!(
                    main_filter_on_period(stored, period, "RecorderSubordinate"),
                    Some(false)
                );
            }
        }
        assert_eq!(main_filter_on_period(true, "unknown", "Independent"), None);
        assert_eq!(main_filter_on_period(true, "Day", "unknown"), None);
    }
}
