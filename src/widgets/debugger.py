import sys
from enum import Enum, auto

from PyQt5.QtWidgets import (QWidget, QTreeWidget, QTreeWidgetItem, QAction,
                             QLabel, QTableView)
from PyQt5.QtCore import Qt, QObject, pyqtSlot, pyqtSignal, QEventLoop, QAbstractTableModel
from PyQt5 import QtCore

from spyder.utils.icon_manager import icon

from ..mixins import ComponentMixin
from ..utils import layout

DUMMY_FILE = '<string>'


class DbgState(Enum):
    
    STEP = auto()
    CONT = auto()
    STEP_IN = auto()
    RETURN = auto()

class DbgEevent(object):
    
    LINE = 'line'
    CALL = 'call'
    RETURN = 'return'
    
class LocalsModel(QAbstractTableModel):
    
    HEADER = ('Name','Type')
    
    def __init__(self,parent):
        
        super(LocalsModel,self).__init__(parent)
        self.frame = None
        
    def update_frame(self,frame):
        
        self.frame = \
            [(k,type(v).__name__) for k,v in frame.items() if not k.startswith('_')]
        
    
    def rowCount(self,parent=QtCore.QModelIndex()):
        
        if self.frame:
            return len(self.frame) 
        else:
            return 0

    def columnCount(self,parent=QtCore.QModelIndex()):
        
        return 2
    
    def headerData(self, section, orientation, role=Qt.DisplayRole):
        if role == Qt.DisplayRole and orientation == Qt.Horizontal:
            return self.HEADER[section]
        return QAbstractTableModel.headerData(self, section, orientation, role)
    
    def data(self, index, role):
        if role == QtCore.Qt.DisplayRole:
            i = index.row()
            j = index.column()
            return self.frame[i][j]
        else:
            return QtCore.QVariant()  
    

class LocalsView(QTableView,ComponentMixin):
    
    name = 'Variables'
    
    def __init__(self,parent):
        
        super(LocalsView,self).__init__(parent)
        ComponentMixin.__init__(self)
    
    @pyqtSlot(dict)
    def update_frame(self,frame):
        
        model = LocalsModel(self)
        model.update_frame(frame)
        
        self.setModel(model)

class Debugger(QObject,ComponentMixin):
    
    name = 'Debugger'
    
    sigFrameChanged = pyqtSignal(object)
    sigLineChanged = pyqtSignal(str,int)
    sigDebuggingStarted = pyqtSignal()
    sigDebuggingFinihsed = pyqtSignal()
    
    def __init__(self,parent):
        
        super(Debugger,self).__init__(parent)
        ComponentMixin.__init__(self) 
        
        self.inner_event_loop = QEventLoop(self)
        
        self._actions =  \
                {'Run' : [QAction(icon('debug'),
                                 'Debug',
                                 self,triggered=lambda: None),
                          QAction(icon('arrow-step-over'),
                                 'Step',
                                 self,triggered=lambda: None),
                          QAction(icon('arrow-step-in'),
                                 'Step in',
                                 self,triggered=lambda: None)]}
    
    def debug(self):
        
        self.state = DbgState.STEP
        
        # ## steps needed
        editor = self.parent.components['editor']
        code,module = editor.get_compiled_code()
        self.breakpoints = editor.get_breakpoints()
        
        try:
            sys.settrace()
            exec(code,module.__dict__,module.__dict__)
        finally:
            sys.settrace(None)
            
    
    
    def debug_cmd(self,state=DbgState.STEP):
        
        self.state = state
        self.inner_event_loop.exit(0)
    
    
    def trace_callback(self,frame,event,arg):
        
        filename = frame.f_code.co_filename
        
        if filename==DUMMY_FILE:
            self.trace_local(frame,event,arg)
            return self.trace_callback
        
        else:
            return None
        
    def trace_local(self,frame,event,arg):
        
        filename = frame.f_code.co_filename
        line = frame.f_lineno
        
        if event == DbgEevent.LINE:
            if (self.state in (DbgState.STEP, DbgState.STEP_IN)) \
            or (line in self.breakpoints):
                self.sigLineChanged(filename,line)
                self.sigFrameChanged(frame)
                self.inner_event_loop.exec_()
        
        if event == DbgEevent.CALL:
            
            func_filename = frame.f_code.co_filename
            
            if self.state == DbgState.STEP_IN and func_filename == DUMMY_FILE:
                self.sigLineChanged(filename,line)
                self.sigFrameChanged(frame)
                self.state = DbgState.STEP
                
                
            