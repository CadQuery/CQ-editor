from PyQt5.QtCore import Qt, QTimer
from PyQt5.QtGui import QColor, QCursor, QIcon, QPainter, QPalette, QPen
from PyQt5.QtWidgets import (QDesktopWidget, QFileDialog, QFontDialog,
                             QGraphicsDropShadowEffect, QLabel, QMainWindow,
                             QMenu, QMessageBox, QShortcut, QSystemTrayIcon,
                             QToolBar, QWidget, QDockWidget)

from .widgets.editor import Editor
from .widgets.viewer import OCCViewer
from .widgets.console import ConsoleWidget
from .widgets.object_tree import ObjectTree
from .widgets.traceback_viewer import TracebackTree
from .utils import dock


class MainWindow(QMainWindow):
    
    
    def __init__(self,parent=None):
        
        super(MainWindow,self).__init__(parent)
        
        self.viewer = OCCViewer(self)
        self.setCentralWidget(self.viewer.canvas)

        self.prepare_panes()
        self.prepare_toolbar()
        self.prepare_menubar()
        
        self.prepare_statusbar()
        self.prepare_actions()
        
        self.object_tree.addLines()
        
        self.console.push_vars({'viewer' : self.viewer, 'self' : self})
        self.fill_dummy()

    def prepare_panes(self):
        
        self.editor = Editor(self)
        self.editor_dock = dock(self.editor,
                                'Editor',
                                self,
                                defaultArea='left')
        self.editor_dock.show()
        
        self.object_tree = ObjectTree(self)
        self.objects_dock = dock(self.object_tree,
                                'Objects',
                                self,
                                defaultArea='right')
        self.objects_dock.show()
        
        self.console = ConsoleWidget(self)
        self.console_dock = dock(self.console,
                                 'Console',
                                 self,
                                 defaultArea='bottom')
        self.console_dock.show()
        
        self.traceback_viewer = TracebackTree(self)
        self.traceback_dock = dock(self.traceback_viewer,
                                   'Current traceback',
                                   self,
                                   defaultArea='bottom')
        self.traceback_dock.show()
        
    def prepare_menubar(self):
        
        menu = self.menuBar()
        
        menu_file = menu.addMenu('&File')
        
        
        menu_edit = menu.addMenu('&Edit')
        
        
        menu_edit = menu.addMenu('&Run')
        
        
        menu_view = menu.addMenu('&View')
        for d in self.findChildren(QDockWidget):
            menu_view.addAction(d.toggleViewAction())
        
        menu_view.addSeparator()
        for t in self.findChildren(QToolBar):
            menu_view.addAction(t.toggleViewAction())
        
        menu_help = menu.addMenu('&Help')
    
    def prepare_toolbar(self):
        
        self.toolbar = QToolBar('Main toolbar',self)
        
        self.toolbar.addActions(self.editor.actions())
        
        self.toolbar.addSeparator()
        self.toolbar.addActions(self.viewer.actions())
        
        self.addToolBar(self.toolbar)
    
    def prepare_statusbar(self):
        
        self.status_label = QLabel('',parent=self)
        self.statusBar().insertPermanentWidget(0, self.status_label)
        
    def prepare_actions(self):
        
        self.editor.sigRendered.connect(self.object_tree.addObjects)
        self.editor.sigTraceback.connect(self.traceback_viewer.addTraceback)
        
        self.object_tree.sigObjectsAdded.connect(self.viewer.display_many)
        self.object_tree.itemChanged.connect(self.viewer.update_item)
        self.object_tree.sigObjectsRemoved.connect(self.viewer.remove_items)
        
    def fill_dummy(self):
        
        self.editor.set_text('import cadquery as cq\nresult = cq.Workplane("XY" ).box(3, 3, 0.5).edges("|Z").fillet(0.125)')
    
    
if __name__ == "__main__":
    
    import sys
    from PyQt5.QtWidgets import QApplication
    
    app = QApplication(sys.argv)
    win = MainWindow()
    win.show()
    
    sys.exit(app.exec_())
    