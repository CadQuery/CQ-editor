from math import cos, radians, sin

from PyQt5.QtCore import QRect

from OCP.AIS import AIS_Shape, AIS_ViewCube
from OCP.Aspect import Aspect_TOTP_RIGHT_UPPER
from OCP.BRepBuilderAPI import BRepBuilderAPI_MakeFace, BRepBuilderAPI_MakePolygon
from OCP.gp import gp_Pnt
from OCP.Graphic3d import (
    Graphic3d_TransformPers,
    Graphic3d_TMF_2d,
    Graphic3d_TMF_TriedronPers,
    Graphic3d_TypeOfShadingModel,
    Graphic3d_Vec2i,
    Graphic3d_ZLayerId_Topmost,
)
from OCP.Font import Font_FontAspect
from OCP.Quantity import Quantity_Color, Quantity_TOC_RGB
from OCP.TCollection import TCollection_AsciiString
from OCP.V3d import V3d_TypeOfOrientation

CORNER_OFFSET = 85
SIZE = 55
FACET_EXTENSION = 13
EDGE_MIN_SIZE = 6
CORNER_MIN_SIZE = 6
FONT_HEIGHT = 14
ANIMATION_DURATION = 0.25

ARROW_COLOR = Quantity_Color(0.72, 0.72, 0.75, Quantity_TOC_RGB)
ARROW_HILIGHT_COLOR = Quantity_Color(0.0, 1.0, 1.0, Quantity_TOC_RGB)
ARROW_INNER_RADIUS = 58
ARROW_OUTER_RADIUS = 69
ARROW_TAIL_ANGLE = 74  # degrees from screen-x, where the arc starts
ARROW_HEAD_ANGLE = 50  # degrees, where the head base sits
ARROW_TIP_SWEEP = 13  # degrees swept by the head triangle
ARROW_HEAD_OVERHANG = 6  # px the head base juts beyond the band
ARC_SEGMENTS = 12


class NavigationCube(AIS_ViewCube):
    """FreeCAD-style navigation cube.

    Transform-persistent overlay in the upper-right corner of the view: it
    is rendered by the main camera, so it always reflects the current
    orientation, keeps a constant screen size and is unaffected by zoom.
    Faces, edges and corners are pickable; a click starts the built-in
    eased camera animation (projection preserved; the owning widget
    restores zoom and center, and drives frames via UpdateAnimation()).
    """

    def __init__(self):

        super().__init__()

        self.SetTransformPersistence(
            Graphic3d_TransformPers(
                Graphic3d_TMF_TriedronPers,
                Aspect_TOTP_RIGHT_UPPER,
                Graphic3d_Vec2i(CORNER_OFFSET, CORNER_OFFSET),
            )
        )
        # SetSize(..., True) auto-derives facet/font metrics, so the
        # explicit overrides must come after it
        self.SetSize(SIZE, True)
        self.SetBoxFacetExtension(FACET_EXTENSION)
        self.SetBoxEdgeMinSize(EDGE_MIN_SIZE)
        self.SetBoxCornerMinSize(CORNER_MIN_SIZE)
        self.SetFontHeight(FONT_HEIGHT)
        self.Attributes().TextAspect().Aspect().SetTextFontAspect(
            Font_FontAspect.Font_FontAspect_Bold
        )
        self.SetBoxColor(Quantity_Color(0.72, 0.72, 0.75, Quantity_TOC_RGB))
        self.SetTextColor(Quantity_Color(0.0, 0.0, 0.0, Quantity_TOC_RGB))
        self.SetDrawAxes(False)

        # OCCT labels FRONT on the -Y face; CQ-editor's toolbar Front action
        # looks from +Y, so swap the two labels to keep them consistent
        self.SetBoxSideLabel(
            V3d_TypeOfOrientation.V3d_Ypos, TCollection_AsciiString("FRONT")
        )
        self.SetBoxSideLabel(
            V3d_TypeOfOrientation.V3d_Yneg, TCollection_AsciiString("BACK")
        )

        self.SetDuration(ANIMATION_DURATION)
        self.SetAutoStartAnimation(True)
        self.SetFixedAnimationLoop(False)

    def hit_region(self, widget_width):
        """Screen-space rectangle (widget coordinates) covering the cube."""

        extent = 2 * CORNER_OFFSET
        return QRect(widget_width - extent, 0, extent, extent)


class RotationArrow(AIS_Shape):
    """Curved roll-arrow button drawn above the navigation cube.

    Screen-fixed 2D overlay (it does not rotate with the camera) anchored
    at the cube's corner offset. Clicking it rolls the view by 45 degrees
    around the screen-normal axis; `sense` (+1 left arrow, -1 right arrow)
    selects the roll direction and mirrors the geometry.
    """

    def __init__(self, sense):

        self.sense = sense
        super().__init__(self._make_face(sense))

        self.SetTransformPersistence(
            Graphic3d_TransformPers(
                Graphic3d_TMF_2d,
                Aspect_TOTP_RIGHT_UPPER,
                Graphic3d_Vec2i(CORNER_OFFSET, CORNER_OFFSET),
            )
        )
        self.SetColor(ARROW_COLOR)
        self.SetZLayer(Graphic3d_ZLayerId_Topmost)
        # unlit: a flat overlay button facing the camera head-on saturates
        # to white under the headlight's specular otherwise
        self.Attributes().ShadingAspect().Aspect().SetShadingModel(
            Graphic3d_TypeOfShadingModel.Graphic3d_TypeOfShadingModel_Unlit
        )

    def set_hovered(self, hovered):
        """Fill with the highlight color, like the cube's face hover.

        OCCT's dynamic-highlight presentation is coplanar with the arrow
        face and loses the depth test (only its boundary shows), so the
        hover feedback swaps the fill color instead.
        """

        self.SetColor(ARROW_HILIGHT_COLOR if hovered else ARROW_COLOR)

    @staticmethod
    def _make_face(sense):
        """Flat curved-arrow face in pixel units around the local origin."""

        def polar(radius, angle):
            # sense +1 mirrors the right-side arrow across the vertical
            a = radians(180 - angle if sense > 0 else angle)
            return gp_Pnt(radius * cos(a), radius * sin(a), 0)

        mid = (ARROW_INNER_RADIUS + ARROW_OUTER_RADIUS) / 2
        polygon = BRepBuilderAPI_MakePolygon()

        for i in range(ARC_SEGMENTS + 1):
            step = (ARROW_HEAD_ANGLE - ARROW_TAIL_ANGLE) * i / ARC_SEGMENTS
            polygon.Add(polar(ARROW_OUTER_RADIUS, ARROW_TAIL_ANGLE + step))
        polygon.Add(
            polar(ARROW_OUTER_RADIUS + ARROW_HEAD_OVERHANG, ARROW_HEAD_ANGLE)
        )
        polygon.Add(polar(mid, ARROW_HEAD_ANGLE - ARROW_TIP_SWEEP))
        polygon.Add(
            polar(ARROW_INNER_RADIUS - ARROW_HEAD_OVERHANG, ARROW_HEAD_ANGLE)
        )
        for i in range(ARC_SEGMENTS + 1):
            step = (ARROW_TAIL_ANGLE - ARROW_HEAD_ANGLE) * i / ARC_SEGMENTS
            polygon.Add(polar(ARROW_INNER_RADIUS, ARROW_HEAD_ANGLE + step))
        polygon.Close()

        return BRepBuilderAPI_MakeFace(polygon.Wire()).Face()
