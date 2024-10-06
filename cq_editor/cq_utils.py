import cadquery as cq
from cadquery.occ_impl.assembly import toCAF

from typing import List, Union, Any, Optional
from importlib import reload
from types import SimpleNamespace

from typish import instance_of as isinstance

from OCP.XCAFPrs import XCAFPrs_AISObject
from OCP.TopoDS import TopoDS_Shape
from OCP.AIS import AIS_InteractiveObject, AIS_Shape, AIS_Trihedron, AIS_Point
from OCP.Prs3d import Prs3d_DatumParts, Prs3d_DatumMode
from OCP.TCollection import TCollection_ExtendedString
from OCP.gp import gp_Ax2
from OCP.Geom import Geom_Axis2Placement, Geom_CartesianPoint
from OCP.Quantity import (
    Quantity_TOC_RGB as TOC_RGB,
    Quantity_Color,
    Quantity_NOC_GOLD as GOLD,
    Quantity_NOC_BLUE2,
    Quantity_NOC_GREEN2,
    Quantity_NOC_RED2,
)
from OCP.Graphic3d import Graphic3d_NOM_JADE, Graphic3d_MaterialAspect

from PyQt5.QtGui import QColor

DEFAULT_FACE_COLOR = Quantity_Color(GOLD)
DEFAULT_MATERIAL = Graphic3d_MaterialAspect(Graphic3d_NOM_JADE)
DEFAULT_TRIHEDRON_SIZE = 0.1

CompoundLike = Union[cq.Shape, cq.Workplane, cq.Sketch, cq.Assembly]
AISLike = Union[
    CompoundLike, cq.Location, cq.Plane, cq.Vector, AIS_InteractiveObject, TopoDS_Shape
]
AISLikeLists = Union[tuple(List[T] for T in AISLike.__args__)]

Showable = Union[AISLike, AISLikeLists]


def is_showable(obj: Any) -> bool:
    """
    Check if object is showable.
    """

    return isinstance(obj, Showable)


def ensure_showable(obj: Any):
    """
    Raise if object is not showable.
    """

    # validate
    if not is_showable(obj):
        raise ValueError(f"{obj} has incorrect type {type(obj)}.")


def is_cq_obj(obj):

    from cadquery import Workplane, Shape, Assembly, Sketch

    return isinstance(obj, Union[Workplane, Shape, Assembly, Sketch])


def find_cq_objects(results: dict):

    return {
        k: SimpleNamespace(shape=v, options={})
        for k, v in results.items()
        if is_cq_obj(v)
    }


def to_compound(
    obj: Union[cq.Workplane, List[cq.Workplane], cq.Shape, List[cq.Shape], cq.Sketch]
):

    vals = []

    if isinstance(obj, cq.Workplane):
        vals.extend(obj.vals())
    elif isinstance(obj, cq.Shape):
        vals.append(obj)
    elif isinstance(obj, list) and isinstance(obj[0], cq.Workplane):
        for o in obj:
            vals.extend(o.vals())
    elif isinstance(obj, list) and isinstance(obj[0], cq.Shape):
        vals.extend(obj)
    elif isinstance(obj, TopoDS_Shape):
        vals.append(cq.Shape.cast(obj))
    elif isinstance(obj, list) and isinstance(obj[0], TopoDS_Shape):
        vals.extend(cq.Shape.cast(o) for o in obj)
    elif isinstance(obj, cq.Sketch):
        if obj._faces:
            vals.append(obj._faces)
        else:
            vals.extend(obj._edges)
    else:
        raise ValueError(f"Invalid type {type(obj)}")

    return cq.Compound.makeCompound(vals)


def to_workplane(obj: cq.Shape):

    rv = cq.Workplane("XY")
    rv.objects = [
        obj,
    ]

    return rv


def make_trihedron(ax):

    ais = AIS_Trihedron(Geom_Axis2Placement(ax))
    ais.SetSize(DEFAULT_TRIHEDRON_SIZE)

    # disable labels
    for ax in (
        Prs3d_DatumParts.Prs3d_DatumParts_XAxis,
        Prs3d_DatumParts.Prs3d_DatumParts_YAxis,
        Prs3d_DatumParts.Prs3d_DatumParts_ZAxis,
    ):
        ais.SetLabel(ax, TCollection_ExtendedString())

    ais.SetDatumPartColor(
        Prs3d_DatumParts.Prs3d_DP_XAxis, Quantity_Color(Quantity_NOC_RED2)
    )
    ais.SetDatumPartColor(
        Prs3d_DatumParts.Prs3d_DP_YAxis, Quantity_Color(Quantity_NOC_GREEN2)
    )
    ais.SetDatumPartColor(
        Prs3d_DatumParts.Prs3d_DP_ZAxis, Quantity_Color(Quantity_NOC_BLUE2)
    )

    ais.SetDatumDisplayMode(Prs3d_DatumMode.Prs3d_DM_Shaded)

    return ais


