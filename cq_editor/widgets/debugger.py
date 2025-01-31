import sys
from contextlib import ExitStack, contextmanager
from enum import Enum, auto
from types import SimpleNamespace, FrameType, ModuleType
from typing import List
from bdb import BdbQuit
from inspect import currentframe

import cadquery as cq
from PyQt5 import QtCore
from PyQt5.QtCore import Qt, QObject, pyqtSlot, pyqtSignal, QEventLoop, QAbstractTableModel
from PyQt5.QtWidgets import QAction, QTableView

from logbook import info
from path import Path
from pyqtgraph.parametertree import Parameter
from spyder.utils.icon_manager import icon
from random import randrange as rrr,seed

from ..cq_utils import find_cq_objects, reload_cq
from ..mixins import ComponentMixin

DUMMY_FILE = '<cq_editor-string>'


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
        {'name': 'Reload CQ', 'type': 'bool', 'value': False},
        {'name': 'Add script dir to path','type': 'bool', 'value': True},
        {'name': 'Change working dir to script dir','type': 'bool', 'value': True},
        {'name': 'Reload imported modules', 'type': 'bool', 'value': True},
    ])


    sigRendered = pyqtSignal(dict)
    sigLocals = pyqtSignal(dict)
    sigTraceback = pyqtSignal(object,str)

    sigFrameChanged = pyqtSignal(object)
    sigLineChanged = pyqtSignal(int)
    sigLocalsChanged = pyqtSignal(dict)
    sigCQChanged = pyqtSignal(dict,bool)
    sigDebugging = pyqtSignal(bool)

    _frames : List[FrameType]
    _stop_debugging : bool

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
                             triggered=lambda: self.debug_cmd(DbgState.STEP_IN)),
                      QAction(icon('arrow-continue'),
                              'Continue',
                              self,
                              shortcut='ctrl+F12',
                              triggered=lambda: self.debug_cmd(DbgState.CONT))
                      ]}

        self._frames = []
        self._stop_debugging = False

    def get_current_script(self):

        return self.parent().components['editor'].get_text_with_eol()
    
    def get_current_script_path(self):
        
        filename = self.parent().components["editor"].filename
        if filename:
            return Path(filename).absolute()

    def get_breakpoints(self):

        return self.parent().components['editor'].debugger.get_breakpoints()

    def compile_code(self, cq_script, cq_script_path=None):

        try:
            module = ModuleType('__cq_main__')
            if cq_script_path:
                module.__dict__["__file__"] = cq_script_path
            cq_code = compile(cq_script, DUMMY_FILE, 'exec')
            return cq_code, module
        except Exception:
            self.sigTraceback.emit(sys.exc_info(), cq_script)
            return None, None

    def _exec(self, code, locals_dict, globals_dict):

        with ExitStack() as stack:
            p = (self.get_current_script_path() or Path("")).absolute().dirname()

            if self.preferences['Add script dir to path'] and p.exists():
                sys.path.insert(0,p)
                stack.callback(sys.path.remove, p)
            if self.preferences['Change working dir to script dir'] and p.exists():
                stack.enter_context(p)
            if self.preferences['Reload imported modules']:
                stack.enter_context(module_manager())

            exec(code, locals_dict, globals_dict)

    @staticmethod
    def _rand_color(alpha = 0., cfloat=False):
        #helper function to generate a random color dict
        #for CQ-editor's show_object function
        lower = 10
        upper = 100 #not too high to keep color brightness in check
        if cfloat: #for two output types depending on need
            return (
                    (rrr(lower,upper)/255),
                    (rrr(lower,upper)/255),
                    (rrr(lower,upper)/255),
                    alpha,
                    )
        return {"alpha": alpha,
                "color": (
                          rrr(lower,upper),
                          rrr(lower,upper),
                          rrr(lower,upper),
                         )}

    def _inject_locals(self,module):

        cq_objects = {}

        def _show_object(obj,name=None, options={}):

            if name:
                cq_objects.update({name : SimpleNamespace(shape=obj,options=options)})
            else:
                #get locals of the enclosing scope
                d = currentframe().f_back.f_locals

                #try to find the name
                try:
                    name = list(d.keys())[list(d.values()).index(obj)]
                except ValueError:
                    #use id if not found
                    name = str(id(obj))

                cq_objects.update({name : SimpleNamespace(shape=obj,options=options)})

        def _debug(obj,name=None):

            _show_object(obj,name,options=dict(color='red',alpha=0.2))

        module.__dict__['show_object'] = _show_object
        module.__dict__['debug'] = _debug
        module.__dict__['rand_color'] = self._rand_color
        module.__dict__['log'] = lambda x: info(str(x))
        module.__dict__['cq'] = cq

        return cq_objects, set(module.__dict__)-{'cq'}

    def _cleanup_locals(self,module,injected_names):

        for name in injected_names: module.__dict__.pop(name)

    @pyqtSlot(bool)
    def render(self):

        seed(59798267586177)
        if self.preferences['Reload CQ']:
            reload_cq()

        cq_script = self.get_current_script()
        cq_script_path = self.get_current_script_path()
        cq_code,module = self.compile_code(cq_script, cq_script_path)

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
            exc_info = sys.exc_info()
            sys.last_traceback = exc_info[-1]
            self.sigTraceback.emit(exc_info, cq_script)

    @property
    def breakpoints(self):
        return [ el[0] for el in self.get_breakpoints()]

    @pyqtSlot(bool)
    def debug(self,value):

        # used to stop the debugging session early
        self._stop_debugging = False

        if value:
            self.previous_trace = previous_trace = sys.gettrace()

            self.sigDebugging.emit(True)
            self.state = DbgState.STEP

            self.script = self.get_current_script()
            cq_script_path = self.get_current_script_path()
            code,module = self.compile_code(self.script, cq_script_path)

            if code is None:
                self.sigDebugging.emit(False)
                self._actions['Run'][1].setChecked(False)
                return

            cq_objects,injected_names = self._inject_locals(module)

            #clear possible traceback
            self.sigTraceback.emit(None,
                                   self.script)

            try:
                sys.settrace(self.trace_callback)
                exec(code,module.__dict__,module.__dict__)
            except BdbQuit:
                pass
            except Exception:
                exc_info = sys.exc_info()
                sys.last_traceback = exc_info[-1]
                self.sigTraceback.emit(exc_info,
                                       self.script)
            finally:
                sys.settrace(previous_trace)
                self.sigDebugging.emit(False)
                self._actions['Run'][1].setChecked(False)

                if len(cq_objects) == 0:
                    cq_objects = find_cq_objects(module.__dict__)
                self.sigRendered.emit(cq_objects)

                self._cleanup_locals(module,injected_names)
                self.sigLocals.emit(module.__dict__)

                self._frames = []
                self.inner_event_loop.exit(0)
        else:
            self._stop_debugging = True
            self.inner_event_loop.exit(0)

    def debug_cmd(self,state=DbgState.STEP):

        self.state = state
        self.inner_event_loop.exit(0)


    def trace_callback(self,frame,event,arg):

        filename = frame.f_code.co_filename

        if filename==DUMMY_FILE:
            if not self._frames:
                self._frames.append(frame)
            self.trace_local(frame,event,arg)
            return self.trace_callback

        else:
            return None

    def trace_local(self,frame,event,arg):

        lineno = frame.f_lineno

        if event in (DbgEevent.LINE,):
            if (self.state in (DbgState.STEP, DbgState.STEP_IN) and frame is self._frames[-1]) \
            or (lineno in self.breakpoints):

                if lineno in self.breakpoints:
                    self._frames.append(frame)

                self.sigLineChanged.emit(lineno)
                self.sigFrameChanged.emit(frame)
                self.sigLocalsChanged.emit(frame.f_locals)
                self.sigCQChanged.emit(find_cq_objects(frame.f_locals),True)

                self.inner_event_loop.exec_()

        elif event in (DbgEevent.RETURN):
            self.sigLocalsChanged.emit(frame.f_locals)
            self._frames.pop()

        elif event == DbgEevent.CALL:
            func_filename = frame.f_code.co_filename
            if self.state == DbgState.STEP_IN and func_filename == DUMMY_FILE:
                self.sigLineChanged.emit(lineno)
                self.sigFrameChanged.emit(frame)
                self.state = DbgState.STEP
                self._frames.append(frame)

        if self._stop_debugging:
            raise BdbQuit #stop debugging if requested


@contextmanager
def module_manager():
    """ unloads any modules loaded while the context manager is active """
    loaded_modules = set(sys.modules.keys())

    try:
        yield
    finally:
        new_modules = set(sys.modules.keys()) - loaded_modules
        for module_name in new_modules:
            del sys.modules[module_name]
