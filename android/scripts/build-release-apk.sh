#!/usr/bin/env bash
# Builds the release APK for VERSION into OUT_DIR as crumb-android-VERSION.apk plus .sha256.
# Signs it when KEYSTORE_BASE64 (and the CRUMB_KEYSTORE_* / CRUMB_KEY_* variables) are set;
# otherwise the APK is unsigned and a note goes to the job summary. Used by main.yml and
# promote.yml; android.yml checks the same build on every change.
set -euo pipefail

version=${1:?usage: build-release-apk.sh VERSION OUT_DIR}
out=${2:?usage: build-release-apk.sh VERSION OUT_DIR}
root=$(cd "$(dirname "$0")/../.." && pwd)
mkdir -p "$out"
out=$(cd "$out" && pwd)

suffix=""
if [[ -n ${KEYSTORE_BASE64:-} ]]; then
  keystore=$(mktemp --suffix=.jks)
  trap 'rm -f "$keystore"' EXIT
  echo "$KEYSTORE_BASE64" | base64 --decode > "$keystore"
  export CRUMB_KEYSTORE_FILE=$keystore
else
  suffix="-unsigned"
  [[ -n ${GITHUB_STEP_SUMMARY:-} ]] &&
    echo "No release keystore; crumb-android-$version is unsigned." >> "$GITHUB_STEP_SUMMARY"
fi

cd "$root/android"
CRUMB_VERSION_NAME=$version ./gradlew --console=plain :app:assembleRelease

apk=$(ls app/build/outputs/apk/release/*.apk | head -1)
name="crumb-android-$version$suffix.apk"
cp "$apk" "$out/$name"
(cd "$out" && sha256sum "$name" > "$name.sha256")
echo "Built $out/$name"
