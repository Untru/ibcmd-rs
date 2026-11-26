//! Standard attributes the native records name by negative markers.

pub const CHART_OF_ACCOUNTS_STANDARD_ATTRIBUTE_DEFINITIONS: &[(&str, &str)] = &[
    ("-28", "PredefinedDataName"),
    ("-17", "Order"),
    ("-11", "OffBalance"),
    ("-10", "Type"),
    ("-8", "Description"),
    ("-7", "Code"),
    ("-6", "Parent"),
    ("-5", "Predefined"),
    ("-4", "DeletionMark"),
    ("-2", "Ref"),
];
/// The standard tabular sections a `ChartOfAccounts` carries, each with the
/// standard attributes it declares, keyed by the marker the native record
/// names them by.  The metadata compiler reads this table to write
/// `<xr:StandardTabularSection name="ExtDimensionTypes">` and its four
/// `<xr:StandardAttribute>` children, and a form's bound slot names the very
/// same members by the very same markers.
pub const CHART_OF_ACCOUNTS_STANDARD_TABULAR_SECTION_DEFINITIONS:
    &[ChartStandardTabularSectionDefinition] = &[(
    "-12",
    "ExtDimensionTypes",
    &[
        ("-15", "TurnoversOnly"),
        ("-14", "Predefined"),
        ("-13", "ExtDimensionType"),
        ("-12", "LineNumber"),
    ],
)];

pub const ACCUMULATION_REGISTER_BALANCE_STANDARD_ATTRIBUTES: [(&str, &str); 5] = [
    ("-9", "RecordType"),
    ("-5", "Active"),
    ("-4", "LineNumber"),
    ("-3", "Recorder"),
    ("-2", "Period"),
];
pub const ACCUMULATION_REGISTER_TURNOVERS_STANDARD_ATTRIBUTES: [(&str, &str); 4] = [
    ("-5", "Active"),
    ("-4", "LineNumber"),
    ("-3", "Recorder"),
    ("-2", "Period"),
];

/// The standard attribute an accumulation register's record set spells for a
/// marker a form binding names it by.
///
/// The turnovers table is a proper subset of the balance one and the two agree
/// on every marker they share, so one lookup answers both kinds: a turnovers
/// register simply never carries `-9`, the only marker the balance kind adds.
/// That is what lets this read the marker without first proving which kind the
/// register is.
pub fn accumulation_register_record_set_standard_attribute_name(
    marker: &str,
) -> Option<&'static str> {
    debug_assert!(
        ACCUMULATION_REGISTER_TURNOVERS_STANDARD_ATTRIBUTES
            .iter()
            .all(|entry| ACCUMULATION_REGISTER_BALANCE_STANDARD_ATTRIBUTES.contains(entry))
    );
    ACCUMULATION_REGISTER_BALANCE_STANDARD_ATTRIBUTES
        .iter()
        .find_map(|(candidate, name)| (*candidate == marker).then_some(*name))
}

/// The `ChartOfAccounts` standard-attribute table, as one slice, so a caller
/// that needs the whole family reads the same rows the name lookup does.
pub const fn chart_of_accounts_standard_attribute_definitions()
-> &'static [(&'static str, &'static str)] {
    CHART_OF_ACCOUNTS_STANDARD_ATTRIBUTE_DEFINITIONS
}

/// The `ChartOfAccounts` standard tabular section a bound slot names by
/// `marker`, with the standard attributes that section declares.
pub fn chart_of_accounts_standard_tabular_section(
    marker: &str,
) -> Option<&'static ChartStandardTabularSectionDefinition> {
    CHART_OF_ACCOUNTS_STANDARD_TABULAR_SECTION_DEFINITIONS
        .iter()
        .find(|(candidate, _, _)| *candidate == marker)
}

/// The sole standard tabular section a `ChartOfAccounts` declares — the one an
/// `ExtDimensionAccountingFlag` is a column of.  Reading it from the same table
/// the metadata compiler reads keeps the two spellings one fact; were the
/// family ever to declare a second section, the flag's owning section would no
/// longer follow from the family alone and this refuses rather than guesses.
pub fn chart_of_accounts_sole_standard_tabular_section_name() -> Option<&'static str> {
    match CHART_OF_ACCOUNTS_STANDARD_TABULAR_SECTION_DEFINITIONS {
        [(_, name, _)] => Some(name),
        _ => None,
    }
}

pub type ChartStandardTabularSectionDefinition = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

pub const DOCUMENT_STANDARD_ATTRIBUTES: [(&str, &str); 5] = [
    ("-7", "Posted"),
    ("-5", "Ref"),
    ("-4", "DeletionMark"),
    ("-3", "Date"),
    ("-2", "Number"),
];
