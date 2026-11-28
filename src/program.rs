//! Find the program to launch for a command name such as `flutter`, `dart` or
//! `sdkmanager`.
//!
//! On macOS and Linux a name is launched as is. On Windows, Rust's
//! `Command::new("flutter")` only looks for `flutter.exe`, but Flutter's
//! tools are batch files (`flutter.bat`, `dart.bat`, `sdkmanager.bat`). So
//! on Windows a bare name is looked up the way `cmd.exe` does: `FLUTTER_ROOT`
//! first for Flutter's own tools, then each `PATH` directory with each
//! `PATHEXT` extension.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The `flutter` program to launch.
pub fn flutter() -> OsString {
    resolve("flutter")
}

/// The `dart` program to launch.
pub fn dart() -> OsString {
    resolve("dart")
}

/// The program to launch for `name`. Unchanged except on Windows.
pub fn resolve(name: impl AsRef<OsStr>) -> OsString {
    let name = name.as_ref();
    if !cfg!(windows) {
        return name.to_os_string();
    }
    let env = WindowsEnv {
        flutter_root: std::env::var_os("FLUTTER_ROOT"),
        path: std::env::var_os("PATH"),
        pathext: std::env::var_os("PATHEXT"),
    };
    find_on_windows(name, &env, |p| p.is_file())
        .map(PathBuf::into_os_string)
        .unwrap_or_else(|| name.to_os_string())
}

struct WindowsEnv {
    flutter_root: Option<OsString>,
    path: Option<OsString>,
    pathext: Option<OsString>,
}

const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// Windows lookup, kept free of the real environment and file system so it
/// can be tested on any OS.
fn find_on_windows(
    name: &OsStr,
    env: &WindowsEnv,
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let text = name.to_string_lossy();
    let pathext = env
        .pathext
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_PATHEXT.to_string());
    let exts: Vec<String> = pathext
        .split(';')
        .map(|e| e.trim().to_ascii_lowercase())
        .filter(|e| e.starts_with('.'))
        .collect();
    let with_exts = |base: &Path| -> Option<PathBuf> {
        exts.iter()
            .map(|e| PathBuf::from(format!("{}{}", base.to_string_lossy(), e)))
            .find(|p| exists(p))
    };

    // A path such as C:\sdk\flutter\bin\flutter: add the missing extension.
    if text.contains(['\\', '/']) {
        let path = Path::new(name);
        if path.extension().is_some() || exists(path) {
            return None;
        }
        return with_exts(path);
    }
    // A name that already has an extension, e.g. flutter.bat, needs nothing.
    if Path::new(name).extension().is_some() {
        return None;
    }
    if matches!(text.as_ref(), "flutter" | "dart") {
        if let Some(root) = env.flutter_root.as_ref().filter(|r| !r.is_empty()) {
            let base = Path::new(root).join("bin").join(name);
            if let Some(found) = with_exts(&base) {
                return Some(found);
            }
        }
    }
    let path = env.path.as_ref()?.to_string_lossy().into_owned();
    path.split(';')
        .map(|dir| dir.trim().trim_matches('"'))
        .filter(|dir| !dir.is_empty())
        .find_map(|dir| with_exts(&Path::new(dir).join(name)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn env(root: Option<&str>, path: &str) -> WindowsEnv {
        WindowsEnv {
            flutter_root: root.map(OsString::from),
            path: Some(path.into()),
            pathext: None,
        }
    }

    fn files(list: &[PathBuf]) -> impl Fn(&Path) -> bool {
        let set: HashSet<PathBuf> = list.iter().cloned().collect();
        move |p| set.contains(p)
    }

    fn p(dir: &str, file: &str) -> PathBuf {
        Path::new(dir).join(file)
    }

    #[test]
    fn finds_flutter_bat_on_path() {
        let bat = p(r"C:\sdk\flutter\bin", "flutter.bat");
        let found = find_on_windows(
            OsStr::new("flutter"),
            &env(None, r"C:\Windows;C:\sdk\flutter\bin"),
            files(std::slice::from_ref(&bat)),
        );
        assert_eq!(found, Some(bat));
    }

    #[test]
    fn flutter_root_wins_over_path() {
        let root_bat = Path::new(r"D:\flutter").join("bin").join("dart.bat");
        let path_exe = p(r"C:\dart-sdk\bin", "dart.exe");
        let found = find_on_windows(
            OsStr::new("dart"),
            &env(Some(r"D:\flutter"), r"C:\dart-sdk\bin"),
            files(&[root_bat.clone(), path_exe]),
        );
        assert_eq!(found, Some(root_bat));
    }

    #[test]
    fn pathext_order_prefers_exe_over_bat_in_one_directory() {
        let exe = p(r"C:\dart-sdk\bin", "dart.exe");
        let bat = p(r"C:\dart-sdk\bin", "dart.bat");
        let found = find_on_windows(
            OsStr::new("dart"),
            &env(None, r"C:\dart-sdk\bin"),
            files(&[bat, exe.clone()]),
        );
        assert_eq!(found, Some(exe));
    }

    #[test]
    fn other_tools_skip_flutter_root() {
        let sdkmanager = p(r"C:\Android\cmdline-tools\latest\bin", "sdkmanager.bat");
        let found = find_on_windows(
            OsStr::new("sdkmanager"),
            &env(Some(r"D:\flutter"), r"C:\Android\cmdline-tools\latest\bin"),
            files(std::slice::from_ref(&sdkmanager)),
        );
        assert_eq!(found, Some(sdkmanager));
    }

    #[test]
    fn a_path_without_extension_gets_one() {
        let bat = PathBuf::from(r"C:\sdk\flutter\bin\flutter.bat");
        let found = find_on_windows(
            OsStr::new(r"C:\sdk\flutter\bin\flutter"),
            &env(None, ""),
            files(std::slice::from_ref(&bat)),
        );
        assert_eq!(found, Some(bat));
    }

    #[test]
    fn names_with_extensions_and_missing_programs_are_left_alone() {
        let any = files(&[p(r"C:\x", "flutter.bat")]);
        assert_eq!(
            find_on_windows(OsStr::new("flutter.bat"), &env(None, r"C:\x"), &any),
            None
        );
        assert_eq!(
            find_on_windows(OsStr::new("pod"), &env(None, r"C:\x"), &any),
            None
        );
    }

    #[test]
    fn quoted_and_empty_path_entries_are_handled() {
        let bat = p(r"C:\Program Files\flutter\bin", "flutter.bat");
        let found = find_on_windows(
            OsStr::new("flutter"),
            &env(None, r#";"C:\Program Files\flutter\bin";"#),
            files(std::slice::from_ref(&bat)),
        );
        assert_eq!(found, Some(bat));
    }

    #[test]
    fn unchanged_off_windows() {
        if !cfg!(windows) {
            assert_eq!(resolve("flutter"), OsString::from("flutter"));
            assert_eq!(flutter(), OsString::from("flutter"));
            assert_eq!(dart(), OsString::from("dart"));
        }
    }
}
