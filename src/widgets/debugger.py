import sys
from enum import Enum, auto

from PyQt5.QtWidgets import (QWidget, QTreeWidget, QTreeWidgetItem, QAction,
                             QLabel, QTableView)
from PyQt5.QtCore import Qt, QObject, pyqtSlot, pyqtSignal, QEventLoop, QAbstractTableModel
from PyQt5 import QtCore

from spyder.utils.icon_manager import icon

from ..mixins import ComponentMixin
from ..utils import layout
from ..cq_utils import find_cq_objects

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
    
    HEADER = ('Name','Type', 'Value')
    
    def __init__(self,parent):
        
        super(LocalsModel,self).__init__(parent)
        self.frame = None
        
    def update_frame(self,frame):
        
        self.frame = \
            [(k,type(v).__name__, str(v)) for k,v in frame.items() if not k.startswith('_')]
        
    
    def rowCount(self,parent=QtCore.QModelIndex()):
        
        if self.frame:
            return len(self.frame) 
        else:
            return 0

    def columnCount(self,parent=QtCore.QModelIndex()):
        
        return 3
    
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
        
        header = self.horizontalHeader()
        header.setStretchLastSection(True)
        
        vheader = self.verticalHeader()
        vheader.setVisible(False)
    
    @pyqtSlot(dict)
    def update_frame(self,frame):
        
        model = LocalsModel(self)
        model.update_frame(frame)
        
        self.setModel(model)

class Debugger(QObject,ComponentMixin):
    
    name = 'Debugger'
    
    sigFrameChanged = pyqtSignal(object)
    sigLineChanged = pyqtSignal(int)
    sigLocalsChanged = pyqtSignal(dict)
    sigCQChanged = pyqtSignal(list,bool)
    sigDebugging = pyqtSignal(bool)
    sigTraceback = pyqtSignal(object,str)
    
    def __init__(self,parent):
        
        super(Debugger,self).__init__(parent)
        ComponentMixin.__init__(self) 
        
        self.inner_event_loop = QEventLoop(self)
        
        self._actions =  \
            {'Run' : [QAction(icon('debug'),
                             'Debug',
                             self,
                             checkable=True,
                             triggered=self.debug),
                      QAction(icon('arrow-step-over'),
                             'Step',
                             self,
                             triggered=lambda: self.debug_cmd(DbgState.STEP)),
                      QAction(icon('arrow-step-in'),
                             'Step in',
                             self,
                             triggered=lambda: None),
                      QAction(icon('arrow-continue'),
                              'Continue',
                              self,
                              triggered=lambda: self.debug_cmd(DbgState.CONT))
                      ]}
    
    @pyqtSlot(bool)
    def debug(self,value):
        if value:
            self.sigDebugging.emit(True)
            self.state = DbgState.STEP
            
            editor = self.parent().components['editor']
            code,self.script,module = editor.get_compiled_code()
            self.breakpoints = [ el[0] for el in editor.get_breakpoints()]
            
            try:
                sys.settrace(self.trace_callback)
                exec(code,module.__dict__,module.__dict__)
            except Exception:
                self.sigTraceback.emit(sys.exc_info(),
                                       self.script)
            finally:
                sys.settrace(None)
                
            self.sigDebugging.emit(False)
            self._actions['Run'][0].setChecked(False)
        else:
            sys.settrace(None)
            self.inner_event_loop.exit(0)

    
    
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
        
        lineno = frame.f_lineno
        line = self.script.splitlines()[lineno-1]
        f_id = id(frame)
        
        if event in (DbgEevent.LINE,DbgEevent.RETURN):
            if (self.state in (DbgState.STEP, DbgState.STEP_IN)) \
            or (lineno in self.breakpoints):
                self.sigLineChanged.emit(lineno)
                self.sigFrameChanged.emit(frame)
                self.sigLocalsChanged.emit(frame.f_locals)
                self.sigCQChanged.emit(find_cq_objects(frame.f_locals),True)
                
                self.inner_event_loop.exec_()
        
        elif event in (DbgEevent.RETURN):
            self.sigLocalsChanged.emit(frame.f_locals)
        
        elif event == DbgEevent.CALL:    
            func_filename = frame.f_code.co_filename
            
            if self.state == DbgState.STEP_IN and func_filename == DUMMY_FILE:
                self.sigLineChanged.emit(lineno)
                self.sigFrameChanged.emit(frame)
                self.state = DbgState.STEP
                
                
            