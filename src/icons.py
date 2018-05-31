#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Created on Fri May 25 14:47:10 2018

@author: adam
"""

import qtawesome as qta


_icons_specs = {
    'new'  : (('fa.file-o',),{}),
    'open' : (('fa.folder-open-o',),{}),
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
                     'color': 'gray'},{}]})
}

def icon(name):
    
    args,kwargs = _icons_specs[name]
    
    return qta.icon(*args,**kwargs)