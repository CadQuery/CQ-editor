#!/bin/sh
export QT_MAC_WANTS_LAYER=1
chmod u+x ./CQ-editor/CQ-editor
QT_QPA_PLATFORM=xcb PYOPENGL_PLATFORM=x11 ./CQ-editor/CQ-editor
