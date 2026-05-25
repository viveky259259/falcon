//! Apple-SDK API → Info.plist key dictionary. Stable Apple territory (changes
//! rarely), so we ship this in source rather than maintaining a per-plugin map.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppleApi {
    /// Substring Falcon greps for inside the plugin's `ios/` / `darwin/` sources.
    pub api: &'static str,
    /// The Info.plist key the API requires. Empty string means "no specific key
    /// needed" (capability-only API; we still record the hit for telemetry).
    pub info_plist_key: &'static str,
    /// The Apple framework (informational; surfaces in the report).
    pub framework: &'static str,
}

pub const APPLE_API_MAP: &[AppleApi] = &[
    AppleApi { api: "requestWhenInUseAuthorization", info_plist_key: "NSLocationWhenInUseUsageDescription", framework: "CoreLocation" },
    AppleApi { api: "requestAlwaysAuthorization", info_plist_key: "NSLocationAlwaysAndWhenInUseUsageDescription", framework: "CoreLocation" },
    AppleApi { api: "AVCaptureDevice.requestAccess", info_plist_key: "NSCameraUsageDescription", framework: "AVFoundation" },
    AppleApi { api: "AVAudioSession.requestRecordPermission", info_plist_key: "NSMicrophoneUsageDescription", framework: "AVFoundation" },
    AppleApi { api: "PHPhotoLibrary.requestAuthorization", info_plist_key: "NSPhotoLibraryUsageDescription", framework: "Photos" },
    AppleApi { api: "PHAsset.creationRequestForAssetFromImage", info_plist_key: "NSPhotoLibraryAddUsageDescription", framework: "Photos" },
    AppleApi { api: "CNContactStore", info_plist_key: "NSContactsUsageDescription", framework: "Contacts" },
    AppleApi { api: "EKEventStore", info_plist_key: "NSCalendarsUsageDescription", framework: "EventKit" },
    AppleApi { api: "EKReminder", info_plist_key: "NSRemindersUsageDescription", framework: "EventKit" },
    AppleApi { api: "CMMotionManager", info_plist_key: "NSMotionUsageDescription", framework: "CoreMotion" },
    AppleApi { api: "HKHealthStore", info_plist_key: "NSHealthShareUsageDescription", framework: "HealthKit" },
    AppleApi { api: "CBCentralManager", info_plist_key: "NSBluetoothAlwaysUsageDescription", framework: "CoreBluetooth" },
    AppleApi { api: "MFMessageComposeViewController", info_plist_key: "", framework: "MessageUI" },
    AppleApi { api: "SFSpeechRecognizer.requestAuthorization", info_plist_key: "NSSpeechRecognitionUsageDescription", framework: "Speech" },
    AppleApi { api: "LAContext.canEvaluatePolicy", info_plist_key: "NSFaceIDUsageDescription", framework: "LocalAuthentication" },
    AppleApi { api: "MPMediaLibrary", info_plist_key: "NSAppleMusicUsageDescription", framework: "MediaPlayer" },
    AppleApi { api: "HMHomeManager", info_plist_key: "NSHomeKitUsageDescription", framework: "HomeKit" },
    AppleApi { api: "NEHotspotConfigurationManager", info_plist_key: "", framework: "NetworkExtension" },
    AppleApi { api: "UNUserNotificationCenter.requestAuthorization", info_plist_key: "", framework: "UserNotifications" },
    AppleApi { api: "CBPeripheralManager", info_plist_key: "NSBluetoothPeripheralUsageDescription", framework: "CoreBluetooth" },
    AppleApi { api: "CTCellularData", info_plist_key: "", framework: "CoreTelephony" },
    AppleApi { api: "INPreferences.requestSiriAuthorization", info_plist_key: "NSSiriUsageDescription", framework: "Intents" },
];

pub fn find_apis_in_source(source_text: &str) -> Vec<&'static AppleApi> {
    APPLE_API_MAP
        .iter()
        .filter(|entry| !entry.api.is_empty() && source_text.contains(entry.api))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_has_22_entries() {
        assert_eq!(APPLE_API_MAP.len(), 22);
    }

    #[test]
    fn no_duplicate_api_substrings() {
        let mut seen = std::collections::HashSet::new();
        for entry in APPLE_API_MAP {
            assert!(seen.insert(entry.api), "duplicate API substring: {}", entry.api);
        }
    }

    #[test]
    fn find_apis_finds_location_when_use_authorization() {
        let src = "[CLLocationManager.shared requestWhenInUseAuthorization];";
        let hits = find_apis_in_source(src);
        assert!(hits
            .iter()
            .any(|e| e.info_plist_key == "NSLocationWhenInUseUsageDescription"));
    }

    #[test]
    fn find_apis_finds_camera_when_request_access() {
        let src = "AVCaptureDevice.requestAccess(for: .video) { granted in }";
        let hits = find_apis_in_source(src);
        assert!(hits.iter().any(|e| e.info_plist_key == "NSCameraUsageDescription"));
    }

    #[test]
    fn find_apis_finds_multiple_in_same_source() {
        let src = r#"
import CoreLocation
import Photos
[CLLocationManager.shared requestWhenInUseAuthorization];
PHPhotoLibrary.requestAuthorization { status in }
"#;
        let hits = find_apis_in_source(src);
        let keys: std::collections::HashSet<_> = hits.iter().map(|e| e.info_plist_key).collect();
        assert!(keys.contains("NSLocationWhenInUseUsageDescription"));
        assert!(keys.contains("NSPhotoLibraryUsageDescription"));
    }

    #[test]
    fn find_apis_empty_for_unrelated_source() {
        let src = "print(\"hello world\")";
        assert!(find_apis_in_source(src).is_empty());
    }

    #[test]
    fn entries_with_empty_key_still_found_but_no_plist_key() {
        let src = "MFMessageComposeViewController.canSendText();";
        let hits = find_apis_in_source(src);
        assert!(hits.iter().any(|e| e.api == "MFMessageComposeViewController"));
        assert!(hits.iter().any(|e| e.info_plist_key.is_empty()));
    }
}
