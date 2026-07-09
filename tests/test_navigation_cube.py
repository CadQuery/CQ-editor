import pytest

from PyQt5.QtCore import QPoint, Qt, QEvent, QPointF
from PyQt5.QtGui import QMouseEvent
from PyQt5.QtWidgets import QDialog

from OCP.AIS import AIS_ViewCube, AIS_ViewCubeOwner, AIS_ColoredShape
from OCP.BRepPrimAPI import BRepPrimAPI_MakeBox
from OCP.V3d import V3d_TypeOfOrientation

from cq_editor.utils import layout
from cq_editor.widgets.navigation_cube import NavigationCube, CORNER_OFFSET
from cq_editor.widgets.viewer import OCCViewer


def test_hit_region():

    cube = NavigationCube()
    region = cube.hit_region(600)

    assert region.contains(QPoint(600 - CORNER_OFFSET, CORNER_OFFSET))
    assert region.contains(QPoint(600 - 1, 0))
    assert not region.contains(QPoint(600 - 2 * CORNER_OFFSET - 1, 10))
    assert not region.contains(QPoint(300, 200))
    assert not region.contains(QPoint(600 - CORNER_OFFSET, 2 * CORNER_OFFSET + 1))


def test_cube_front_matches_toolbar_front():

    # the toolbar's Front action looks from +Y (OCCViewer.front_view uses
    # SetProj(0, 1, 0)), so the cube face seen from +Y must be labeled FRONT
    cube = NavigationCube()

    assert cube.BoxSideLabel(V3d_TypeOfOrientation.V3d_Ypos).ToCString() == "FRONT"
    assert cube.BoxSideLabel(V3d_TypeOfOrientation.V3d_Yneg).ToCString() == "BACK"


@pytest.fixture
def viewer(qtbot):

    win = QDialog()
    win.setFixedSize(600, 400)

    v = OCCViewer()
    layout(win, (v,), win)

    qtbot.addWidget(win)
    with qtbot.waitExposed(win):
        win.show()

    v.canvas.repaint()

    # generator fixture: keeps `win` referenced for the test's duration
    # (qtbot.addWidget holds only a weakref)
    yield qtbot, v


def test_cube_displayed(viewer):

    qtbot, v = viewer

    cube = v.canvas.view_cube
    assert isinstance(cube, AIS_ViewCube)
    assert v.canvas.context.IsDisplayed(cube)
    assert cube.ViewAnimation().View() is not None


def test_cube_click_reorients_camera(viewer):

    qtbot, v = viewer
    canvas = v.canvas

    # a displayed shape is required for the click's scene-fit to be
    # observable (empty scenes have nothing to re-fit to)
    box = AIS_ColoredShape(BRepPrimAPI_MakeBox(20, 20, 30).Shape())
    v.display(box)

    canvas.view_cube.SetDuration(0)
    owner = AIS_ViewCubeOwner(canvas.view_cube, V3d_TypeOfOrientation.V3d_Xpos)

    # register the view as the context's last active view, as the MoveTo in
    # mouseReleaseEvent does before any real cube click (StartAnimation
    # silently no-ops when LastActiveView is unset)
    canvas.context.MoveTo(0, 0, canvas.view, True)

    canvas.view.SetZoom(3.0)
    scale_before = canvas.view.Scale()
    camera = canvas.view.Camera()
    distance_before = camera.Distance()
    center_before = camera.Center()

    canvas._handle_cube_click(owner)
    qtbot.waitUntil(lambda: not canvas._cube_timer.isActive(), timeout=2000)

    camera = canvas.view.Camera()
    assert canvas.view.Proj() == pytest.approx((1, 0, 0), abs=1e-6)
    assert canvas.view.Scale() == pytest.approx(scale_before, rel=1e-6)
    assert camera.Distance() == pytest.approx(distance_before, rel=1e-6)
    assert camera.Center().Distance(center_before) == pytest.approx(0, abs=1e-6)


