#!/bin/sh
set -eu

chmod 755 "$PREFIX/bin/cq-editor"

# Reinstalling the shortcut is idempotent and also refreshes stale launchers
# from an older CQ-editor installation.
"$PREFIX/bin/menuinst" --prefix "$PREFIX" --remove cq-editor_menu.json >/dev/null 2>&1 || true
"$PREFIX/bin/menuinst" --prefix "$PREFIX" --install cq-editor_menu.json
