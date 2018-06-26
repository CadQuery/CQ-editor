import sys, imp
from enum import Enum, auto

from PyQt5.QtWidgets import (QWidget, QTreeWidget, QTreeWidgetItem, QAction,
                             QLabel, QTableView)
from PyQt5.QtCore import Qt, QObject, pyqtSlot, pyqtSignal, QEventLoop, QAbstractTableModel
from PyQt5 import QtCore

from logbook import info
from spyder.utils.icon_manager import icon
import cadquery as cq

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
    
    sigRendered = pyqtSignal(dict)
    sigLocals = pyqtSignal(dict)
    sigTraceback = pyqtSignal(object,str)
    
    sigFrameChanged = pyqtSignal(object)
    sigLineChanged = pyqtSignal(int)
    sigLocalsChanged = pyqtSignal(dict)
    sigCQChanged = pyqtSignal(dict,bool)
    sigDebugging = pyqtSignal(bool)
    
    
    def __init__(self,parent):
        
        super(Debugger,self).__init__(parent)
        ComponentMixin.__init__(self) 
        
        self.inner_event_loop = QEventLoop(self)
        
        self._actions =  \
            {'Run' : [QAction(icon('run'),
                              'Render',
                               self,triggered=self.render),
                      QAction(icon('debug'),
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
    
    def get_current_script(self):
        
        return self.parent().components['editor'].get_text_with_eol()
    
    def get_breakpoints(self):
        
        return self.parent().components['editor'].get_breakpoints()
    
    def compile_code(self,cq_script):
                
        try:
            module = imp.new_module('temp')
            cq_code = compile(cq_script,'<string>','exec')
            return cq_code,module
        except Exception: 
            self.sigTraceback.emit(sys.exc_info(),
                                   cq_script)
            return None,None

    @pyqtSlot(bool)            
    def render(self):
        
        cq_script = self.get_current_script()
        cq_code,t = self.compile_code(cq_script)
        
        cq_objects = {}
        
        t.__dict__['show_object'] = lambda x: cq_objects.update({str(id(x)) : x})
        t.__dict__['debug'] = lambda x: info(str(x))
        t.__dict__['cq'] = cq
        
        if cq_code is None: return
        
        try:
            exec(cq_code,t.__dict__,t.__dict__)
            
            #collect all CQ objects if no explicti show_object was called
            if len(cq_objects) == 0:
                cq_objects = find_cq_objects(t.__dict__)
            self.sigRendered.emit(cq_objects)
            self.sigTraceback.emit(None,
                                   cq_script)
            self.sigLocals.emit(t.__dict__)
        except Exception: 
            self.sigTraceback.emit(sys.exc_info(),
                                   cq_script)
    
    @pyqtSlot(bool)
    def debug(self,value):
        if value:
            self.sigDebugging.emit(True)
            self.state = DbgState.STEP
            
            self.script = self.get_current_script()
            code,module = self.compile_code(self.script)
            
            if code is None: return
            
            self.breakpoints = [ el[0] for el in self.get_breakpoints()]
            
            try:
                sys.settrace(self.trace_callback)
                exec(code,module.__dict__,module.__dict__)
            except Exception:
                self.sigTraceback.emit(sys.exc_info(),
                                       self.script)
            finally:
                sys.settrace(None)
                
            self.sigDebugging.emit(False)
            self._actions['Run'][1].setChecked(False)
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
