from traceback import extract_tb, format_exception_only

from PyQt5.QtWidgets import (QWidget, QTreeWidget, QTreeWidgetItem, QAction,
                             QLabel)
from PyQt5.QtCore import Qt, pyqtSlot, pyqtSignal

from ..mixins import ComponentMixin
from ..utils import layout

class TracebackTree(QTreeWidget):

    
    def  __init__(self,parent):
        
        super(TracebackTree,self).__init__(parent)   
        self.setHeaderHidden(False)
        self.setItemsExpandable(False)
        self.setRootIsDecorated(False)
        self.setContextMenuPolicy(Qt.ActionsContextMenu)
        
        self.setColumnCount(3)
        self.setHeaderLabels(['File','Line','Code'])
        
        
        self.root = self.invisibleRootItem()

class TracebackPane(QWidget,ComponentMixin):
    
    sigHighlightLine = pyqtSignal(int)
    
    def __init__(self,parent):
        
        super(TracebackPane,self).__init__(parent)
        
        self.tree = TracebackTree(self)
        self.current_exception = QLabel(self)
        self.current_exception.setStyleSheet(\
            "QLabel {color : red; }");
        
        layout(self,
               (self.tree,
               self.current_exception),
               self)
               
        self.tree.currentItemChanged.connect(self.handleSelection)
        
    @pyqtSlot(object,str)
    def addTraceback(self,exc_info,code):
        
        self.tree.clear()
        
        if exc_info:
            t,exc,tb = exc_info
            
            
            root = self.tree.root
            code = code.splitlines()
            tb = extract_tb(tb)[1:] #ignore highest error (exec)
            
            for el in tb:
                #workaround of the traceback module
                if el.line is '':
                    line = code[el.lineno-1].strip()
                else:
                    line = el.line
                
                root.addChild(QTreeWidgetItem([el.filename,
                                               str(el.lineno),
                                               line]))
        
            self.current_exception.\
                setText('<b>{}</b>'.format(*format_exception_only(t,exc)))
        else:
            self.current_exception.setText('')

    @pyqtSlot(QTreeWidgetItem,QTreeWidgetItem)          
    def handleSelection(self,item,*args):
        
        f,line = item.data(0,0),int(item.data(1,0))
        
        if '<string>' in f:
            self.sigHighlightLine.emit(line)
    
        