#!/bin/sh
xattr -r -d com.apple.quarantine .
export QT_MAC_WANTS_LAYER=1
chmod u+x ./CQ-editor/CQ-editor
./CQ-editor/CQ-editor
