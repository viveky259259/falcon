//! `falcon check-platform-deps` — verify Info.plist / AndroidManifest.xml
//! declare every key/permission the project's plugins require.

pub mod android_scan;
pub mod apple_api_map;
pub mod ios_scan;
