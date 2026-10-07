//! Configuration-root rights shared by the native reader and writer.
//!
//! The root order is evidenced by the existing ERP UH role corpus. The
//! termination right's name/UUID is witnessed in native 8.3.27.2214 ITK
//! roles; its position after AnalyticsSystemClient and before SaveUserData
//! is also reported for native 8.3.27.2074 in ibcmd-rs issue #436.

pub const EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START: &str =
    "ExclusiveModeTerminationAtSessionStart";
pub const EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START_UUID: &str =
    "4df6d046-3bf8-4dda-991c-53ba664296a5";

/// The native order places the mode/termination presentation group before this right.
pub const CONFIGURATION_MODE_GROUP_ANCHOR: &str = "SaveUserData";

pub fn termination_right_uuid(name: &str) -> Option<&'static str> {
    (name == EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START)
        .then_some(EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START_UUID)
}

pub fn termination_right_name(uuid: &str) -> Option<&'static str> {
    (uuid == EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START_UUID)
        .then_some(EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START)
}

pub const CONFIGURATION_MODE_RIGHT_NAMES: [&str; 6] = [
    "MainWindowModeNormal",
    "MainWindowModeWorkplace",
    "MainWindowModeEmbeddedWorkplace",
    "MainWindowModeFullscreenWorkplace",
    "MainWindowModeKiosk",
    "AnalyticsSystemClient",
];

pub const CONFIGURATION_RIGHT_ORDER: [&str; 26] = [
    "Administration",
    "DataAdministration",
    "UpdateDataBaseConfiguration",
    "ExclusiveMode",
    "ActiveUsers",
    "EventLog",
    "ThinClient",
    "WebClient",
    "MobileClient",
    "ThickClient",
    "ExternalConnection",
    "Automation",
    "TechnicalSpecialistMode",
    "CollaborationSystemInfoBaseRegistration",
    "MainWindowModeNormal",
    "MainWindowModeWorkplace",
    "MainWindowModeEmbeddedWorkplace",
    "MainWindowModeFullscreenWorkplace",
    "MainWindowModeKiosk",
    "AnalyticsSystemClient",
    EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START,
    CONFIGURATION_MODE_GROUP_ANCHOR,
    "ConfigurationExtensionsAdministration",
    "InteractiveOpenExtDataProcessors",
    "InteractiveOpenExtReports",
    "Output",
];
