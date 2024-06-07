import sys

from PyQt5.QtWidgets import (QLabel, QMainWindow, QToolBar, QDockWidget, QAction)
from logbook import Logger
import cadquery as cq

from .widgets.editor import Editor
from .widgets.viewer import OCCViewer
from .widgets.console import ConsoleWidget
from .widgets.object_tree import ObjectTree
from .widgets.traceback_viewer import TracebackPane
from .widgets.debugger import Debugger, LocalsView
from .widgets.cq_object_inspector import CQObjectInspector
from .widgets.log import LogViewer

from . import __version__
from .utils import dock, add_actions, open_url, about_dialog, check_gtihub_for_updates, confirm
from .mixins import MainMixin
from .icons import icon
from .preferences import PreferencesWidget


class MainWindow(QMainWindow,MainMixin):

    name = 'CQ-Editor'
    org = 'CadQuery'

    def __init__(self,parent=None, filename=None):

        super(MainWindow,self).__init__(parent)
        MainMixin.__init__(self)

        self.setWindowIcon(icon('app'))

        # Windows workaround - makes the correct task bar icon show up.
        if sys.platform == "win32":
            import ctypes
            myappid = 'cq-editor' # arbitrary string
            ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID(myappid)

        self.viewer = OCCViewer(self)
        self.setCentralWidget(self.viewer.canvas)

        self.prepare_panes()
        self.registerComponent('viewer',self.viewer)
        self.prepare_toolbar()
        self.prepare_menubar()

        self.prepare_statusbar()
        self.prepare_actions()
        
        self.components['object_tree'].addLines()

        self.prepare_console()

        self.fill_dummy()

        self.setup_logging()

        self.restorePreferences()
        self.restoreWindow()

        if filename:
            self.components['editor'].load_from_file(filename)

        self.restoreComponentState()

    def closeEvent(self,event):

        self.saveWindow()
        self.savePreferences()
        self.saveComponentState()

        if self.components['editor'].document().isModified():

            rv = confirm(self, 'Confirm close', 'Close without saving?')
            
            if rv:
                event.accept()
                super(MainWindow,self).closeEvent(event)
            else:
                event.ignore()
        else:
            super(MainWindow,self).closeEvent(event)

    def prepare_panes(self):

        self.registerComponent('editor',
                               Editor(self),
                               lambda c : dock(c,
                                               'Editor',
                                               self,
                                               defaultArea='left'))

        self.registerComponent('object_tree',
                               ObjectTree(self),
                               lambda c: dock(c,
                                              'Objects',
                                              self,
                                              defaultArea='right'))

        self.registerComponent('console',
                               ConsoleWidget(self),
                               lambda c: dock(c,
                                              'Console',
                                              self,
                                              defaultArea='bottom'))

        self.registerComponent('traceback_viewer',
                               TracebackPane(self),
                               lambda c: dock(c,
                                              'Current traceback',
                                              self,
                                              defaultArea='bottom'))

        self.registerComponent('debugger',Debugger(self))

        self.registerComponent('variables_viewer',LocalsView(self),
                               lambda c: dock(c,
                                              'Variables',
                                              self,
                                              defaultArea='right'))

        self.registerComponent('cq_object_inspector',
                               CQObjectInspector(self),
                               lambda c: dock(c,
                                              'CQ object inspector',
                                              self,
                                              defaultArea='right'))
        self.registerComponent('log',
                               LogViewer(self),
                               lambda c: dock(c,
                                              'Log viewer',
                                              self,
                                              defaultArea='bottom'))

        for d in self.docks.values():
            d.show()

    def prepare_menubar(self):

        menu = self.menuBar()

        menu_file = menu.addMenu('&File')
        menu_edit = menu.addMenu('&Edit')
        menu_tools = menu.addMenu('&Tools')
        menu_run = menu.addMenu('&Run')
        menu_view = menu.addMenu('&View')
        menu_help = menu.addMenu('&Help')

        #per component menu elements
        menus = {'File' : menu_file,
                 'Edit' : menu_edit,
                 'Run'  : menu_run,
                 'Tools': menu_tools,
                 'View' : menu_view,
                 'Help' : menu_help}

        for comp in self.components.values():
            self.prepare_menubar_component(menus,
                                            comp.menuActions())

        #global menu elements
        menu_view.addSeparator()
        for d in self.findChildren(QDockWidget):
            menu_view.addAction(d.toggleViewAction())

        menu_view.addSeparator()
        for t in self.findChildren(QToolBar):
            menu_view.addAction(t.toggleViewAction())

        menu_edit.addAction( \
            QAction(icon('preferences'),
                    'Preferences',
                    self,triggered=self.edit_preferences))

        menu_help.addAction( \
            QAction(icon('help'),
                    'Documentation',
                    self,triggered=self.documentation))

        menu_help.addAction( \
             QAction('CQ documentation',
                    self,triggered=self.cq_documentation))

        menu_help.addAction( \
            QAction(icon('about'),
                    'About',
                    self,triggered=self.about))
        
        menu_help.addAction( \
            QAction('Check for CadQuery updates',
                    self,triggered=self.check_for_cq_updates))

    def prepare_menubar_component(self,menus,comp_menu_dict):

        for name,action in comp_menu_dict.items():
            menus[name].addActions(action)

    def prepare_toolbar(self):

        self.toolbar = QToolBar('Main toolbar',self,objectName='Main toolbar')

        for c in self.components.values():
            add_actions(self.toolbar,c.toolbarActions())

        self.addToolBar(self.toolbar)

    def prepare_statusbar(self):

        self.status_label = QLabel('',parent=self)
        self.statusBar().insertPermanentWidget(0, self.status_label)

    def prepare_actions(self):

        self.components['debugger'].sigRendered\
            .connect(self.components['object_tree'].addObjects)
        self.components['debugger'].sigTraceback\
            .connect(self.components['traceback_viewer'].addTraceback)
        self.components['debugger'].sigLocals\
            .connect(self.components['variables_viewer'].update_frame)
        self.components['debugger'].sigLocals\
            .connect(self.components['console'].push_vars)

        self.components['object_tree'].sigObjectsAdded[list]\
            .connect(self.components['viewer'].display_many)
        self.components['object_tree'].sigObjectsAdded[list,bool]\
            .connect(self.components['viewer'].display_many)
        self.components['object_tree'].sigItemChanged.\
            connect(self.components['viewer'].update_item)
        self.components['object_tree'].sigObjectsRemoved\
            .connect(self.components['viewer'].remove_items)
        self.components['object_tree'].sigCQObjectSelected\
            .connect(self.components['cq_object_inspector'].setObject)
        self.components['object_tree'].sigObjectPropertiesChanged\
            .connect(self.components['viewer'].redraw)
        self.components['object_tree'].sigAISObjectsSelected\
            .connect(self.components['viewer'].set_selected)

        self.components['viewer'].sigObjectSelected\
            .connect(self.components['object_tree'].handleGraphicalSelection)

        self.components['traceback_viewer'].sigHighlightLine\
            .connect(self.components['editor'].go_to_line)

        self.components['cq_object_inspector'].sigDisplayObjects\
            .connect(self.components['viewer'].display_many)
        self.components['cq_object_inspector'].sigRemoveObjects\
            .connect(self.components['viewer'].remove_items)
        self.components['cq_object_inspector'].sigShowPlane\
            .connect(self.components['viewer'].toggle_grid)
        self.components['cq_object_inspector'].sigShowPlane[bool,float]\
            .connect(self.components['viewer'].toggle_grid)
        self.components['cq_object_inspector'].sigChangePlane\
            .connect(self.components['viewer'].set_grid_orientation)

        self.components['debugger'].sigLocalsChanged\
            .connect(self.components['variables_viewer'].update_frame)
        self.components['debugger'].sigLineChanged\
            .connect(self.components['editor'].go_to_line)
        self.components['debugger'].sigDebugging\
            .connect(self.components['object_tree'].stashObjects)
        self.components['debugger'].sigCQChanged\
            .connect(self.components['object_tree'].addObjects)
        self.components['debugger'].sigTraceback\
            .connect(self.components['traceback_viewer'].addTraceback)

        # trigger re-render when file is modified externally or saved
        self.components['editor'].triggerRerender \
            .connect(self.components['debugger'].render)
        self.components['editor'].sigFilenameChanged\
            .connect(self.handle_filename_change)

    def prepare_console(self):

        console = self.components['console']
        obj_tree = self.components['object_tree']
        
        #application related items
        console.push_vars({'self' : self})

        #CQ related items
        console.push_vars({'show' : obj_tree.addObject,
                           'show_object' : obj_tree.addObject,
                           'rand_color' : self.components['debugger']._rand_color,
                           'cq' : cq,
                           'log' : Logger(self.name).info})

    def fill_dummy(self):

        self.components['editor']\
            .set_text('import cadquery as cq\nresult = cq.Workplane("XY" ).box(3, 3, 0.5).edges("|Z").fillet(0.125)')

    def setup_logging(self):

        from logbook.compat import redirect_logging
        from logbook import INFO, Logger

        redirect_logging()
        self.components['log'].handler.level = INFO
        self.components['log'].handler.push_application()

        self._logger = Logger(self.name)

        def handle_exception(exc_type, exc_value, exc_traceback):

            if issubclass(exc_type, KeyboardInterrupt):
                sys.__excepthook__(exc_type, exc_value, exc_traceback)
                return

            self._logger.error("Uncaught exception occurred",
                               exc_info=(exc_type, exc_value, exc_traceback))

        sys.excepthook = handle_exception


    def edit_preferences(self):

        prefs = PreferencesWidget(self,self.components)
        prefs.exec_()

    def about(self):

        about_dialog(
            self,
            f'About CQ-editor',
            f'PyQt GUI for CadQuery.\nVersion: {__version__}.\nSource Code: https://github.com/CadQuery/CQ-editor',
        )
        
    def check_for_cq_updates(self):
        
        check_gtihub_for_updates(self,cq)

    def documentation(self):

        open_url('https://github.com/CadQuery')

    def cq_documentation(self):

        open_url('https://cadquery.readthedocs.io/en/latest/')

    def handle_filename_change(self, fname):

        new_title = fname if fname else "*"
        self.setWindowTitle(f"{self.name}: {new_title}")

if __name__ == "__main__":

    pass
