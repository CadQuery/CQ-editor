from sys import platform


from PyQt5.QtWidgets import QWidget, QApplication
from PyQt5.QtCore import pyqtSlot, pyqtSignal, Qt, QPoint, QTimer

import OCP

from OCP.Aspect import Aspect_DisplayConnection, Aspect_TypeOfTriedronPosition
from OCP.OpenGl import OpenGl_GraphicDriver
from OCP.V3d import V3d_Viewer
from OCP.gp import gp_Trsf, gp_Ax1, gp_Dir, gp_Pnt
from OCP.AIS import AIS_InteractiveContext, AIS_DisplayMode, AIS_ViewCubeOwner
from OCP.Quantity import Quantity_Color

from .navigation_cube import NavigationCube

ZOOM_STEP = 0.9


class OCCTWidget(QWidget):

    sigObjectSelected = pyqtSignal(list)

    def __init__(self, parent=None):

        super(OCCTWidget, self).__init__(parent)

        self.setAttribute(Qt.WA_NativeWindow)
        self.setAttribute(Qt.WA_PaintOnScreen)
        self.setAttribute(Qt.WA_NoSystemBackground)

        self._initialized = False
        self._needs_update = False
        self._previous_pos = QPoint(
            0, 0  # Keeps track of where the previous mouse position
        )
        self._rotate_step = (
            0.008  # Controls the speed of rotation with the turntable orbit method
        )

        # Orbit method settings
        self._orbit_method = "Turntable"

        # Drives the view cube camera animation; runs only while animating
        self._cube_timer = QTimer(self)
        self._cube_timer.setInterval(16)
        self._cube_timer.timeout.connect(self._animate_view_cube)

        self._cube_hover = False
        self.setMouseTracking(True)

        # OCCT secific things
        self.display_connection = Aspect_DisplayConnection()
        self.graphics_driver = OpenGl_GraphicDriver(self.display_connection)

        self.viewer = V3d_Viewer(self.graphics_driver)
        self.view = self.viewer.CreateView()
        self.context = AIS_InteractiveContext(self.viewer)

        # Trihedorn, lights, etc
        self.prepare_display()

    def prepare_display(self):

        view = self.view

        params = view.ChangeRenderingParams()
        params.NbMsaaSamples = 8
        params.IsAntialiasingEnabled = True

        view.TriedronDisplay(
            Aspect_TypeOfTriedronPosition.Aspect_TOTP_RIGHT_LOWER, Quantity_Color(), 0.1
        )

        viewer = self.viewer

        viewer.SetDefaultLights()
        viewer.SetLightOn()

        ctx = self.context

        ctx.SetDisplayMode(AIS_DisplayMode.AIS_Shaded, True)
        ctx.DefaultDrawer().SetFaceBoundaryDraw(True)

        self.view_cube = NavigationCube()
        self.view_cube.ViewAnimation().SetView(view)
        ctx.Display(self.view_cube, False)

    def set_orbit_method(self, method):
        """
        Set the orbit method for the OCCT view.
        """

        # Keep track of which orbit method is used
        if method == "Turntable":
            self._orbit_method = "Turntable"
            self.view.SetUp(0, 0, 1)
        elif method == "Trackball":
            self._orbit_method = "Trackball"
        else:
            raise ValueError(f"Unknown orbit method: {method}")

    def wheelEvent(self, event):

        delta = event.angleDelta().y()
        factor = ZOOM_STEP if delta < 0 else 1 / ZOOM_STEP

        self.view.SetZoom(factor)

    def mousePressEvent(self, event):

        pos = event.pos()

        if event.button() == Qt.LeftButton:
            # Used to prevent drag selection of objects
            self.pending_select = True
            self.left_press = pos

            # We only start the rotation if the orbit method is set to Trackball
            if self._orbit_method == "Trackball":
                self.view.StartRotation(pos.x(), pos.y())
        elif event.button() == Qt.RightButton:
            self.view.StartZoomAtPoint(pos.x(), pos.y())

        self._previous_pos = pos

    def mouseMoveEvent(self, event):

        pos = event.pos()
        x, y = pos.x(), pos.y()

        # Hover highlight is confined to the cube's corner region so the
        # rest of the viewport keeps its existing no-hover behavior
        if not event.buttons():
            if self.view_cube.hit_region(self.width()).contains(pos):
                self.context.MoveTo(x, y, self.view, True)
                owner = (
                    self.context.DetectedOwner()
                    if self.context.HasDetected()
                    else None
                )
                if not isinstance(owner, AIS_ViewCubeOwner):
                    self.context.ClearDetected(True)
                self._cube_hover = True
            elif self._cube_hover:
                self.context.ClearDetected(True)
                self._cube_hover = False
            self._previous_pos = pos
            return

        # Check for mouse drag rotation
        if event.buttons() == Qt.LeftButton and event.modifiers() not in (
            Qt.ShiftModifier,
            Qt.ControlModifier,
        ):
            # Set the rotation differently based on the orbit method
            if self._orbit_method == "Trackball":
                self.view.Rotation(x, y)
            elif self._orbit_method == "Turntable":
                # Control the turntable rotation manually
                delta_x, delta_y = (
                    x - self._previous_pos.x(),
                    y - self._previous_pos.y(),
                )
                cam = self.view.Camera()
                z_rotation = gp_Trsf()
                z_rotation.SetRotation(
                    gp_Ax1(cam.Center(), gp_Dir(0, 0, 1)), -delta_x * self._rotate_step
                )
                cam.Transform(z_rotation)
                self.view.Rotate(0, -delta_y * self._rotate_step, 0)

        # If the user moves the mouse at all, the selection will not happen
        if event.buttons() == Qt.LeftButton:
            if abs(x - self.left_press.x()) > 2 or abs(y - self.left_press.y()) > 2:
                self.pending_select = False

        if event.buttons() == Qt.MiddleButton or (
            event.buttons() == Qt.LeftButton and event.modifiers() == Qt.ControlModifier
        ):
            self.view.Pan(
                x - self._previous_pos.x(), self._previous_pos.y() - y, theToStart=True
            )

        elif event.buttons() == Qt.RightButton or (
            event.buttons() == Qt.LeftButton and event.modifiers() == Qt.ShiftModifier
        ):
            self.view.ZoomAtPoint(self._previous_pos.x(), y, x, self._previous_pos.y())

        self._previous_pos = pos

    def mouseReleaseEvent(self, event):

        if event.button() == Qt.LeftButton:
            pos = event.pos()
            x, y = pos.x(), pos.y()

            # Only make the selection if the user has not moved the mouse
            if self.pending_select:
                self.context.MoveTo(x, y, self.view, True)

                owner = (
                    self.context.DetectedOwner()
                    if self.context.HasDetected()
                    else None
                )
                if isinstance(owner, AIS_ViewCubeOwner):
                    self._handle_cube_click(owner)
                else:
                    self._handle_selection()

    def _handle_selection(self):

        self.context.Select(True)
        self.context.InitSelected()

        selected = []
        if self.context.HasSelectedShape():
            selected.append(self.context.SelectedShape())

        self.sigObjectSelected.emit(selected)

    def _settle_animations(self):
        """Snap any running camera animation to its end state.

        A click during a running animation must start from the settled end:
        OCCT treats a repeated click on the current orientation specially
        (snaps up to the canonical value), and that detection compares
        against a partially-rotated camera otherwise.
        """

        self._cube_timer.stop()

        cube_animation = self.view_cube.ViewAnimation()
        if not cube_animation.IsStopped():
            cube_animation.Stop()
            self.view.Camera().Copy(cube_animation.CameraEnd())

    def _handle_cube_click(self, owner):
        """Start the animated camera reorientation for a picked cube part."""

        self._settle_animations()

        camera = self.view.Camera()
        center, distance, scale = camera.Center(), camera.Distance(), camera.Scale()

        self.view_cube.HandleClick(owner)

        # HandleClick fits the scene into the animation's target camera;
        # restore the current framing so the click is a pure rotation that
        # preserves zoom, center and distance. The eye is placed explicitly
        # together with the old center: SetCenter alone would tilt the
        # target direction after a pan, and MoveEyeTo alone would keep the
        # fitted distance and drag the center along the view axis (visible
        # as a zoom in perspective projection).
        animation = self.view_cube.ViewAnimation()
        self._restore_framing(animation.CameraEnd(), center, distance, scale)

        # with a zero duration the animation completes inside HandleClick,
        # so the fitted end camera is already applied; restore the view
        # camera directly in that case
        if animation.IsStopped():
            self._restore_framing(self.view.Camera(), center, distance, scale)
            self.view.Redraw()

        self._cube_timer.start()

    @staticmethod
    def _restore_framing(camera, center, distance, scale):
        """Re-frame a camera on center/distance/scale, keeping its direction."""

        direction = camera.Direction()
        camera.SetEyeAndCenter(
            gp_Pnt(
                center.X() - direction.X() * distance,
                center.Y() - direction.Y() * distance,
                center.Z() - direction.Z() * distance,
            ),
            center,
        )
        camera.SetScale(scale)

    def _animate_view_cube(self):

        if not self.view_cube.UpdateAnimation(True):
            self._cube_timer.stop()
            # the finishing UpdateAnimation applies the end camera but only
            # invalidates the view; without an explicit redraw the screen
            # keeps the previous frame until an unrelated repaint
            self.view.Redraw()

    def paintEngine(self):

        return None

    def paintEvent(self, event):

        if not self._initialized:
            self._initialize()
        else:
            self.view.Redraw()

    def showEvent(self, event):

        super(OCCTWidget, self).showEvent(event)

    def resizeEvent(self, event):

        super(OCCTWidget, self).resizeEvent(event)

        self.view.MustBeResized()

    def leaveEvent(self, event):

        if self._cube_hover:
            self.context.ClearDetected(True)
            self._cube_hover = False

    def _initialize(self):

        wins = {
            "darwin": self._get_window_osx,
            "linux": self._get_window_linux,
            "win32": self._get_window_win,
        }

        self.view.SetWindow(wins.get(platform, self._get_window_linux)(self.winId()))

        self._initialized = True

    def _get_window_win(self, wid):

        from OCP.WNT import WNT_Window

        return WNT_Window(wid.ascapsule())

    def _get_window_linux(self, wid):

        from OCP.Xw import Xw_Window

        return Xw_Window(self.display_connection, int(wid))

    def _get_window_osx(self, wid):

        from OCP.Cocoa import Cocoa_Window

        return Cocoa_Window(wid.ascapsule())
