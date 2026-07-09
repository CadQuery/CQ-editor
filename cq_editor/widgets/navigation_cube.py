from PyQt5.QtCore import QRect

from OCP.AIS import AIS_ViewCube
from OCP.Aspect import Aspect_TOTP_RIGHT_UPPER
from OCP.Graphic3d import (
    Graphic3d_TransformPers,
    Graphic3d_TMF_TriedronPers,
    Graphic3d_Vec2i,
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

