import sys, imp
from enum import Enum, auto
from imp import reload
from types import SimpleNamespace

from PyQt5.QtWidgets import (QWidget, QTreeWidget, QTreeWidgetItem, QAction,
                             QLabel, QTableView)
from PyQt5.QtCore import Qt, QObject, pyqtSlot, pyqtSignal, QEventLoop, QAbstractTableModel
from PyQt5 import QtCore

from pyqtgraph.parametertree import Parameter, ParameterTree
from logbook import info
from spyder.utils.icon_manager import icon
from path import Path
from contextlib import ExitStack

import cadquery as cq

from ..mixins import ComponentMixin
from ..utils import layout
from ..cq_utils import find_cq_objects, reload_cq

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

    preferences = Parameter.create(name='Preferences',children=[
        {'name': 'Reload CQ', 'type': 'bool', 'value': True},
        {'name': 'Add script dir to path','type': 'bool', 'value': True},
        {'name': 'Change working dir to script dir','type': 'bool', 'value': True}])


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
                               self,
                               shortcut='F5',
                               triggered=self.render),
                      QAction(icon('debug'),
                             'Debug',
                             self,
                             checkable=True,
                             shortcut='ctrl+F5',
                             triggered=self.debug),
                      QAction(icon('arrow-step-over'),
                             'Step',
                             self,
                             shortcut='ctrl+F10',
                             triggered=lambda: self.debug_cmd(DbgState.STEP)),
                      QAction(icon('arrow-step-in'),
                             'Step in',
                             self,
                             shortcut='ctrl+F11',
                             triggered=lambda: None),
                      QAction(icon('arrow-continue'),
                              'Continue',
                              self,
                              shortcut='ctrl+F12',
                              triggered=lambda: self.debug_cmd(DbgState.CONT))
                      ]}

    def get_current_script(self):

        return self.parent().components['editor'].get_text_with_eol()

    def get_breakpoints(self):

        return self.parent().components['editor'].debugger.get_breakpoints()

    def compile_code(self,cq_script):

        try:
            module = imp.new_module('temp')
            cq_code = compile(cq_script,'<string>','exec')
            return cq_code,module
        except Exception:
            self.sigTraceback.emit(sys.exc_info(),
                                   cq_script)
            return None,None

    def _exec(self, code, locals_dict, globals_dict):

        with ExitStack() as stack:
            p = Path(self.parent().components['editor'].filename).abspath().dirname()
            if self.preferences['Add script dir to path'] and p:
                sys.path.insert(0,p)
                stack.callback(sys.path.remove, p)
            if self.preferences['Change working dir to script dir'] and p:
                stack.enter_context(p)
            exec(code, locals_dict, globals_dict)
            
    def _inject_locals(self,module):
        
        cq_objects = {}
        
        def _show_object(obj,name=None, options={}):

            if name:
                cq_objects.update({name : SimpleNamespace(shape=obj,options=options)})
            else:
                cq_objects.update({str(id(obj)) : SimpleNamespace(shape=obj,options=options)})
                
        def _debug(obj,name=None):
            
            _show_object(obj,name,options=dict(color='red',alpha=0.2))

        module.__dict__['show_object'] = _show_object
        module.__dict__['debug'] = _debug
        module.__dict__['log'] = lambda x: info(str(x))
        module.__dict__['cq'] = cq
        
        return cq_objects, set(module.__dict__)-{'cq'}
    
    def _cleanup_locals(self,module,injected_names):
        
        for name in injected_names: module.__dict__.pop(name)

    @pyqtSlot(bool)
    def render(self):

        if self.preferences['Reload CQ']:
            reload_cq()

        cq_script = self.get_current_script()
        cq_code,module = self.compile_code(cq_script)

        if cq_code is None: return
        
        cq_objects,injected_names = self._inject_locals(module)

        try:
            self._exec(cq_code, module.__dict__, module.__dict__)

            #remove the special methods
            self._cleanup_locals(module,injected_names)
                        
            #collect all CQ objects if no explicit show_object was called
            if len(cq_objects) == 0:
                cq_objects = find_cq_objects(module.__dict__)
            self.sigRendered.emit(cq_objects)
            self.sigTraceback.emit(None,
                                   cq_script)
            self.sigLocals.emit(module.__dict__)
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

            if code is None:
                self.sigDebugging.emit(False)
                self._actions['Run'][1].setChecked(False)
                return
            
            cq_objects,injected_names = self._inject_locals(module)

            self.breakpoints = [ el[0] for el in self.get_breakpoints()]

            #clear possible traceback
            self.sigTraceback.emit(None,
                                   self.script)
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
                
                if len(cq_objects) == 0:
                    cq_objects = find_cq_objects(module.__dict__)
                self.sigRendered.emit(cq_objects)
                
                self._cleanup_locals(module,injected_names)
                self.sigLocals.emit(module.__dict__)
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
