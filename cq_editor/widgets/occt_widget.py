import math
from sys import platform

from OCP.AIS import AIS_InteractiveContext, AIS_DisplayMode
from OCP.Aspect import Aspect_DisplayConnection, Aspect_TypeOfTriedronPosition
from OCP.OpenGl import OpenGl_GraphicDriver
from OCP.Quantity import Quantity_Color
from OCP.V3d import V3d_Viewer
from OCP.gp import gp_Trsf, gp_Ax1, gp_Dir
from PyQt5.QtCore import pyqtSignal, Qt, QPoint
from PyQt5.QtWidgets import QWidget


class OCCTWidget(QWidget):
    
    sigObjectSelected = pyqtSignal(list)
    
    def __init__(self, parent=None, *,
                 orbital_rotation: bool = True,
                 rotate_step: float = 0.008,
                 zoom_step: float = 0.1):
        
        super(OCCTWidget, self).__init__(parent)
        
        self.setAttribute(Qt.WA_NativeWindow)
        self.setAttribute(Qt.WA_PaintOnScreen)
        self.setAttribute(Qt.WA_NoSystemBackground)
        
        self._initialized = False
        self._needs_update = False
        self._old_pos = QPoint(0, 0)
        self._rotate_step = rotate_step
        self._zoom_step = zoom_step
        self._orbital_rotation = orbital_rotation
        
        # OCCT secific things
        self.display_connection = Aspect_DisplayConnection()
        self.graphics_driver = OpenGl_GraphicDriver(self.display_connection)
        
        self.viewer = V3d_Viewer(self.graphics_driver)
        self.view = self.viewer.CreateView()
        self.context = AIS_InteractiveContext(self.viewer)
        
        # Trihedorn, lights, etc
        self.prepare_display()

    def set_orbital_rotation(self, new_value: bool):
        if self._orbital_rotation != new_value:
            self._orbital_rotation = new_value
            if self._orbital_rotation:
                self.view.SetUp(0, 0, 1)

    def prepare_display(self):
        
        view = self.view
        
        params = view.ChangeRenderingParams()
        params.NbMsaaSamples = 8
        params.IsAntialiasingEnabled = True
        
        view.TriedronDisplay(
            Aspect_TypeOfTriedronPosition.Aspect_TOTP_RIGHT_LOWER,
            Quantity_Color(), 0.1)
        
        viewer = self.viewer
        
        viewer.SetDefaultLights()
        viewer.SetLightOn()
        
        ctx = self.context
        
        ctx.SetDisplayMode(AIS_DisplayMode.AIS_Shaded, True)
        ctx.DefaultDrawer().SetFaceBoundaryDraw(True)
        
    def wheelEvent(self, event):

        # dividing by 120 gets number of notches on a typical scroll wheel.
        # See QWheelEvent documentation
        delta_notches = event.angleDelta().y() / 120
        direction = math.copysign(1, delta_notches)
        factor = (1 + self._zoom_step * direction) ** abs(delta_notches)

        self.view.SetZoom(factor)
        
    def mousePressEvent(self, event):
        
        pos = event.pos()

        if event.button() == Qt.LeftButton:
            if not self._orbital_rotation:
                self.view.StartRotation(pos.x(), pos.y())
        elif event.button() == Qt.RightButton:
            self.view.StartZoomAtPoint(pos.x(), pos.y())
        
        self._old_pos = pos

    def mouseMoveEvent(self, event):
        
        pos = event.pos()
        x, y = pos.x(), pos.y()

        if event.buttons() == Qt.LeftButton:
            if self._orbital_rotation:
                delta_x, delta_y = x - self._old_pos.x(), y - self._old_pos.y()
                cam = self.view.Camera()
                z_rotation = gp_Trsf()
                z_rotation.SetRotation(gp_Ax1(cam.Center(), gp_Dir(0, 0, 1)), -delta_x * self._rotate_step)
                cam.Transform(z_rotation)
                self.view.Rotate(0, -delta_y * self._rotate_step, 0)
            else:
                self.view.Rotation(x, y)

        elif event.buttons() == Qt.MiddleButton:
            delta_x, delta_y = x - self._old_pos.x(), y - self._old_pos.y()
            self.view.Pan(delta_x, -delta_y, theToStart=True)
            
        elif event.buttons() == Qt.RightButton:
            self.view.ZoomAtPoint(self._old_pos.x(), y,
                                  x, self._old_pos.y())
        
        self._old_pos = pos
        
    def mouseReleaseEvent(self, event):
        
        if event.button() == Qt.LeftButton:
            pos = event.pos()
            self.context.MoveTo(pos.x(), pos.y(), self.view, True)
            self._handle_selection()
            
    def _handle_selection(self):
        
        self.context.Select(True)
        self.context.InitSelected()
        
        selected = []
        if self.context.HasSelectedShape():
            selected.append(self.context.SelectedShape())
        
        self.sigObjectSelected.emit(selected)

    def paintEngine(self):
    
        return None
    
    def paintEvent(self, event):
        
        if not self._initialized:
            self._initialize()
        else:
            self.view.Redraw()

    def showEvent(self, event):
    
        super(OCCTWidget,self).showEvent(event)
        
    def resizeEvent(self, event):
        
        super(OCCTWidget,self).resizeEvent(event)
        
        self.view.MustBeResized()
    
    def _initialize(self):

        wins = {
            'darwin' : self._get_window_osx,
            'linux'  : self._get_window_linux,
            'win32': self._get_window_win           
        }

        self.view.SetWindow(wins.get(platform,self._get_window_linux)(self.winId()))

        self._initialized = True
        
    def _get_window_win(self,wid):
    
        from OCP.WNT import WNT_Window
        
        return WNT_Window(wid.ascapsule())

    def _get_window_linux(self,wid):
        
        from OCP.Xw import Xw_Window
        
        return Xw_Window(self.display_connection,int(wid))
    
    def _get_window_osx(self,wid):
        
        from OCP.Cocoa import Cocoa_Window
        
        return Cocoa_Window(wid.ascapsule())
