# -*- coding: utf-8 -*-
from __future__ import unicode_literals

import biplist
import os.path

#
# Example settings file for dmgbuild
#
# Use like this: dmgbuild -s settings.py "Test Volume" test.dmg

# You can actually use this file for your own application (not just TextEdit)
# by doing e.g.
#
#   dmgbuild -s settings.py -D app=/path/to/My.app "My Application" MyApp.dmg

# .. Useful stuff ..............................................................

application = defines.get("app", "../dist/CQ-Editor.app")
appname = os.path.basename(application)

# Volume format (see hdiutil create -help)
format = defines.get("format", "UDBZ")

# Volume size
size = defines.get("size", "1.0g")

# Files to include
files = [application]

# Symlinks to create
symlinks = {"Applications": "/Applications"}

# Volume icon
#
# You can either define icon, in which case that icon file will be copied to the
# image, *or* you can define badge_icon, in which case the icon file you specify
# will be used to badge the system's Removable Disk icon
#
badge_icon = "../icons/cadquery_logo_dark.icns"

# Where to put the icons
icon_locations = {appname: (130, 190), "Applications": (470, 185)}

# .. Window configuration ......................................................

background = "CQInstallerBackground.png"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
sidebar_width = 180

# Window position in ((x, y), (w, h)) format
window_rect = ((200, 120), (600, 400))

# Select the default view; must be one of
#
#    'icon-view'
#    'list-view'
#    'column-view'
#    'coverflow'
#
default_view = "icon-view"

# General view configuration
show_icon_preview = False

# Set these to True to force inclusion of icon/list view settings (otherwise
# we only include settings for the default view)
include_icon_view_settings = "auto"
include_list_view_settings = "auto"

# .. Icon view configuration ...................................................

arrange_by = None
grid_offset = (0, 0)
grid_spacing = 100
scroll_position = (0, 0)
label_pos = "bottom"  # or 'right'
text_size = 16
icon_size = 88
