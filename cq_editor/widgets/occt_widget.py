from sys import platform


from PyQt5.QtWidgets import QWidget, QApplication
from PyQt5.QtCore import pyqtSlot, pyqtSignal, Qt, QEvent

import OCP

from OCP.Aspect import Aspect_DisplayConnection, Aspect_TypeOfTriedronPosition
from OCP.OpenGl import OpenGl_GraphicDriver
from OCP.V3d import V3d_Viewer
from OCP.AIS import AIS_InteractiveContext, AIS_DisplayMode
from OCP.Quantity import Quantity_Color


ZOOM_STEP = 0.9

   
class OCCTWidget(QWidget):
    
    sigObjectSelected = pyqtSignal(list)
    
    def __init__(self,parent=None):
        
        super(OCCTWidget,self).__init__(parent)
        
        self.setAttribute(Qt.WA_NativeWindow)
        self.setAttribute(Qt.WA_PaintOnScreen)
        self.setAttribute(Qt.WA_NoSystemBackground)
        
        self._initialized = False
        self._needs_update = False
        
        #OCCT secific things
        self.display_connection = Aspect_DisplayConnection()
        self.graphics_driver = OpenGl_GraphicDriver(self.display_connection)
        
        self.viewer = V3d_Viewer(self.graphics_driver)
        self.view = self.viewer.CreateView()
        self.context = AIS_InteractiveContext(self.viewer)
        
        #Trihedorn, lights, etc
        self.prepare_display()
        
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
        
        delta = event.angleDelta().y()
        factor = ZOOM_STEP if delta<0 else 1/ZOOM_STEP
        
        self.view.SetZoom(factor)
        
    def mousePressEvent(self,event):
        
        pos = event.pos()
        
        if event.button() == Qt.LeftButton:
            self.view.StartRotation(pos.x(), pos.y())
        elif event.button() == Qt.RightButton:
            self.view.StartZoomAtPoint(pos.x(), pos.y())
        
        self.old_pos = pos
            
    def mouseMoveEvent(self,event):
        
        pos = event.pos()
        x,y = pos.x(),pos.y()
        
        if event.buttons() == Qt.LeftButton:
            self.view.Rotation(x,y)
            
        elif event.buttons() == Qt.MiddleButton:
            self.view.Pan(x - self.old_pos.x(),
                          self.old_pos.y() - y, theToStart=True)
            
        elif event.buttons() == Qt.RightButton:
            self.view.ZoomAtPoint(self.old_pos.x(), y,
                                  x, self.old_pos.y())
        
        self.old_pos = pos
        
    def mouseReleaseEvent(self,event):
        
        if event.button() == Qt.LeftButton:
            pos = event.pos()
            x,y = pos.x(),pos.y()
            
            self.context.MoveTo(x,y,self.view,True)
            
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
