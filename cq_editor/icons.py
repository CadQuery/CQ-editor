#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Created on Fri May 25 14:47:10 2018

@author: adam
"""

from PyQt5.QtGui import QIcon

from . import icons_res

_icons = {"app": QIcon(":/images/icons/cadquery_logo_dark.svg")}

import qtawesome as qta

_icons_specs = {
    "new": (("fa5.file",), {}),
    "open": (("fa5.folder-open",), {}),
    # borrowed from spider-ide
    "autoreload": [
        ("fa5s.redo-alt", "fa5.clock"),
        {
            "options": [
                {"scale_factor": 0.75, "offset": (-0.1, -0.1)},
                {"scale_factor": 0.5, "offset": (0.25, 0.25)},
            ]
        },
    ],
    "save": (("fa5.save",), {}),
    "save_as": (
        ("fa5.save", "fa5s.pencil-alt"),
        {
            "options": [
                {
                    "scale_factor": 1,
                },
                {"scale_factor": 0.8, "offset": (0.2, 0.2)},
            ]
        },
    ),
    "run": (("fa5s.play",), {}),
    "debug": (("fa5s.bug",), {}),
    "delete": (("fa5s.trash",), {}),
    "delete-many": (
        (
            "fa5s.trash",
            "fa5s.trash",
        ),
        {
            "options": [
                {"scale_factor": 0.8, "offset": (0.2, 0.2), "color": "gray"},
                {"scale_factor": 0.8},
            ]
        },
    ),
    "help": (("fa5s.life-ring",), {}),
    "about": (("fa5s.info",), {}),
    "preferences": (("fa5s.cogs",), {}),
    "inspect": (
        ("fa5s.cubes", "fa5s.search"),
        {"options": [{"scale_factor": 0.8, "offset": (0, 0), "color": "gray"}, {}]},
    ),
    "screenshot": (("fa5s.camera",), {}),
    "screenshot-save": (
        ("fa5.save", "fa5s.camera"),
        {
            "options": [
                {"scale_factor": 0.8},
                {"scale_factor": 0.8, "offset": (0.2, 0.2)},
            ]
        },
    ),
    "toggle-comment": (("fa5s.hashtag",), {}),
    "search": (("fa5s.search",), {}),
    "arrow-step-over": (("fa5s.step-forward",), {}),
    "arrow-step-in": (("fa5s.angle-down",), {}),
    "arrow-continue": (("fa5s.arrow-right",), {}),
}


def icon(name):

    if name in _icons:
        return _icons[name]

    args, kwargs = _icons_specs[name]

    return qta.icon(*args, **kwargs)
