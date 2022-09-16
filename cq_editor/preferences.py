from PyQt5.QtWidgets import (QTreeWidget, QTreeWidgetItem, 
                             QStackedWidget, QDialog)
from PyQt5.QtCore import pyqtSlot, Qt

from pyqtgraph.parametertree import ParameterTree

from .utils import splitter, layout


class PreferencesTreeItem(QTreeWidgetItem):
    
    def __init__(self,name,widget,):
        
        super(PreferencesTreeItem,self).__init__(name)
        self.widget = widget

class PreferencesWidget(QDialog):
    
    def __init__(self,parent,components):
        
        super(PreferencesWidget,self).__init__(
                parent,
                Qt.Window | Qt.CustomizeWindowHint | Qt.WindowCloseButtonHint,
                windowTitle='Preferences')
        
        self.stacked = QStackedWidget(self)
        self.preferences_tree = QTreeWidget(self,
                                            headerHidden=True,
                                            itemsExpandable=False,
                                            rootIsDecorated=False,
                                            columnCount=1)
        
        self.root = self.preferences_tree.invisibleRootItem()
        
        self.add('General',
                 parent)
        
        for v in parent.components.values():
            self.add(v.name,v)
        
        self.splitter = splitter((self.preferences_tree,self.stacked),(2,5))
        layout(self,(self.splitter,),self)
        
        self.preferences_tree.currentItemChanged.connect(self.handleSelection)

    def add(self,name,component):
        
        if component.preferences:
            widget = ParameterTree()
            widget.setHeaderHidden(True)
            widget.setParameters(component.preferences,showTop=False)
            self.root.addChild(PreferencesTreeItem((name,),
                                                    widget))
            
            self.stacked.addWidget(widget)
            
    @pyqtSlot(QTreeWidgetItem,QTreeWidgetItem)          
    def handleSelection(self,item,*args):
        
        if item:
            self.stacked.setCurrentWidget(item.widget)

