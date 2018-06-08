#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Created on Wed May 23 22:02:30 2018

@author: adam
"""

from functools import reduce
from operator import add

from PyQt5.QtCore import pyqtSlot, QSettings

class MainMixin(object):
    
    name = 'Main'
    org = 'Unknown'
    
    components = {}
    docks = {}
    preferences = None
    
    def __init__(self):
    
        self.settings = QSettings(self.org,self.name)
    
    def registerComponent(self,name,component,dock=None):
        
        self.components[name] = component
        
        if dock:
            self.docks[name] = dock(component)
            
    def saveWindow(self):
        
        self.settings.setValue('geometry',self.saveGeometry())
        self.settings.setValue('windowState',self.saveState())
    
    def restoreWindow(self):
        
        if self.settings.value("geometry"):
            self.restoreGeometry(self.settings.value("geometry"))
        if self.settings.value("windowState"):
            self.restoreState(self.settings.value("windowState"))
        

class ComponentMixin(object):
    
    
    name = 'Component'
    preferences = None
    
    _actions = {}
    
    
    def __init__(self):
        
        if self.preferences:
            self.preferences.sigTreeStateChanged.\
                connect(self.updatePreferences)
    
    def menuActions(self):
        
        return self._actions
    
    def toolbarActions(self):
        
        if len(self._actions) > 0:
            return reduce(add,[a for a in self._actions.values()])
        else:
            return []
    
    @pyqtSlot(object,object)
    def updatePreferences(self,*args):
        
        pass