def make_AIS(
    obj: Union[AISLike, AISLikeLists], options={},
) -> Union[List[AIS_InteractiveObject], Optional[cq.Shape]]:

    shape = None
    ais = None
    rv = []

    if isinstance(obj, cq.Assembly):
        label, shape = toCAF(obj)
        ais = XCAFPrs_AISObject(label)

    elif isinstance(obj, AIS_InteractiveObject):
        ais = obj

    elif isinstance(obj, cq.Location):
        ax = gp_Ax2()
        ax.Transform(obj.wrapped.Transformation())

        ais = make_trihedron(ax)

    elif isinstance(obj, cq.Plane):
        ais = make_trihedron(obj.lcs.Ax2())

    elif isinstance(obj, CompoundLike):
        shape = to_compound(obj)
        ais = AIS_Shape(shape.wrapped)

    elif isinstance(obj, cq.Vector):
        ais = AIS_Point(Geom_CartesianPoint(obj.toPnt()))

    elif isinstance(obj, TopoDS_Shape):
        ais = AIS_Shape(obj)

    elif isinstance(obj, AISLikeLists):
        rv = [make_AIS(el, options)[0][0] for el in obj]

    if ais:
        set_material(ais, DEFAULT_MATERIAL)
        set_color(ais, DEFAULT_FACE_COLOR)

        if "alpha" in options:
            set_transparency(ais, options["alpha"])
        if "color" in options:
            set_color(ais, to_occ_color(options["color"]))
        if "rgba" in options:
            r, g, b, a = options["rgba"]
            set_color(ais, to_occ_color((r, g, b)))
            set_transparency(ais, a)
        if "size" in options:
            ais.SetSize(options["size"])

        rv = [ais]

    return rv, shape


def export(
    obj: Union[cq.Workplane, List[cq.Workplane]], type: str, file, precision=1e-1
):

    comp = to_compound(obj)

    if type == "stl":
        comp.exportStl(file, tolerance=precision)
    elif type == "step":
        comp.exportStep(file)
    elif type == "brep":
        comp.exportBrep(file)


def to_occ_color(color) -> Quantity_Color:

    if not isinstance(color, QColor):
        if isinstance(color, tuple):
            if isinstance(color[0], int):
                color = QColor(*color)
            elif isinstance(color[0], float):
                color = QColor.fromRgbF(*color)
            else:
                raise ValueError("Unknown color format")
        else:
            color = QColor(color)

    return Quantity_Color(color.redF(), color.greenF(), color.blueF(), TOC_RGB)


def get_occ_color(obj: Union[AIS_InteractiveObject, Quantity_Color]) -> QColor:

    if isinstance(obj, AIS_InteractiveObject):
        color = Quantity_Color()
        obj.Color(color)
    else:
        color = obj

    return QColor.fromRgbF(color.Red(), color.Green(), color.Blue())


def set_color(ais: AIS_Shape, color: Quantity_Color) -> AIS_Shape:

    drawer = ais.Attributes()
    drawer.SetupOwnShadingAspect()
    drawer.ShadingAspect().SetColor(color)

    return ais


def set_material(ais: AIS_Shape, material: Graphic3d_MaterialAspect) -> AIS_Shape:

    drawer = ais.Attributes()
    drawer.SetupOwnShadingAspect()
    drawer.ShadingAspect().SetMaterial(material)

    return ais


def set_transparency(ais: AIS_Shape, alpha: float) -> AIS_Shape:

    drawer = ais.Attributes()
    drawer.SetupOwnShadingAspect()
    drawer.ShadingAspect().SetTransparency(alpha)

    return ais


def reload_cq():

    # # NB: order of reloads is important
    # reload(cq.types)
    # reload(cq.occ_impl.geom)
    # reload(cq.occ_impl.shapes)
    # reload(cq.occ_impl.shapes)
    # reload(cq.occ_impl.importers.dxf)
    # reload(cq.occ_impl.importers)
    # reload(cq.occ_impl.solver)
    # reload(cq.occ_impl.assembly)
    # reload(cq.occ_impl.sketch_solver)
    # reload(cq.hull)
    # reload(cq.selectors)
    # reload(cq.sketch)
    # reload(cq.occ_impl.exporters.svg)
    # reload(cq.cq)
    # reload(cq.occ_impl.exporters.dxf)
    # reload(cq.occ_impl.exporters.amf)
    # reload(cq.occ_impl.exporters.json)
    # # reload(cq.occ_impl.exporters.assembly)
    # reload(cq.occ_impl.exporters)
    # reload(cq.assembly)
    # reload(cq)
    pass


def is_obj_empty(obj: Union[cq.Workplane, cq.Shape]) -> bool:

    rv = False

    if isinstance(obj, cq.Workplane):
        rv = True if isinstance(obj.val(), cq.Vector) else False

    return rv
