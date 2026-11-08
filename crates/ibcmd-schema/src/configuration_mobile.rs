//! Configuration mobile functionality names and physical IDs.
//!
//! These existing facts are shared by the Config reader, both root writers
//! and source XML admission. Permission IDs are a separate vocabulary.

pub const FUNCTIONALITIES: [(u32, &str); 38] = [
    (0, "Biometrics"),
    (1, "Location"),
    (2, "BackgroundLocation"),
    (3, "BluetoothPrinters"),
    (4, "WiFiPrinters"),
    (5, "Contacts"),
    (6, "Calendars"),
    (7, "PushNotifications"),
    (8, "LocalNotifications"),
    (9, "InAppPurchases"),
    (10, "PersonalComputerFileExchange"),
    (11, "Ads"),
    (12, "NumberDialing"),
    (13, "CallProcessing"),
    (14, "CallLog"),
    (15, "AutoSendSMS"),
    (16, "ReceiveSMS"),
    (17, "SMSLog"),
    (18, "Camera"),
    (19, "Microphone"),
    (20, "MusicLibrary"),
    (21, "PictureAndVideoLibraries"),
    (22, "AudioPlaybackAndVibration"),
    (23, "BackgroundAudioPlaybackAndVibration"),
    (24, "InstallPackages"),
    (25, "OSBackup"),
    (26, "ApplicationUsageStatistics"),
    (27, "BarcodeScanning"),
    (32, "BackgroundAudioRecording"),
    (33, "AllFilesAccess"),
    (34, "Videoconferences"),
    (35, "NFC"),
    (36, "DocumentScanning"),
    (37, "SpeechToText"),
    (38, "Geofences"),
    (39, "IncomingShareRequests"),
    (40, "AllIncomingShareRequestsTypesProcessing"),
    (41, "TextToSpeech"),
];

/// The complete named XML roster already emitted by the native reader.
/// 2.17 omits TextToSpeech; 2.20/2.21 include it even for older sparse rows.
/// Unversioned legacy source retains the current complete roster contract.
pub fn source_functionalities(version: Option<&str>) -> Option<&'static [(u32, &'static str)]> {
    match version {
        Some("2.17") => Some(&FUNCTIONALITIES[..FUNCTIONALITIES.len() - 1]),
        None | Some("2.20" | "2.21") => Some(&FUNCTIONALITIES),
        _ => None,
    }
}

/// Native message IDs are distinct from functionality IDs.
pub const PERMISSIONS: [(u32, &str); 7] = [
    (0, "Biometrics"),
    (12, "Camera"),
    (13, "Microphone"),
    (14, "MusicLibrary"),
    (15, "PictureAndVideoLibraries"),
    (16, "AudioPlaybackAndVibration"),
    (20, "PostNotifications"),
];

/// Distinct legacy capability IDs measured in the native message controls.
pub fn legacy_permission_id(permission: u32) -> Option<u32> {
    match permission {
        0 => Some(29),
        12 => Some(22),
        13 => Some(23),
        14 => Some(24),
        15 => Some(25),
        16 => Some(26),
        20 => Some(35),
        _ => None,
    }
}

/// Property-tuple field carrying legacy permission flags and descriptions.
pub const LEGACY_PERMISSION_TABLE_FIELD: usize = 51;

/// The counted pair table starts after its version and pair count.
pub fn functionality_tail_start(pair_count: usize) -> Option<usize> {
    pair_count.checked_add(2)
}
