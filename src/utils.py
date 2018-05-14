from PyQt5 import QtCore, QtWidgets

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