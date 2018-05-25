from PyQt5.QtWidgets import QTreeWidget, QTreeWidgetItem, QAction
from PyQt5.QtCore import Qt, pyqtSlot, pyqtSignal

from ..mixins import ComponentMixin

class TracebackTree(QTreeWidget,ComponentMixin):

    
    def  __init__(self,parent):
        
        super(TracebackTree,self).__init__(parent)   
        self.setHeaderHidden(False)
        self.setItemsExpandable(False)
        self.setRootIsDecorated(False)
        self.setContextMenuPolicy(Qt.ActionsContextMenu)
        
        self.setColumnCount(3)
        self.setHeaderLabels(['File','Line number','Code'])
        
        
        self.root = self.invisibleRootItem()
       
    @pyqtSlot(list,str)
    def addTraceback(self,tb,code):
        
        self.clear()
        root = self.root
        code = code.splitlines()
        tb = tb[1:] #ignore highest error (exec)
        
        for el in tb:
            #workaround of the traceback module
            if el.line is '':
                line = code[el.lineno-1].strip()
            else:
                line = el.line
            
            root.addChild(QTreeWidgetItem([el.filename,
                                           str(el.lineno),
                                           line]))