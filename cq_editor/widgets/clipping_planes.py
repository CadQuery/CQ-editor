from PyQt5.QtWidgets import QApplication, QCheckBox, QHBoxLayout, QLabel, QSpinBox, QWidget
from PyQt5.QtCore import Qt, pyqtSlot, pyqtSignal

from qtconsole.rich_jupyter_widget import RichJupyterWidget

from ..mixins import ComponentMixin
from ..utils import layout

class ClippingPlane(QWidget):

    sigClippingPlaneChanged = pyqtSignal(bool,int,bool)

    def __init__(self, parent, dimension):
        super(ClippingPlane, self).__init__(parent)

        self.check = QCheckBox(f"{dimension} Axis", self)
        self.spinbox = QSpinBox(self)
        self.spinbox.setValue(50)
        self.spinbox.setRange(1, 256)
        self.invert = QCheckBox("Invert", self)

        self.group = layout(self,(self.spinbox,self.invert),layout_type=QHBoxLayout, spacing=5)

        layout(self,(self.check,self.group),layout_type=QHBoxLayout, top_widget=self)

        self.group.setDisabled(True)
        self.check.stateChanged.connect(self.handleChecked)
        self.invert.stateChanged.connect(self.handleInvertChecked)
        self.spinbox.valueChanged.connect(self.handlePositionChanged)

    def handleChecked(self, arg):
        self.group.setDisabled(not self.check.checkState())
        self.emitState()

    def handleInvertChecked(self, arg):
        self.emitState()

    def handlePositionChanged(self, arg):
        self.emitState()

    def emitState(self):
        self.sigClippingPlaneChanged.emit(self.check.checkState(), self.spinbox.value(), self.invert.checkState())

class ClippingPlanes(QWidget,ComponentMixin):
    
    name = 'Clipping Planes'

    sigClippingPlaneChanged = pyqtSignal(str,bool,int,bool)

    def __init__(self, parent):
        super(ClippingPlanes, self).__init__(parent)

        self.x = ClippingPlane(self, "X")
        self.x.sigClippingPlaneChanged.connect(self.onXPlaneChanged)
        self.y = ClippingPlane(self, "Y")
        self.y.sigClippingPlaneChanged.connect(self.onYPlaneChanged)
        self.z = ClippingPlane(self, "Z")
        self.z.sigClippingPlaneChanged.connect(self.onZPlaneChanged)

        layout(self,(self.x,self.y,self.z),top_widget=self)

    @pyqtSlot(bool,int,bool)
    def onXPlaneChanged(self,enabled,val,inverted):
        self.sigClippingPlaneChanged.emit("X",enabled,val,inverted)

    @pyqtSlot(bool,int,bool)
    def onYPlaneChanged(self,enabled,val,inverted):
        self.sigClippingPlaneChanged.emit("Y",enabled,val,inverted)

    @pyqtSlot(bool,int,bool)
    def onZPlaneChanged(self,enabled,val,inverted):
        self.sigClippingPlaneChanged.emit("Z",enabled,val,inverted)


