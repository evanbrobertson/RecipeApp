#!/usr/bin/env bash
# Builds the iOS app for VERSION into OUT_DIR. Used by .github/workflows/ios-release.yml
# (run by main.yml on every master push and by promote.yml); works the same on a Mac.
#
# Signed, when APP_STORE_CONNECT_KEY_ID, APP_STORE_CONNECT_ISSUER_ID,
# APP_STORE_CONNECT_PRIVATE_KEY (the .p8's contents) and APPLE_TEAM_ID are set: Xcode signs
# with automatic, cloud-managed signing through the API key and exports
# crumb-ios-VERSION.ipa for the App Store; with UPLOAD=1 it also goes to TestFlight. An
# optional IOS_CERTIFICATE_P12_BASE64 (+ IOS_CERTIFICATE_PASSWORD) is imported first for
# teams that keep their own distribution certificate.
#
# Otherwise: an unsigned Release archive, packed as crumb-ios-VERSION-unsigned.ipa (for
# re-signing or sideloading) and a note in the job summary.
#
# The marketing version is VERSION without its pre-release part (3.1.0-main.57 → 3.1.0);
# the build number is CRUMB_BUILD_NUMBER, or the UTC time as YYYYMMDD.HHMM, so every upload
# is newer than the last.
set -euo pipefail

version=${1:?usage: build-release.sh VERSION OUT_DIR}
out=${2:?usage: build-release.sh VERSION OUT_DIR}
ios=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$out"
out=$(cd "$out" && pwd)

marketing=${version%%-*}
build=${CRUMB_BUILD_NUMBER:-$(date -u +%Y%m%d.%H%M)}
bundle_id=${CRUMB_BUNDLE_ID:-app.crumb.ios}
summary() { [[ -n ${GITHUB_STEP_SUMMARY:-} ]] && echo "$1" >> "$GITHUB_STEP_SUMMARY" || true; }

"$ios/scripts/build-core.sh" --release
cd "$ios"
xcodegen generate

work=$(mktemp -d)
cleanup() {
  if [[ -n ${keychain:-} ]]; then security delete-keychain "$keychain" 2> /dev/null || true; fi
  rm -rf "$work"
}
trap cleanup EXIT

archive="$work/Crumb.xcarchive"
settings=(
  MARKETING_VERSION="$marketing"
  CURRENT_PROJECT_VERSION="$build"
  CRUMB_BUNDLE_ID="$bundle_id"
)

if [[ -n ${APP_STORE_CONNECT_KEY_ID:-} && -n ${APP_STORE_CONNECT_ISSUER_ID:-} &&
  -n ${APP_STORE_CONNECT_PRIVATE_KEY:-} && -n ${APPLE_TEAM_ID:-} ]]; then
  key="$work/AuthKey_$APP_STORE_CONNECT_KEY_ID.p8"
  printf '%s\n' "$APP_STORE_CONNECT_PRIVATE_KEY" > "$key"
  auth=(
    -allowProvisioningUpdates
    -authenticationKeyPath "$key"
    -authenticationKeyID "$APP_STORE_CONNECT_KEY_ID"
    -authenticationKeyIssuerID "$APP_STORE_CONNECT_ISSUER_ID"
  )

  if [[ -n ${IOS_CERTIFICATE_P12_BASE64:-} ]]; then
    keychain="$work/signing.keychain-db"
    password=$(uuidgen)
    security create-keychain -p "$password" "$keychain"
    security set-keychain-settings -lut 3600 "$keychain"
    security unlock-keychain -p "$password" "$keychain"
    echo "$IOS_CERTIFICATE_P12_BASE64" | base64 --decode > "$work/cert.p12"
    security import "$work/cert.p12" -k "$keychain" -P "${IOS_CERTIFICATE_PASSWORD:-}" -T /usr/bin/codesign
    security set-key-partition-list -S apple-tool:,apple: -s -k "$password" "$keychain" > /dev/null
    # shellcheck disable=SC2046 # the existing search list, one path per word
    security list-keychains -d user -s "$keychain" $(security list-keychains -d user | tr -d '"')
  fi

  echo "▸ Archiving $marketing ($build), signed for team $APPLE_TEAM_ID"
  xcodebuild archive -project Crumb.xcodeproj -scheme Crumb -configuration Release \
    -destination 'generic/platform=iOS' -archivePath "$archive" \
    "${settings[@]}" DEVELOPMENT_TEAM="$APPLE_TEAM_ID" CODE_SIGN_STYLE=Automatic \
    "${auth[@]}" | grep -E '^(\*\*|error|warning: .*sign)|: error' || true
  [[ -d $archive ]] || { echo "::error::The archive failed; see the xcodebuild output above."; exit 1; }

  export_plist() {
    cat > "$work/$1.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>method</key><string>app-store-connect</string>
  <key>destination</key><string>$1</string>
  <key>teamID</key><string>$APPLE_TEAM_ID</string>
  <key>signingStyle</key><string>automatic</string>
  <key>manageAppVersionAndBuildNumber</key><false/>
  <key>uploadSymbols</key><true/>
</dict>
</plist>
PLIST
    echo "$work/$1.plist"
  }

  xcodebuild -exportArchive -archivePath "$archive" -exportPath "$work/export" \
    -exportOptionsPlist "$(export_plist export)" "${auth[@]}"
  name="crumb-ios-$version.ipa"
  cp "$work/export/Crumb.ipa" "$out/$name"
  summary "Signed **crumb-ios-$version** ($marketing, build $build)."

  if [[ ${UPLOAD:-0} == 1 ]]; then
    echo "▸ Uploading to App Store Connect (TestFlight)"
    xcodebuild -exportArchive -archivePath "$archive" -exportPath "$work/upload" \
      -exportOptionsPlist "$(export_plist upload)" "${auth[@]}"
    summary "Uploaded $marketing ($build) to TestFlight. It appears there once Apple has processed it."
  fi
else
  echo "▸ Archiving $marketing ($build), unsigned"
  xcodebuild archive -project Crumb.xcodeproj -scheme Crumb -configuration Release \
    -destination 'generic/platform=iOS' -archivePath "$archive" \
    "${settings[@]}" CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY="" \
    | grep -E '^(\*\*|error)|: error' || true
  [[ -d $archive/Products/Applications/Crumb.app ]] || { echo "::error::The archive failed."; exit 1; }
  mkdir -p "$work/ipa/Payload"
  cp -R "$archive/Products/Applications/Crumb.app" "$work/ipa/Payload/"
  name="crumb-ios-$version-unsigned.ipa"
  (cd "$work/ipa" && zip -qry "$out/$name" Payload)
  summary "No App Store Connect key; **crumb-ios-$version** is unsigned (see ios/README.md to set up signing)."
fi

(cd "$out" && shasum -a 256 "$name" > "$name.sha256")
echo "Built $out/$name"
