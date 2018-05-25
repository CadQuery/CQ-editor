from PyQt5.QtCore import Qt, QTimer
from PyQt5.QtGui import QColor, QCursor, QIcon, QPainter, QPalette, QPen
from PyQt5.QtWidgets import (QDesktopWidget, QFileDialog, QFontDialog,
                             QGraphicsDropShadowEffect, QLabel, QMainWindow,
                             QMenu, QMessageBox, QShortcut, QSystemTrayIcon,
                             QToolBar, QWidget, QDockWidget, QAction)

import qtawesome as qta

from .widgets.editor import Editor
from .widgets.viewer import OCCViewer
from .widgets.console import ConsoleWidget
from .widgets.object_tree import ObjectTree
from .widgets.traceback_viewer import TracebackPane
from .utils import dock, add_actions
from .mixins import MainMixin


class MainWindow(QMainWindow,MainMixin):
    
    
    def __init__(self,parent=None):
        
        super(MainWindow,self).__init__(parent)
        
        self.viewer = OCCViewer(self)
        self.setCentralWidget(self.viewer.canvas)

        self.prepare_panes()
        self.registerComponent('viewer',self.viewer)
        self.prepare_toolbar()
        self.prepare_menubar()
        
        self.prepare_statusbar()
        self.prepare_actions()
        
        self.components['object_tree'].addLines()
        self.components['console']\
            .push_vars({'viewer' : self.viewer, 'self' : self})
        
        self.fill_dummy()

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
        
        for d in self.docks.values():
            d.show()
        
    def prepare_menubar(self):
        
        menu = self.menuBar()
        
        menu_file = menu.addMenu('&File')
        menu_edit = menu.addMenu('&Edit')
        menu_run = menu.addMenu('&Run')
        menu_view = menu.addMenu('&View')
        menu_help = menu.addMenu('&Help')
        
        #per componenet menu elements
        menus = {'File' : menu_file,
                 'Edit' : menu_edit,
                 'Run'  : menu_run,
                 'View' : menu_view,
                 'Help' : menu_help}
        
        for comp in self.components.values():
            self.prepare_menubar_componenet(menus,
                                            comp.menuActions())
            
        #global menu elements
        menu_view.addSeparator()
        for d in self.findChildren(QDockWidget):
            menu_view.addAction(d.toggleViewAction())
            
        menu_view.addSeparator()
        for t in self.findChildren(QToolBar):
            menu_view.addAction(t.toggleViewAction())
            
        menu_edit.addAction( \
            QAction(qta.icon('fa.cogs'),
                    'Preferences',
                    self,triggered=self.edit_preferences))
        
        menu_help.addAction( \
            QAction(qta.icon('fa.info'),
                    'About',
                    self,triggered=self.about))
    
    def prepare_menubar_componenet(self,menus,comp_menu_dict):
        
        for name,action in comp_menu_dict.items():
            menus[name].addActions(action)
    
    def prepare_toolbar(self):
        
        self.toolbar = QToolBar('Main toolbar',self)
        
        for c in self.components.values():
            add_actions(self.toolbar,c.toolbarActions())
        
        self.addToolBar(self.toolbar)
    
    def prepare_statusbar(self):
        
        self.status_label = QLabel('',parent=self)
        self.statusBar().insertPermanentWidget(0, self.status_label)
        
    def prepare_actions(self):
        
        self.components['editor'].sigRendered\
            .connect(self.components['object_tree'].addObjects)
        self.components['editor'].sigTraceback\
            .connect(self.components['traceback_viewer'].addTraceback)
        
        self.components['object_tree'].sigObjectsAdded\
            .connect(self.components['viewer'].display_many)
        self.components['object_tree'].itemChanged.\
            connect(self.components['viewer'].update_item)
        self.components['object_tree'].sigObjectsRemoved\
            .connect(self.components['viewer'].remove_items)
            
        self.components['traceback_viewer'].sigHighlightLine\
            .connect(self.components['editor'].go_to_line)
        
    def fill_dummy(self):
        
        self.components['editor']\
            .set_text('import cadquery as cq\nresult = cq.Workplane("XY" ).box(3, 3, 0.5).edges("|Z").fillet(0.125)')
            
    def about(self):
        
        pass
    
    def edit_preferences(self):
        
        pass
    
    
if __name__ == "__main__":
    
    import sys
    from PyQt5.QtWidgets import QApplication
    
    app = QApplication(sys.argv)
    win = MainWindow()
    win.show()
    
    sys.exit(app.exec_())
    