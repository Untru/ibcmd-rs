//! Stored codes of the command-interface family of configuration assets
//! and the names the platform exports them under: command placements, home
//! page work-area templates and the panel definitions the platform owns.
//! The exporter's readers in the root and the base-free writer read the
//! same tables, in opposite directions.

// From `src/mssql_dump/command_interface.rs`.
/// The panel definitions the platform owns, in the platform's own order.
///
/// Read off 8.3.27.2214 with nothing of ours in the input: the seed
/// `seeds/ada-panels` is the Web_Service tree plus an
/// `Ext/ClientApplicationInterface.xml` whose only `<panelDef/>` is a freshly
/// generated identifier no configuration on the stand carries, placed in one
/// `<top>` panel. Importing that tree and exporting the infobase back writes
/// six definitions: these five, in this order, and the seed's one behind them.
/// Not one of the five was supplied to the platform, so the list is the
/// platform naming its own table rather than a white list read off a corpus.
///
/// The companion seed that classifies the codes -- import seven definitions,
/// save, and read the stored record -- puts exactly these five under the code
/// `2` and everything else under the code `1`, which is what makes the table
/// usable as the code-`2` test.
pub const CLIENT_APPLICATION_STANDARD_PANEL_DEFS: [&str; 5] = [
    "b553047f-c9aa-4157-978d-448ecad24248",
    "13322b22-3960-4d68-93a6-fe2dd7f28ca3",
    "c933ac92-92cd-459d-81cc-e0c8a83ced99",
    "cbab57f2-a0f3-4f0a-89ea-4cb19570ab75",
    "b2735bd3-d822-4430-ba59-c9e869693b24",
];

pub fn client_application_panel_def_is_standard(id: &str) -> bool {
    CLIENT_APPLICATION_STANDARD_PANEL_DEFS
        .iter()
        .any(|standard| standard.eq_ignore_ascii_case(id))
}

/// Stored placement codes and the names the platform exports them under.
pub const COMMAND_INTERFACE_PLACEMENTS: [(&str, &str); 2] = [("0", "Auto"), ("1", "Manual")];

/// Stored work-area template codes and the names the platform exports them
/// under, read and written alike: 8.3.27.2214 stores OneColumn as 0,
/// TwoColumnsEqualWidth as 1 and TwoColumnsVariableWidth as 2 (fixtures
/// `home_page`).
pub const HOME_PAGE_WORK_AREA_TEMPLATES: [(&str, &str); 3] = [
    ("0", ONE_COLUMN_TEMPLATE),
    ("1", "TwoColumnsEqualWidth"),
    ("2", "TwoColumnsVariableWidth"),
];

pub const ONE_COLUMN_TEMPLATE: &str = "OneColumn";
