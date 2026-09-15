//! Which Flutter version does this project want?
//!
//! Ladder, first hit wins: FVM pin, asdf pin, CI workflow pin, pubspec
//! constraints, channel head. The channel head is always offered too, so the
//! user sees the difference between "what the project declares" and "newest".

use super::releases::ReleaseManifest;
use crate::doctor::host::Arch;
use crate::version_util::satisfies;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinSource {
    Fvm,
    Asdf,
    CiWorkflow,
    Pubspec,
    ChannelHead,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionCandidate {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    pub source: PinSource,
    pub rationale: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SdkConstraints {
    pub flutter: Option<String>,
    pub dart: Option<String>,
}

pub fn discover_pin(root: &Path) -> Option<VersionCandidate> {
    read_fvmrc(root)
        .or_else(|| read_fvm_config(root))
        .or_else(|| read_tool_versions(root))
        .or_else(|| read_ci_workflows(root))
}

fn read_fvmrc(root: &Path) -> Option<VersionCandidate> {
    let text = std::fs::read_to_string(root.join(".fvmrc")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let version = value.get("flutter")?.as_str()?.to_string();
    Some(VersionCandidate {
        version,
        channel: None,
        source: PinSource::Fvm,
        rationale: "pinned by FVM in this repo (.fvmrc)".into(),
    })
}

fn read_fvm_config(root: &Path) -> Option<VersionCandidate> {
    let text = std::fs::read_to_string(root.join(".fvm").join("fvm_config.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let version = value.get("flutterSdkVersion")?.as_str()?.to_string();
    Some(VersionCandidate {
        version,
        channel: None,
        source: PinSource::Fvm,
        rationale: "pinned by FVM in this repo (.fvm/fvm_config.json)".into(),
    })
}

fn read_tool_versions(root: &Path) -> Option<VersionCandidate> {
    let text = std::fs::read_to_string(root.join(".tool-versions")).ok()?;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() != Some("flutter") {
            continue;
        }
        let Some(raw) = parts.next() else {
            // Malformed line (e.g. `flutter` with no version) — keep scanning
            // rather than abandoning the whole file.
            continue;
        };
        // asdf spells versions `3.19.0-stable`.
        let (version, channel) = match raw.split_once('-') {
            Some((v, c)) => (v.to_string(), Some(c.to_string())),
            None => (raw.to_string(), None),
        };
        return Some(VersionCandidate {
            version,
            channel,
            source: PinSource::Asdf,
            rationale: "pinned by asdf (.tool-versions)".into(),
        });
    }
    None
}

fn read_ci_workflows(root: &Path) -> Option<VersionCandidate> {
    let dir = root.join(".github").join("workflows");
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("yml" | "yaml")
        ) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            let trimmed = line.trim();
            let Some(rest) = trimmed.strip_prefix("flutter-version:") else {
                continue;
            };
            let version = rest.trim().trim_matches(['\'', '"']).to_string();
            if version.is_empty() || version.contains("${{") {
                continue;
            }
            let file = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            return Some(VersionCandidate {
                version,
                channel: None,
                source: PinSource::CiWorkflow,
                rationale: format!("what CI builds with (.github/workflows/{})", file),
            });
        }
    }
    None
}

/// Read `environment.sdk` and `environment.flutter` from pubspec.yaml.
pub fn read_pubspec_constraints(root: &Path) -> SdkConstraints {
    let Ok(text) = std::fs::read_to_string(root.join("pubspec.yaml")) else {
        return SdkConstraints::default();
    };
    let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(&text) else {
        return SdkConstraints::default();
    };
    let env = doc.get("environment");
    let read =
        |key: &str| -> Option<String> { env?.get(key)?.as_str().map(|s| s.trim().to_string()) };
    SdkConstraints {
        flutter: read("flutter"),
        dart: read("sdk"),
    }
}

/// Newest release on `channel` satisfying every declared constraint.
pub fn resolve_from_constraints(
    manifest: &ReleaseManifest,
    channel: &str,
    arch: Arch,
    constraints: &SdkConstraints,
) -> Option<VersionCandidate> {
    // With no declared constraint at all, there is nothing pubspec-derived to
    // report — fabricating a "satisfying" rationale here would be a lie no
    // matter who calls this function.
    if constraints.flutter.is_none() && constraints.dart.is_none() {
        return None;
    }
    let release = manifest.for_channel(channel, arch).into_iter().find(|r| {
        let flutter_ok = constraints
            .flutter
            .as_deref()
            .map(|c| satisfies(&r.version, c))
            .unwrap_or(true);
        let dart_ok = match (&constraints.dart, &r.dart_sdk_version) {
            (Some(c), Some(v)) => satisfies(v, c),
            (Some(_), None) => false,
            (None, _) => true,
        };
        flutter_ok && dart_ok
    })?;
    let mut parts = Vec::new();
    if let Some(f) = &constraints.flutter {
        parts.push(format!("Flutter {}", f));
    }
    if let Some(d) = &constraints.dart {
        parts.push(format!("Dart {}", d));
    }
    Some(VersionCandidate {
        version: release.version.clone(),
        channel: Some(channel.to_string()),
        source: PinSource::Pubspec,
        rationale: format!(
            "newest {} release satisfying {}",
            channel,
            parts.join(" and ")
        ),
    })
}

/// Every version worth offering, pin first, channel head always included.
pub fn candidates(
    root: &Path,
    manifest: &ReleaseManifest,
    channel: &str,
    arch: Arch,
) -> Vec<VersionCandidate> {
    let constraints = read_pubspec_constraints(root);
    let mut out: Vec<VersionCandidate> = Vec::new();

    if let Some(mut pin) = discover_pin(root) {
        if let Some(c) = &constraints.flutter {
            if !satisfies(&pin.version, c) {
                pin.rationale = format!(
                    "{} — conflict: pubspec requires Flutter {}",
                    pin.rationale, c
                );
            }
        }
        out.push(pin);
    }

    if let Some(from_pubspec) = resolve_from_constraints(manifest, channel, arch, &constraints) {
        out.push(from_pubspec);
    }

    if let Some(head) = manifest.latest(channel, arch) {
        out.push(VersionCandidate {
            version: head.version.clone(),
            channel: Some(channel.to_string()),
            source: PinSource::ChannelHead,
            rationale: format!("latest on {}", channel),
        });
    }

    let mut seen = std::collections::HashSet::new();
    out.retain(|c| seen.insert(c.version.clone()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::flutter::releases::ReleaseManifest;
    use crate::doctor::host::Arch;
    use std::fs;
    use tempfile::TempDir;

    const FIXTURE: &str = include_str!("../../../tests/fixtures/releases_macos.json");

    fn manifest() -> ReleaseManifest {
        ReleaseManifest::parse(FIXTURE).unwrap()
    }

    fn project() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn fvmrc_is_the_highest_priority_pin() {
        let dir = project();
        fs::write(dir.path().join(".fvmrc"), r#"{"flutter":"3.24.5"}"#).unwrap();
        fs::write(dir.path().join(".tool-versions"), "flutter 3.19.0-stable\n").unwrap();
        let pin = discover_pin(dir.path()).unwrap();
        assert_eq!(pin.version, "3.24.5");
        assert_eq!(pin.source, PinSource::Fvm);
        assert!(
            pin.rationale.contains("FVM"),
            "rationale was {:?}",
            pin.rationale
        );
    }

    #[test]
    fn legacy_fvm_config_is_read_when_fvmrc_is_absent() {
        let dir = project();
        fs::create_dir_all(dir.path().join(".fvm")).unwrap();
        fs::write(
            dir.path().join(".fvm/fvm_config.json"),
            r#"{"flutterSdkVersion":"3.22.1"}"#,
        )
        .unwrap();
        assert_eq!(discover_pin(dir.path()).unwrap().version, "3.22.1");
    }

    #[test]
    fn asdf_pin_beats_ci_workflow() {
        let dir = project();
        fs::write(
            dir.path().join(".tool-versions"),
            "dart 3.5.0\nflutter 3.19.0-stable\n",
        )
        .unwrap();
        fs::create_dir_all(dir.path().join(".github/workflows")).unwrap();
        fs::write(
            dir.path().join(".github/workflows/ci.yml"),
            "jobs:\n  t:\n    steps:\n      - uses: subosito/flutter-action@v2\n        with:\n          flutter-version: 3.13.0\n",
        )
        .unwrap();
        let pin = discover_pin(dir.path()).unwrap();
        assert_eq!(pin.version, "3.19.0");
        assert_eq!(pin.source, PinSource::Asdf);
        assert_eq!(pin.channel.as_deref(), Some("stable"));
    }

    #[test]
    fn ci_workflow_pin_is_found_when_nothing_else_pins() {
        let dir = project();
        fs::create_dir_all(dir.path().join(".github/workflows")).unwrap();
        fs::write(
            dir.path().join(".github/workflows/ci.yml"),
            "      - uses: subosito/flutter-action@v2\n        with:\n          flutter-version: '3.13.0'\n",
        )
        .unwrap();
        let pin = discover_pin(dir.path()).unwrap();
        assert_eq!(pin.version, "3.13.0");
        assert_eq!(pin.source, PinSource::CiWorkflow);
    }

    #[test]
    fn no_pin_returns_none() {
        assert!(discover_pin(project().path()).is_none());
    }

    #[test]
    fn pubspec_constraints_are_read_from_the_environment_block() {
        let dir = project();
        fs::write(
            dir.path().join("pubspec.yaml"),
            "name: app\nenvironment:\n  sdk: '>=3.4.0 <4.0.0'\n  flutter: '>=3.22.0'\n",
        )
        .unwrap();
        let c = read_pubspec_constraints(dir.path());
        assert_eq!(c.dart.as_deref(), Some(">=3.4.0 <4.0.0"));
        assert_eq!(c.flutter.as_deref(), Some(">=3.22.0"));
    }

    #[test]
    fn missing_pubspec_yields_empty_constraints() {
        let c = read_pubspec_constraints(project().path());
        assert!(c.flutter.is_none() && c.dart.is_none());
    }

    #[test]
    fn resolve_picks_the_newest_release_satisfying_both_constraints() {
        let c = SdkConstraints {
            flutter: Some(">=3.20.0".into()),
            dart: Some(">=3.4.0 <4.0.0".into()),
        };
        let got = resolve_from_constraints(&manifest(), "stable", Arch::X64, &c).unwrap();
        assert_eq!(got.version, "3.24.5");
        assert_eq!(got.source, PinSource::Pubspec);
    }

    #[test]
    fn resolve_rejects_releases_whose_dart_version_is_too_old() {
        let c = SdkConstraints {
            flutter: None,
            dart: Some(">=3.6.0".into()),
        };
        assert!(resolve_from_constraints(&manifest(), "stable", Arch::X64, &c).is_none());
    }

    #[test]
    fn release_without_dart_version_never_satisfies_a_dart_constraint() {
        // No `dart_sdk_version` key at all — `#[serde(default)]` produces `None`,
        // distinct from an explicit `null`.
        let json = r#"{
            "base_url": "https://example.test",
            "current_release": {},
            "releases": [
                {
                    "hash": "deadbeef",
                    "channel": "stable",
                    "version": "3.30.0",
                    "dart_sdk_arch": "x64",
                    "archive": "stable/macos/flutter_macos_3.30.0-stable.zip",
                    "sha256": "5555555555555555555555555555555555555555555555555555555555555555"
                }
            ]
        }"#;
        let m = ReleaseManifest::parse(json).unwrap();

        let with_dart_constraint = SdkConstraints {
            flutter: None,
            dart: Some(">=3.0.0".into()),
        };
        assert!(
            resolve_from_constraints(&m, "stable", Arch::X64, &with_dart_constraint).is_none(),
            "a release with unknown Dart version must not satisfy a declared Dart constraint"
        );

        // A trivially-satisfied Flutter constraint with no Dart constraint at
        // all — not `SdkConstraints::default()`, since resolve_from_constraints
        // now refuses to fabricate a rationale when *no* constraint is
        // declared at all (see `no_constraint_at_all_never_resolves_from_pubspec`
        // below). This keeps the release itself, and the missing-dart-version
        // interaction specifically, as the only variable, proving the negative
        // case above isn't vacuous.
        let without_dart_constraint = SdkConstraints {
            flutter: Some(">=3.0.0".into()),
            dart: None,
        };
        assert!(
            resolve_from_constraints(&m, "stable", Arch::X64, &without_dart_constraint).is_some(),
            "the same release must resolve when no Dart constraint is declared, \
             proving the negative case above isn't vacuous"
        );
    }

    #[test]
    fn candidates_always_include_channel_head_alongside_the_pin() {
        let dir = project();
        fs::write(dir.path().join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
        let got = candidates(dir.path(), &manifest(), "stable", Arch::X64);
        let versions: Vec<_> = got.iter().map(|c| c.version.clone()).collect();
        assert!(
            versions.contains(&"3.19.0".to_string()),
            "pin missing: {:?}",
            versions
        );
        assert!(
            versions.contains(&"3.24.5".to_string()),
            "channel head missing: {:?}",
            versions
        );
        assert_eq!(
            got[0].source,
            PinSource::Fvm,
            "the pin must be offered first"
        );
    }

    #[test]
    fn candidates_are_deduplicated_when_pin_equals_channel_head() {
        let dir = project();
        fs::write(dir.path().join(".fvmrc"), r#"{"flutter":"3.24.5"}"#).unwrap();
        let got = candidates(dir.path(), &manifest(), "stable", Arch::X64);
        assert_eq!(got.iter().filter(|c| c.version == "3.24.5").count(), 1);
    }

    #[test]
    fn a_pin_conflicting_with_pubspec_is_still_offered_with_the_conflict_stated() {
        let dir = project();
        fs::write(dir.path().join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
        fs::write(
            dir.path().join("pubspec.yaml"),
            "name: app\nenvironment:\n  flutter: '>=3.22.0'\n",
        )
        .unwrap();
        let got = candidates(dir.path(), &manifest(), "stable", Arch::X64);
        let pin = got
            .iter()
            .find(|c| c.version == "3.19.0")
            .expect("pin must still be offered");
        assert!(
            pin.rationale.contains("conflict"),
            "conflict must be stated: {:?}",
            pin.rationale
        );
    }

    #[test]
    fn no_constraint_at_all_never_resolves_from_pubspec() {
        // No pin, no pubspec.yaml at all — the overwhelmingly common case.
        // candidates() must not fabricate a pubspec-sourced entry with an
        // empty "satisfying " rationale; the single candidate offered should
        // be the properly-labeled channel head.
        let dir = project();
        let got = candidates(dir.path(), &manifest(), "stable", Arch::X64);
        assert_eq!(got.len(), 1, "expected only the channel head: {:?}", got);
        assert_eq!(got[0].source, PinSource::ChannelHead);
        assert_eq!(got[0].rationale, "latest on stable");
    }

    #[test]
    fn interpolated_ci_flutter_version_is_not_a_usable_pin() {
        let dir = project();
        fs::create_dir_all(dir.path().join(".github/workflows")).unwrap();
        fs::write(
            dir.path().join(".github/workflows/ci.yml"),
            "      - uses: subosito/flutter-action@v2\n        with:\n          flutter-version: ${{ matrix.flutter }}\n",
        )
        .unwrap();
        assert!(
            discover_pin(dir.path()).is_none(),
            "an interpolated value must never be reported as a pin"
        );
    }

    #[test]
    fn malformed_tool_versions_line_is_skipped_not_fatal() {
        let dir = project();
        // The first `flutter` line has no version at all; a real pin follows.
        fs::write(
            dir.path().join(".tool-versions"),
            "flutter\nflutter 3.19.0-stable\n",
        )
        .unwrap();
        let pin = discover_pin(dir.path()).unwrap();
        assert_eq!(pin.version, "3.19.0");
        assert_eq!(pin.source, PinSource::Asdf);
    }
}
