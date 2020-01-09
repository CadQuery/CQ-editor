#!/bin/sh
version="0.1"
appname="CQ-Editor"
appbundle="../dist/CQ-Editor.app"
dmgfile="${appname} v${version}.dmg"
test -f "$dmgfile" && rm "$dmgfile"
# xattr -cr $appbundle
dmgbuild -s cq_dmg_settings.py "${appname} App v.${version}" "$dmgfile"
fileicon set "$dmgfile" CQDiskImageIcon.png
