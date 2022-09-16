import requests

from pkg_resources import parse_version

from PyQt5 import QtCore, QtWidgets
from PyQt5.QtGui import QDesktopServices
from PyQt5.QtCore import QUrl
from PyQt5.QtWidgets import QFileDialog, QMessageBox

DOCK_POSITIONS = {'right'   : QtCore.Qt.RightDockWidgetArea,
                  'left'    : QtCore.Qt.LeftDockWidgetArea,
                  'top'     : QtCore.Qt.TopDockWidgetArea,
                  'bottom'  : QtCore.Qt.BottomDockWidgetArea}

def layout(parent,items,
           top_widget = None,
           layout_type = QtWidgets.QVBoxLayout,
           margin = 2,
           spacing = 0):
    
    if not top_widget:
        top_widget = QtWidgets.QWidget(parent)
        top_widget_was_none = True
    else:
        top_widget_was_none = False
    layout = layout_type(top_widget)
    top_widget.setLayout(layout)
    
    for item in items: layout.addWidget(item)

    layout.setSpacing(spacing)
    layout.setContentsMargins(margin,margin,margin,margin)
    
    if top_widget_was_none:
        return top_widget
    else:
        return layout
        
def splitter(items,
             stretch_factors = None,
             orientation=QtCore.Qt.Horizontal):
    
    sp = QtWidgets.QSplitter(orientation)
    
    for item in items: sp.addWidget(item)
    
    if stretch_factors:
        for i,s in enumerate(stretch_factors):
            sp.setStretchFactor(i,s)
        
    
    return sp

def dock(widget,
         title,
         parent,
         allowedAreas = QtCore.Qt.AllDockWidgetAreas,
         defaultArea = 'right',
         name=None,
         icon = None):
    
    dock = QtWidgets.QDockWidget(title,parent,objectName=title)
    
    if name: dock.setObjectName(name)
    if icon: dock.toggleViewAction().setIcon(icon)
    
    dock.setAllowedAreas(allowedAreas)
    dock.setWidget(widget)
    action = dock.toggleViewAction()
    action.setText(title)
    
    dock.setFeatures(QtWidgets.QDockWidget.DockWidgetFeatures(\
                     QtWidgets.QDockWidget.AllDockWidgetFeatures))
    
    parent.addDockWidget(DOCK_POSITIONS[defaultArea],
                         dock)
    
    return dock

def add_actions(menu,actions):
    
    if len(actions) > 0:
        menu.addActions(actions)
        menu.addSeparator()
        
def open_url(url):
    
     QDesktopServices.openUrl(QUrl(url))
     
def about_dialog(parent,title,text):
    
    QtWidgets.QMessageBox.about(parent,title,text)
    
def get_save_filename(suffix):
    
    rv,_ = QFileDialog.getSaveFileName(filter='*.{}'.format(suffix))
    if rv != '' and not rv.endswith(suffix): rv += '.'+suffix
    
    return rv

def get_open_filename(suffix, curr_dir):
    
    rv,_ = QFileDialog.getOpenFileName(directory=curr_dir, filter='*.{}'.format(suffix))
    if rv != '' and not rv.endswith(suffix): rv += '.'+suffix
    
    return rv

def check_gtihub_for_updates(parent,
                             mod,
                             github_org='cadquery',
                             github_proj='cadquery'):
    
    url = f'https://api.github.com/repos/{github_org}/{github_proj}/releases'    
    resp = requests.get(url).json()
    
    newer = [el['tag_name'] for el in resp if not el['draft'] and \
             parse_version(el['tag_name']) > parse_version(mod.__version__)]    
    
    if newer:
        title='Updates available'
        text=f'There are newer versions of {github_proj} ' \
             f'available on github:\n' + '\n'.join(newer)
             
    else:
        title='No updates available'
        text=f'You are already using the latest version of {github_proj}'
        
    QtWidgets.QMessageBox.about(parent,title,text)
    
def confirm(parent,title,msg):
    
    rv = QMessageBox.question(parent, title, msg, QMessageBox.Yes, QMessageBox.No)
    
    return True if rv == QMessageBox.Yes else False
