#!/usr/bin/env bash
# Builds crumb-core for the iOS app: libcrumb_ffi.a (crates/crumb-ffi) for iPhone, the
# simulator and the Mac (for `swift test`), wrapped as CrumbKit/Frameworks/CrumbCoreFFI.xcframework,
# plus the Swift bindings UniFFI generates, CrumbKit/Sources/CrumbCore/CrumbCore.swift.
# Nothing it writes is committed. Needs macOS with Xcode and rustup.
#
#   scripts/build-core.sh              debug build, every slice
#   scripts/build-core.sh --release    optimised (what archives and releases use)
#   scripts/build-core.sh --quick      Apple silicon slices only (arm64 simulator and Mac)
set -euo pipefail

cd "$(dirname "$0")/../.."
root=$(pwd)
ios="$root/ios"

profile=debug
cargo_profile=()
quick=0
for arg in "$@"; do
  case "$arg" in
    --release)
      profile=release
      cargo_profile=(--release)
      ;;
    --quick) quick=1 ;;
    *)
      echo "Unknown option: $arg" >&2
      exit 2
      ;;
  esac
done

# Oldest OS versions the app supports (project.yml, Package.swift), so the linker agrees
export IPHONEOS_DEPLOYMENT_TARGET=17.0
export MACOSX_DEPLOYMENT_TARGET=14.0

device=(aarch64-apple-ios)
simulator=(aarch64-apple-ios-sim x86_64-apple-ios)
mac=(aarch64-apple-darwin x86_64-apple-darwin)
if [[ $quick == 1 ]]; then
  simulator=(aarch64-apple-ios-sim)
  mac=(aarch64-apple-darwin)
fi

targets=("${device[@]}" "${simulator[@]}" "${mac[@]}")
rustup target add "${targets[@]}" > /dev/null

for target in "${targets[@]}"; do
  echo "▸ crumb-core for $target ($profile)"
  # A static library for Xcode to link; the crate itself stays a cdylib for Android
  cargo rustc --locked -p crumb-ffi --lib --crate-type staticlib --target "$target" "${cargo_profile[@]}"
done

# The Swift bindings, generated from a host build's metadata (identical on every target)
echo "▸ Swift bindings"
cargo build --locked -p crumb-ffi --lib "${cargo_profile[@]}"
host_lib="target/$profile/libcrumb_ffi.dylib"
[[ -f $host_lib ]] || host_lib="target/$profile/libcrumb_ffi.so"
gen=$(mktemp -d)
trap 'rm -rf "$gen"' EXIT
cargo run --locked -q -p crumb-ffi --features bindgen --bin uniffi-bindgen -- \
  generate "$host_lib" --language swift --no-format --out-dir "$gen"

mkdir -p "$gen/headers"
cp "$gen/CrumbCoreFFI.h" "$gen/headers/"
cp "$gen/CrumbCoreFFI.modulemap" "$gen/headers/module.modulemap"

# One library per platform: lipo the architectures of each together
fat() {
  local out=$1
  shift
  local libs=()
  for target in "$@"; do libs+=("target/$target/$profile/libcrumb_ffi.a"); done
  mkdir -p "$(dirname "$out")"
  lipo -create "${libs[@]}" -output "$out"
}
fat "$gen/ios/libcrumb_ffi.a" "${device[@]}"
fat "$gen/simulator/libcrumb_ffi.a" "${simulator[@]}"
fat "$gen/mac/libcrumb_ffi.a" "${mac[@]}"

xcframework="$ios/CrumbKit/Frameworks/CrumbCoreFFI.xcframework"
rm -rf "$xcframework"
mkdir -p "$(dirname "$xcframework")"
xcodebuild -create-xcframework \
  -library "$gen/ios/libcrumb_ffi.a" -headers "$gen/headers" \
  -library "$gen/simulator/libcrumb_ffi.a" -headers "$gen/headers" \
  -library "$gen/mac/libcrumb_ffi.a" -headers "$gen/headers" \
  -output "$xcframework" > /dev/null

cp "$gen/CrumbCore.swift" "$ios/CrumbKit/Sources/CrumbCore/CrumbCore.swift"
echo "✓ $xcframework"
echo "✓ ios/CrumbKit/Sources/CrumbCore/CrumbCore.swift"
