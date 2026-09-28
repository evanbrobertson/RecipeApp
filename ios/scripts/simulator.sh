#!/usr/bin/env bash
# Prints the UDID of an available iPhone simulator on the newest iOS runtime installed, for
# xcodebuild's -destination. The runner images change which models they ship, so CI asks
# rather than naming one.
set -euo pipefail

xcrun simctl list devices available --json | python3 -c '
import json, sys
runtimes = json.load(sys.stdin)["devices"]
def version(key):
    tail = key.rsplit(".", 1)[-1]  # com.apple.CoreSimulator.SimRuntime.iOS-18-2
    return [int(p) for p in tail.split("-")[1:] if p.isdigit()] if tail.startswith("iOS-") else []
for key in sorted(runtimes, key=version, reverse=True):
    if not version(key):
        continue
    phones = [d for d in runtimes[key] if d["name"].startswith("iPhone")]
    # Prefer a plain current model over Plus/Pro Max/SE for a typical screen size
    phones.sort(key=lambda d: (any(w in d["name"] for w in ("Plus", "Max", "SE", "mini")), d["name"]))
    if phones:
        print(phones[0]["udid"])
        sys.exit(0)
sys.exit("No iPhone simulator is installed")
'
