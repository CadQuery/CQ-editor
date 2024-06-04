from traceback import extract_tb, format_exception_only
from itertools import dropwhile

from PyQt5.QtWidgets import (QWidget, QTreeWidget, QTreeWidgetItem, QAction,
                             QLabel)
from PyQt5.QtCore import Qt, pyqtSlot, pyqtSignal

from ..mixins import ComponentMixin
from ..utils import layout

class TracebackTree(QTreeWidget):

    name = 'Traceback Viewer'
    
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
               (self.current_exception,
                self.tree),
               self)
               
        self.tree.currentItemChanged.connect(self.handleSelection)
        
    @pyqtSlot(object,str)
    def addTraceback(self,exc_info,code):
        
        self.tree.clear()
        
        if exc_info:
            t,exc,tb = exc_info
            
            root = self.tree.root
            code = code.splitlines()

            for el in dropwhile(
              lambda el: 'string>' not in el.filename, extract_tb(tb)
            ):
                #workaround of the traceback module
                if el.line == '':
                    line = code[el.lineno-1].strip()
                else:
                    line = el.line

                root.addChild(QTreeWidgetItem([el.filename,
                                               str(el.lineno),
                                               line]))

            exc_name = t.__name__
            exc_msg = str(exc)
            exc_msg = exc_msg.replace('<', '&lt;').replace('>', '&gt;') #replace <> 

            self.current_exception.\
                setText('<b>{}</b>: {}'.format(exc_name,exc_msg))
            
            # handle the special case of a SyntaxError
            if t is SyntaxError: 
                root.addChild(QTreeWidgetItem(
                  [exc.filename,
                   str(exc.lineno),
                   exc.text.strip() if exc.text else '']
                ))
        else:
            self.current_exception.setText('')

    @pyqtSlot(QTreeWidgetItem,QTreeWidgetItem)          
    def handleSelection(self,item,*args):
        
        if item:
            f,line = item.data(0,0),int(item.data(1,0))
            
            if '<string>' in f:
                self.sigHighlightLine.emit(line)
    
        
