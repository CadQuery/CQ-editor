from PyQt5.QtWidgets import QWidget, QDialog, QTreeWidgetItem, QApplication, QAction

from PyQt5.QtCore import pyqtSlot, pyqtSignal
from PyQt5.QtGui import QIcon

from OCP.Graphic3d import Graphic3d_Camera, Graphic3d_StereoMode, Graphic3d_NOM_JADE,\
    Graphic3d_MaterialAspect
from OCP.AIS import AIS_Shaded,AIS_WireFrame, AIS_ColoredShape, AIS_Axis
from OCP.Aspect import Aspect_GDM_Lines, Aspect_GT_Rectangular
from OCP.Quantity import Quantity_NOC_BLACK as BLACK, Quantity_TOC_RGB as TOC_RGB,\
    Quantity_Color
from OCP.Geom import Geom_Axis1Placement
from OCP.gp import gp_Ax3, gp_Dir, gp_Pnt, gp_Ax1

from ..utils import layout, get_save_filename
from ..mixins import ComponentMixin
from ..icons import icon
from ..cq_utils import to_occ_color, make_AIS, DEFAULT_FACE_COLOR

from .occt_widget import OCCTWidget

from pyqtgraph.parametertree import Parameter
import qtawesome as qta



DEFAULT_EDGE_COLOR = Quantity_Color(BLACK)
DEFAULT_EDGE_WIDTH = 2

