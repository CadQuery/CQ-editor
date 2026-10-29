#!/usr/bin/env bash
set -euo pipefail

for name in CQ_MAC_NOTARY_KEY_BASE64 CQ_MAC_NOTARY_KEY_ID CQ_MAC_NOTARY_ISSUER_ID; do
    if [[ -z "${!name:-}" ]]; then
        printf 'Missing notarization credential: %s\n' "$name" >&2
        exit 1
    fi
done

shopt -s nullglob
packages=(dist/*.pkg)
if (( ${#packages[@]} != 1 )); then
    echo 'Expected exactly one macOS installer package.' >&2
    exit 1
fi
package="${packages[0]}"
key="$RUNNER_TEMP/cq-editor-notary-key.p8"
result="$RUNNER_TEMP/cq-editor-notarization.json"
trap 'rm -f "$key" "$result"' EXIT
printf '%s' "$CQ_MAC_NOTARY_KEY_BASE64" | base64 -D > "$key"
chmod 600 "$key"

pkgutil --check-signature "$package"
xcrun notarytool submit "$package" --key "$key" --key-id "$CQ_MAC_NOTARY_KEY_ID" --issuer "$CQ_MAC_NOTARY_ISSUER_ID" --wait --timeout 45m --output-format json > "$result"
status="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["status"])' "$result")"
if [[ "$status" != Accepted ]]; then
    submission="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$result")"
    xcrun notarytool log "$submission" --key "$key" --key-id "$CQ_MAC_NOTARY_KEY_ID" --issuer "$CQ_MAC_NOTARY_ISSUER_ID" || true
    printf 'Apple notarization failed with status: %s\n' "$status" >&2
    exit 1
fi
xcrun stapler staple "$package"
xcrun stapler validate "$package"
pkgutil --check-signature "$package"
spctl --assess --type install --verbose=2 "$package"
