#!/usr/bin/env bash
set -euo pipefail

for name in CQ_MAC_APP_CERT_BASE64 CQ_MAC_APP_CERT_PASSWORD CQ_MAC_INSTALLER_CERT_BASE64 CQ_MAC_INSTALLER_CERT_PASSWORD CQ_MAC_APPLICATION_IDENTITY CQ_MAC_INSTALLER_IDENTITY; do
    if [[ -z "${!name:-}" ]]; then
        printf 'Missing macOS signing credential: %s\n' "$name" >&2
        exit 1
    fi
done

keychain="$RUNNER_TEMP/cq-editor-signing.keychain-db"
app_cert="$RUNNER_TEMP/cq-editor-application.p12"
installer_cert="$RUNNER_TEMP/cq-editor-installer.p12"
password="$(openssl rand -hex 24)"
trap 'rm -f "$app_cert" "$installer_cert"' EXIT

printf '%s' "$CQ_MAC_APP_CERT_BASE64" | base64 -D > "$app_cert"
printf '%s' "$CQ_MAC_INSTALLER_CERT_BASE64" | base64 -D > "$installer_cert"
security create-keychain -p "$password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$password" "$keychain"
security import "$app_cert" -k "$keychain" -P "$CQ_MAC_APP_CERT_PASSWORD" -T /usr/bin/codesign
security import "$installer_cert" -k "$keychain" -P "$CQ_MAC_INSTALLER_CERT_PASSWORD" -T /usr/bin/productsign
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$password" "$keychain"
security list-keychains -d user -s "$keychain" "$HOME/Library/Keychains/login.keychain-db"

security find-identity -v -p codesigning "$keychain" | grep -F -- "$CQ_MAC_APPLICATION_IDENTITY" >/dev/null
security find-identity -v "$keychain" | grep -F -- "$CQ_MAC_INSTALLER_IDENTITY" >/dev/null
printf 'CQ_MAC_KEYCHAIN=%s\n' "$keychain" >> "$GITHUB_ENV"
