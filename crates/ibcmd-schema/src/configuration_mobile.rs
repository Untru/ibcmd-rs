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
