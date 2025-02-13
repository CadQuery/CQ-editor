from sys import platform
import sys

from PyQt5.QtWidgets import QWidget, QApplication
from PyQt5.QtCore import pyqtSlot, pyqtSignal, Qt, QEvent, QTimer

import OCP

from OCP.Aspect import Aspect_DisplayConnection, Aspect_TypeOfTriedronPosition
from OCP.OpenGl import OpenGl_GraphicDriver
from OCP.V3d import V3d_Viewer
from OCP.AIS import AIS_InteractiveContext, AIS_DisplayMode
from OCP.Quantity import Quantity_Color

# pyspacemouse is an optional dependency, so don't fail if it isn't present
try:
    import pyspacemouse
except:
    pass


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

        self._spacemouse_connected = False
        if 'pyspacemouse' in sys.modules:
            # TODO: this currently checks for 3d mouse presence once at
            # initialization, which means that if you plug in your mouse after
            # starting cq-editor it won't work. We should instead periodically
            # check to see if a mouse was newly-connected.

            # TODO: the callback-based approach seems better than polling, but
            # it didn't work. Probably something threading-related. The
            # callback api works for me in a standalone script.
            #
            # self._spacemouse_connected = pyspacemouse.open(callback=self.spacemouse_callback, dof_callback=self.spacemouse_dof_callback, button_callback=self.spacemouse_button_callback)

            self._spacemouse_connected = pyspacemouse.open()
            if self._spacemouse_connected:
                print("3DConnexion SpaceNavigator 3d mouse detected")
                print(self._spacemouse_connected)

                # setup a timer to periodically read the 3d mouse state and
                # update the view
                self._spacemouse_timer = QTimer(self)
                self._spacemouse_timer.setInterval(10) # milliseconds
                self._spacemouse_timer.setSingleShot(False)
                self._spacemouse_timer.timeout.connect(self._update_from_spacemouse)
                self._spacemouse_timer.start()

        
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

    # def spacemouse_callback(self, state):
    #     print("spacemouse_callback: ", state)

    # def spacemouse_dof_callback(self, state):
    #     print("spacemouse_dof_callback: ", state)

    # def spacemouse_button_callback(self, state):
    #     print("spacemouse_button_callback: ", state)

    # called periodically to read new values from the 3d mouse and update the
    # view
    def _update_from_spacemouse(self):
        state = pyspacemouse.read()
        # print(state)

        # axis values less than this are ignored
        threshold = 0.05

        if abs(state.y) > threshold:
            # if state.y < 0:
            self.view.SetZoom(1-state.y/20)

        # TODO: roll and pitch should also factor into rotation
        # if abs(state.yaw) > threshold:
        #     angle_rad = state.yaw
        #     # TODO: this rotates about the "current axis". Whatever the current
        #     # axis is, it doesn't seem to be what we want
        #     self.view.Rotate(angle_rad,False)

        pan_x = 0
        pan_y = 0
        if abs(state.x) > threshold:
            pan_x = int(state.x*100)
        if abs(state.z) > threshold:
            pan_y = int(state.z*100)
        if pan_x != 0 or pan_y != 0:
            self.view.Pan(pan_x, pan_y, theToStart=True)


    # Override QWidget.destroy to cleanup 3d mouse connection
    # TODO: this doesn't seem to get called... use __del__() instead?
    def destroy(self, destroyWindow, destroySubWindows):
        super().destroy(destroyWindow, destroySubWindows)
        print("DEBUG: OcctWidget.destroy() called")
        if self._spacemouse_connected:
            pyspacemouse.close()
            self._spacemouse_timer.stop()
