#!/bin/sh
set -eu
repo=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
if [ "$#" -ne 1 ]; then
  echo 'Usage: scripts/verify-native-probe.sh macos' >&2
  exit 2
fi
device=$1
if [ "$device" != macos ]; then
  echo 'This reference fixture currently provisions a macOS host. Use falcon x verify directly for other app/device targets.' >&2
  exit 2
fi
cd "$repo"
mkdir -p .tmp
proof=$(mktemp -d "$repo/.tmp/native-proof.XXXXXX")
work=$(mktemp -d "$repo/.tmp/native-work.XXXXXX")
cleanup() { rm -rf "$work"; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP
if [ -z "${DEVELOPER_DIR:-}" ] && [ -x /Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild ]; then
  export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
fi
echo "Evidence: $proof"
cargo build --bin falcon > "$proof/build.log" 2>&1
./target/debug/falcon x verify --help > "$proof/doctor.log"
flutter create --platforms macos --project-name falcon_native_probe --no-pub "$work" > "$proof/create.log" 2>&1
cp examples/native-verification/pubspec.yaml "$work/pubspec.yaml"
cp examples/native-verification/analysis_options.yaml "$work/analysis_options.yaml"
cp examples/native-verification/lib/main.dart "$work/lib/main.dart"
mkdir -p "$work/integration_test"
cp examples/native-verification/integration_test/native_test.dart "$work/integration_test/native_test.dart"
# The generated template unit test references the template app we replaced.
rm -f "$work/test/widget_test.dart"
(cd "$work" && flutter pub get) > "$proof/pub.log" 2>&1
(cd "$work" && flutter analyze --fatal-infos) > "$proof/analyze.log" 2>&1
./target/debug/falcon x verify "$work" --target integration_test/native_test.dart \
  --device "$device" --timeout 180 --output-dir "$proof/result" > "$proof/cli.log" 2>&1
cat "$proof/result/summary.json"