def test_cube_click_reorients_camera_after_pan(viewer):

    qtbot, v = viewer
    canvas = v.canvas

    box = AIS_ColoredShape(BRepPrimAPI_MakeBox(20, 20, 30).Shape())
    v.display(box)

    canvas.view_cube.SetDuration(0)
    owner = AIS_ViewCubeOwner(canvas.view_cube, V3d_TypeOfOrientation.V3d_Xpos)

    canvas.context.MoveTo(0, 0, canvas.view, True)

    canvas.view.Pan(80, 50)
    scale_before = canvas.view.Scale()
    camera = canvas.view.Camera()
    distance_before = camera.Distance()
    center_before = camera.Center()

    canvas._handle_cube_click(owner)
    qtbot.waitUntil(lambda: not canvas._cube_timer.isActive(), timeout=2000)

    camera = canvas.view.Camera()
    assert canvas.view.Proj() == pytest.approx((1, 0, 0), abs=1e-6)
    assert canvas.view.Scale() == pytest.approx(scale_before, rel=1e-6)
    assert camera.Distance() == pytest.approx(distance_before, rel=1e-6)
    assert camera.Center().Distance(center_before) == pytest.approx(0, abs=1e-6)


def test_cube_click_final_frame_redrawn(viewer):

    qtbot, v = viewer
    canvas = v.canvas

    box = AIS_ColoredShape(BRepPrimAPI_MakeBox(20, 20, 30).Shape())
    v.display(box)

    canvas.view_cube.SetDuration(0.15)
    owner = AIS_ViewCubeOwner(canvas.view_cube, V3d_TypeOfOrientation.V3d_Xpos)
    canvas.context.MoveTo(0, 0, canvas.view, True)

    canvas._handle_cube_click(owner)
    qtbot.waitUntil(lambda: not canvas._cube_timer.isActive(), timeout=2000)

    # the last animation frame must actually reach the screen; a view left
    # invalidated shows the previous frame until an unrelated repaint
    assert not canvas.view.IsInvalidated()


def test_cube_click_during_animation_settles_previous(viewer):

    qtbot, v = viewer
    canvas = v.canvas

    box = AIS_ColoredShape(BRepPrimAPI_MakeBox(20, 20, 30).Shape())
    v.display(box)

    canvas.view_cube.SetDuration(0.15)
    corner = AIS_ViewCubeOwner(
        canvas.view_cube, V3d_TypeOfOrientation.V3d_XposYposZpos
    )
    canvas.context.MoveTo(0, 0, canvas.view, True)

    v.top_view()

    # second click lands mid-animation; it must behave as a repeated click
    # on the settled orientation, which snaps up to the canonical value
    canvas._handle_cube_click(corner)
    qtbot.wait(50)
    canvas._handle_cube_click(corner)
    qtbot.waitUntil(lambda: not canvas._cube_timer.isActive(), timeout=2000)

    up = canvas.view.Camera().Up()
    assert (up.X(), up.Y(), up.Z()) == pytest.approx(
        (-0.408248, -0.408248, 0.816497), abs=1e-5
    )


def _move(canvas, x, y):

    event = QMouseEvent(
        QEvent.MouseMove,
        QPointF(x, y),
        Qt.NoButton,
        Qt.MouseButtons(Qt.NoButton),
        Qt.KeyboardModifiers(Qt.NoModifier),
    )
    canvas.mouseMoveEvent(event)


def test_cube_hover_highlight(viewer):

    qtbot, v = viewer
    canvas = v.canvas

    assert canvas.hasMouseTracking()

    _move(canvas, canvas.width() - 85, 85)
    assert canvas.context.HasDetected()

    _move(canvas, canvas.width() // 2, canvas.height() // 2)
    assert not canvas.context.HasDetected()


def test_cube_highlight_cleared_on_leave(viewer):

    qtbot, v = viewer
    canvas = v.canvas

    _move(canvas, canvas.width() - 85, 85)
    assert canvas.context.HasDetected()

    canvas.leaveEvent(None)
    assert not canvas.context.HasDetected()


def test_cube_visibility_preference(viewer):

    qtbot, v = viewer
    ctx = v.canvas.context
    cube = v.canvas.view_cube

    v.preferences["Show navigation cube"] = False
    assert not ctx.IsDisplayed(cube)

    v.preferences["Show navigation cube"] = True
    assert ctx.IsDisplayed(cube)


def test_fit_ignores_cube(viewer):

    qtbot, v = viewer
    view = v.canvas.view

    box = AIS_ColoredShape(BRepPrimAPI_MakeBox(20, 20, 30).Shape())
    v.display(box)

    v.preferences["Show navigation cube"] = False
    v.fit()
    scale_without_cube = view.Scale()

    v.preferences["Show navigation cube"] = True
    v.fit()

    assert view.Scale() == pytest.approx(scale_without_cube, rel=1e-6)
