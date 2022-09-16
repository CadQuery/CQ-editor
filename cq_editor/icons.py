#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Created on Fri May 25 14:47:10 2018

@author: adam
"""

from PyQt5.QtGui import QIcon

from . import icons_res
_icons = {
    'app' : QIcon(":/images/icons/cadquery_logo_dark.svg")
    }

import qtawesome as qta

_icons_specs = {
    'new'  : (('fa.file-o',),{}),
    'open' : (('fa.folder-open-o',),{}),
    # borrowed from spider-ide
    'autoreload': [('fa.repeat', 'fa.clock-o'), {'options': [{'scale_factor': 0.75, 'offset': (-0.1, -0.1)}, {'scale_factor': 0.5, 'offset': (0.25, 0.25)}]}],
    'save' : (('fa.save',),{}),
    'save_as': (('fa.save','fa.pencil'),
               {'options':[{'scale_factor': 1,},
                           {'scale_factor': 0.8,
                            'offset': (0.2, 0.2)}]}),
    'run'  : (('fa.play',),{}),
    'delete' : (('fa.trash',),{}),
    'delete-many' : (('fa.trash','fa.trash',),
                     {'options' : \
                      [{'scale_factor': 0.8,
                         'offset': (0.2, 0.2),
                         'color': 'gray'},
                       {'scale_factor': 0.8}]}),
    'help' : (('fa.life-ring',),{}),
    'about': (('fa.info',),{}),
    'preferences' : (('fa.cogs',),{}),
    'inspect' : (('fa.cubes','fa.search'),
                 {'options' : \
                  [{'scale_factor': 0.8,
                     'offset': (0,0),
                     'color': 'gray'},{}]}),
    'screenshot' : (('fa.camera',),{}),
    'screenshot-save' : (('fa.save','fa.camera'),
                         {'options' : \
                          [{'scale_factor': 0.8},
                           {'scale_factor': 0.8,
                            'offset': (.2,.2)}]})
}

def icon(name):

    if name in _icons:
        return _icons[name]

    args,kwargs = _icons_specs[name]

    return qta.icon(*args,**kwargs)