class OCCViewer(QWidget,ComponentMixin):

    name = '3D Viewer'

    preferences = Parameter.create(name='Pref',children=[
        {'name': 'Fit automatically', 'type': 'bool', 'value': True},
        {'name': 'Use gradient', 'type': 'bool', 'value': False},
        {'name': 'Background color', 'type': 'color', 'value': (95,95,95)},
        {'name': 'Background color (aux)', 'type': 'color', 'value': (30,30,30)},
        {'name': 'Default object color', 'type': 'color', 'value': "#FF0"},
        {'name': 'Deviation', 'type': 'float', 'value': 1e-5, 'dec': True, 'step': 1},
        {'name': 'Angular deviation', 'type': 'float', 'value': 0.1, 'dec': True, 'step': 1},
        {'name': 'Projection Type', 'type': 'list', 'value': 'Orthographic',
         'values': ['Orthographic', 'Perspective', 'Stereo', 'MonoLeftEye', 'MonoRightEye']},
        {'name': 'Stereo Mode', 'type': 'list', 'value': 'QuadBuffer',
         'values': ['QuadBuffer', 'Anaglyph', 'RowInterlaced', 'ColumnInterlaced',
                    'ChessBoard', 'SideBySide', 'OverUnder']}])
    IMAGE_EXTENSIONS = 'png'

    sigObjectSelected = pyqtSignal(list)

    def __init__(self,parent=None):

        super(OCCViewer,self).__init__(parent)
        ComponentMixin.__init__(self)

        self.canvas = OCCTWidget()
        self.canvas.sigObjectSelected.connect(self.handle_selection)

        self.create_actions(self)

        self.layout_ = layout(self,
                             [self.canvas,],
                             top_widget=self,
                             margin=0)

        self.setup_default_drawer()
        self.updatePreferences()

    def setup_default_drawer(self):

        # set the default color and material
        material = Graphic3d_MaterialAspect(Graphic3d_NOM_JADE)

        shading_aspect = self.canvas.context.DefaultDrawer().ShadingAspect()
        shading_aspect.SetMaterial(material)
        shading_aspect.SetColor(DEFAULT_FACE_COLOR)

        # face edge lw
        line_aspect = self.canvas.context.DefaultDrawer().FaceBoundaryAspect()
        line_aspect.SetWidth(DEFAULT_EDGE_WIDTH)
        line_aspect.SetColor(DEFAULT_EDGE_COLOR)

    def updatePreferences(self,*args):

        color1 = to_occ_color(self.preferences['Background color'])
        color2 = to_occ_color(self.preferences['Background color (aux)'])

        if not self.preferences['Use gradient']:
            color2 = color1
        self.canvas.view.SetBgGradientColors(color1,color2,theToUpdate=True)

        self.canvas.update()

        ctx = self.canvas.context
        ctx.SetDeviationCoefficient(self.preferences['Deviation'])
        ctx.SetDeviationAngle(self.preferences['Angular deviation'])

        v = self._get_view()
        camera = v.Camera()
        projection_type = self.preferences['Projection Type']
        camera.SetProjectionType(getattr(Graphic3d_Camera, f'Projection_{projection_type}',
                                         Graphic3d_Camera.Projection_Orthographic))

        # onle relevant for stereo projection
        stereo_mode = self.preferences['Stereo Mode']
        params = v.ChangeRenderingParams()
        params.StereoMode = getattr(Graphic3d_StereoMode, f'Graphic3d_StereoMode_{stereo_mode}',
                                    Graphic3d_StereoMode.Graphic3d_StereoMode_QuadBuffer)

    def create_actions(self,parent):

        self._actions =  \
                {'View' : [QAction(qta.icon('fa.arrows-alt'),
                                   'Fit (Shift+F1)',
                                   parent,
                                   shortcut='shift+F1',
                                   triggered=self.fit),
                          QAction(QIcon(':/images/icons/isometric_view.svg'),
                                  'Iso (Shift+F2)',
                                  parent,
                                  shortcut='shift+F2',
                                  triggered=self.iso_view),
                          QAction(QIcon(':/images/icons/top_view.svg'),
                                  'Top (Shift+F3)',
                                  parent,
                                  shortcut='shift+F3',
                                  triggered=self.top_view),
                          QAction(QIcon(':/images/icons/bottom_view.svg'),
                                  'Bottom (Shift+F4)',
                                  parent,
                                  shortcut='shift+F4',
                                  triggered=self.bottom_view),
                          QAction(QIcon(':/images/icons/front_view.svg'),
                                  'Front (Shift+F5)',
                                  parent,
                                  shortcut='shift+F5',
                                  triggered=self.front_view),
                          QAction(QIcon(':/images/icons/back_view.svg'),
                                  'Back (Shift+F6)',
                                  parent,
                                  shortcut='shift+F6',
                                  triggered=self.back_view),
                          QAction(QIcon(':/images/icons/left_side_view.svg'),
                                  'Left (Shift+F7)',
                                  parent,
                                  shortcut='shift+F7',
                                  triggered=self.left_view),
                          QAction(QIcon(':/images/icons/right_side_view.svg'),
                                  'Right (Shift+F8)',
                                  parent,
                                  shortcut='shift+F8',
                                  triggered=self.right_view),
                          QAction(qta.icon('fa.square-o'),
                                  'Wireframe (Shift+F9)',
                                  parent,
                                  shortcut='shift+F9',
                                  triggered=self.wireframe_view),
                                  QAction(qta.icon('fa.square'),
                                          'Shaded (Shift+F10)',
                                          parent,
                                          shortcut='shift+F10',
                                          triggered=self.shaded_view)],
                 'Tools' : [QAction(icon('screenshot'),
                                   'Screenshot',
                                   parent,
                                   triggered=self.save_screenshot)]}

    def toolbarActions(self):

        return self._actions['View']


    def clear(self):

        self.displayed_shapes = []
        self.displayed_ais = []
        self.canvas.context.EraseAll(True)
        context = self._get_context()
        context.PurgeDisplay()
        context.RemoveAll(True)

    def _display(self,shape):

        ais = make_AIS(shape)
        self.canvas.context.Display(shape,True)

        self.displayed_shapes.append(shape)
        self.displayed_ais.append(ais)

        #self.canvas._display.Repaint()

    @pyqtSlot(object)
    def display(self,ais):

        context = self._get_context()
        context.Display(ais,True)

        if self.preferences['Fit automatically']: self.fit()

    @pyqtSlot(list)
    @pyqtSlot(list,bool)
    def display_many(self,ais_list,fit=None):

        context = self._get_context()
        for ais in ais_list:
            context.Display(ais,True)

        if self.preferences['Fit automatically'] and fit is None:
            self.fit()
        elif fit:
            self.fit()

    @pyqtSlot(QTreeWidgetItem,int)
    def update_item(self,item,col):

        ctx = self._get_context()
        if item.checkState(0):
            ctx.Display(item.ais,True)
        else:
            ctx.Erase(item.ais,True)

    @pyqtSlot(list)
    def remove_items(self,ais_items):

        ctx = self._get_context()
        for ais in ais_items: ctx.Erase(ais,True)

    @pyqtSlot()
    def redraw(self):

        self._get_viewer().Redraw()

    def fit(self):

        self.canvas.view.FitAll()

    def iso_view(self):

        v = self._get_view()
        v.SetProj(1,-1,1)
        v.SetTwist(0)

    def bottom_view(self):

        v = self._get_view()
        v.SetProj(0,0,-1)
        v.SetTwist(0)

    def top_view(self):

        v = self._get_view()
        v.SetProj(0,0,1)
        v.SetTwist(0)

    def front_view(self):

        v = self._get_view()
        v.SetProj(0,1,0)
        v.SetTwist(0)

    def back_view(self):

        v = self._get_view()
        v.SetProj(0,-1,0)
        v.SetTwist(0)

    def left_view(self):

        v = self._get_view()
        v.SetProj(-1,0,0)
        v.SetTwist(0)

    def right_view(self):

        v = self._get_view()
        v.SetProj(1,0,0)
        v.SetTwist(0)

    def shaded_view(self):

        c = self._get_context()
        c.SetDisplayMode(AIS_Shaded, True)

    def wireframe_view(self):

        c = self._get_context()
        c.SetDisplayMode(AIS_WireFrame, True)

    def show_grid(self,
                  step=1.,
                  size=10.+1e-6,
                  color1=(.7,.7,.7),
                  color2=(0,0,0)):

        viewer = self._get_viewer()
        viewer.ActivateGrid(Aspect_GT_Rectangular,
                            Aspect_GDM_Lines)
        viewer.SetRectangularGridGraphicValues(size, size, 0)
        viewer.SetRectangularGridValues(0, 0, step, step, 0)
        grid = viewer.Grid()
        grid.SetColors(Quantity_Color(*color1,TOC_RGB),
                       Quantity_Color(*color2,TOC_RGB))

    def hide_grid(self):

        viewer = self._get_viewer()
        viewer.DeactivateGrid()

    @pyqtSlot(bool,float)
    @pyqtSlot(bool)
    def toggle_grid(self,
                    value : bool,
                    dim : float = 10.):

        if value:
            self.show_grid(step=dim/20,size=dim+1e-9)
        else:
            self.hide_grid()

    @pyqtSlot(gp_Ax3)
    def set_grid_orientation(self,orientation : gp_Ax3):

        viewer = self._get_viewer()
        viewer.SetPrivilegedPlane(orientation)

    def show_axis(self,origin = (0,0,0), direction=(0,0,1)):

        ax_placement = Geom_Axis1Placement(gp_Ax1(gp_Pnt(*origin),
                                                  gp_Dir(*direction)))
        ax = AIS_Axis(ax_placement)
        self._display_ais(ax)

    def save_screenshot(self):

        fname = get_save_filename(self.IMAGE_EXTENSIONS)
        if fname != '':
             self._get_view().Dump(fname)

    def _display_ais(self,ais):

        self._get_context().Display(ais)


    def _get_view(self):

        return self.canvas.view

    def _get_viewer(self):

        return self.canvas.viewer

    def _get_context(self):

        return self.canvas.context

    @pyqtSlot(list)
    def handle_selection(self,obj):

        self.sigObjectSelected.emit(obj)

    @pyqtSlot(list)
    def set_selected(self,ais):

        ctx = self._get_context()
        ctx.ClearSelected(False)

        for obj in ais:
            ctx.AddOrRemoveSelected(obj,False)

        self.redraw()


if __name__ == "__main__":

    import sys
    from OCP.BRepPrimAPI import BRepPrimAPI_MakeBox

    app = QApplication(sys.argv)
    viewer = OCCViewer()

    dlg = QDialog()
    dlg.setFixedHeight(400)
    dlg.setFixedWidth(600)

    layout(dlg,(viewer,),dlg)
    dlg.show()

    box = BRepPrimAPI_MakeBox(20,20,30)
    box_ais = AIS_ColoredShape(box.Shape())
    viewer.display(box_ais)

    sys.exit(app.exec_())
