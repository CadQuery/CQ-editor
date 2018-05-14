from PyQt5 import QtCore, QtWidgets, QtGui
Qt = QtCore.Qt

class CenteredSquareLayout(QtWidgets.QLayout):

    def __init__(self, parent):
        
        super(CenteredSquareLayout,self).__init__(parent)

        self.setContentsMargins(0, 0, 0, 0)
        self.items = []

    def addLayout(self, layout):

        self.addChildLayout(layout)
        self.addItem(layout)

    def __del__(self):

        item = self.takeAt(0)
        while item:
            item = self.takeAt(0)

    def addItem(self, item):

        self.items.append(item)

    def count(self):

        return len(self.items)

    def itemAt(self, index):

        if index >= 0 and index < len(self.items):
            return self.items[index]

        return None

    def takeAt(self, index):
        if index >= 0 and index < len(self.items):
            return self.items.pop(index)

        return None

    def setGeometry(self, rect):
        
        super(CenteredSquareLayout,self).setGeometry(rect)

        w,h = rect.width(), rect.height()
        x,y = rect.x(), rect.y()
        
        if w>=h:
            w_new = h
            h_new = h
            
            x_new = (w-w_new)/2
            y_new = y
        else:
            w_new = w
            h_new = w
            
            x_new = x
            y_new = (h-h_new)/2

        for item in self.items:
            item.setGeometry(QtCore.QRect(x_new,y_new,w_new,h_new))
            
    def sizeHint(self):
                           
        return self.minimumSize()                     
                                                      
    def minimumSize(self):
                          
        size = QtCore.QSize()                                
                                                      
        for item in self.items:                    
            size = size.expandedTo(item.minimumSize())
                                                      
        margin, _, _, _ = self.getContentsMargins()   
                                                      
        size += QtCore.QSize(2 * margin, 2 * margin)         
        return size

def vspacer():
    
    return QtWidgets.QSpacerItem(1, 1,
                                 QtWidgets.QSizePolicy.Minimum,
                                 QtWidgets.QSizePolicy.Expanding)
    
def hspacer():
    
    return QtWidgets.QSpacerItem(1, 1,
                                 QtWidgets.QSizePolicy.Expanding,
                                 QtWidgets.QSizePolicy.Minimum)


def set_margins(layout,margin,spacing):
    
    layout.setContentsMargins(margin,margin,margin,margin)
    layout.setSpacing(spacing)

def make_layout(widgets,
                parent=None,
                layout=None,
                margin=2,
                spacing=2,
                spacer=None):
    
    if parent is None:
        parent = QtWidgets.QWidget()
        
    if layout is None:
        layout = QtWidgets.QVBoxLayout(parent)
    
    parent.setLayout(layout)
    layout.setParent(parent)
        
    set_margins(layout,margin,spacing)
    
    for w in widgets:
        w.setParent(parent)
        layout.addWidget(w)

    if spacer:
        layout.addItem(spacer)
        
    return parent

def hbox(widgets,**kwargs):
    
    return make_layout(widgets,layout=QtWidgets.QHBoxLayout(),**kwargs)

 
def vbox(widgets,**kwargs):
    
    return make_layout(widgets,layout=QtWidgets.QVBoxLayout(),**kwargs)

 
def split(widgets,
          sizes,
          orientation,
          parent=None,
          layout=None,
          margin=2,
          spacing=2,
          name=None):
    
    if parent is None:
        return_parent = True
        parent = QtWidgets.QWidget()
    else:
        return_parent = False
    
    if layout is None:
        layout = QtWidgets.QVBoxLayout(parent)
    
    splitter = QtWidgets.QSplitter(orientation,
                                   parent=parent)
    
    if not layout is None:
        splitter.setObjectName(name)
    
    for i,(w,s) in enumerate(zip(widgets,sizes)):
        splitter.addWidget(w)
        splitter.setStretchFactor(i,s)
    
    set_margins(layout,margin,spacing)    
    layout.addWidget(splitter)
    
    if return_parent:
        return parent
    else:
        return splitter

 
def hsplit(widgets,sizes,**kwargs):
    
    return split(widgets,sizes,Qt.Horizontal,**kwargs)

 
def vsplit(widgets,sizes,**kwargs):
    
    return split(widgets,sizes,Qt.Vertical,**kwargs)

 
def tab(labels,widgets):
    
    tabs = QtWidgets.QTabWidget()
    
    for l,w in zip(labels,widgets):
        tabs.addTab(w, l)
        
    return tabs

 
def form(labels,widgets,parent=None,margin=2,spacing=2):
    
    if not parent:
        parent = QtWidgets.QWidget()
    
    form = QtWidgets.QFormLayout(parent)
    
    for l,w in zip(labels,widgets):
        form.addRow(l,w)
        
    set_margins(form,margin,spacing)
        
    return parent

 
def stack(widgets,parent):
    
    stacked = QtWidgets.QStackedWidget(parent)
    
    for w in widgets:
        w.setParent(stacked)
        stacked.addWidget(w)
        
    return stacked

def group(label,widgets,layout=vbox,parent=None):
    
    box = QtWidgets.QGroupBox(label,parent=parent)
    layout(widgets, parent=box)
    
    return box

def centered(widget,parent=None):
    
    if not parent:
        parent = QtWidgets.QWidget()
        
    layout  = CenteredSquareLayout(parent)
    parent.setLayout(layout)
    layout.addWidget(widget)
    
    return parent

def frame(widget,layout=vbox,parent=None):
    
    frame = QtWidgets.QFrame(parent=parent)
    frame.setFrameStyle(frame.StyledPanel)
    frame.setLineWidth(1)
    frame.setContentsMargins(1,1,1,1)
    frame.setAutoFillBackground(True)
    frame.setSizePolicy(QtWidgets.QSizePolicy.Expanding,
                        QtWidgets.QSizePolicy.Expanding)
    frame.setBackgroundRole(QtGui.QPalette.Base)
    
    layout((widget,), parent=frame)
    
    return frame

def dock(widget,title,
         parent=None,
         allowedAreas = QtCore.Qt.AllDockWidgetAreas,
         defaultArea = None,
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
    
    return dock
 
def action(name,
           parent,
           callback,
           checkable = False,
           icon = None):
    
    return QtWidgets.QAction(name,
                             parent,
                             triggered=callback,
                             checkable=True,
                             icon=icon)