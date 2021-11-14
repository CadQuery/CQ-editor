import cadquery as cq
from cadquery.occ_impl.assembly import toCAF

from typing import List, Union
from imp import reload
from types import SimpleNamespace

from OCP.XCAFPrs import XCAFPrs_AISObject
from OCP.TopoDS import TopoDS_Shape
from OCP.AIS import AIS_Shape, AIS_ColoredShape
from OCP.Quantity import \
    Quantity_TOC_RGB as TOC_RGB, Quantity_Color
    
from PyQt5.QtGui import QColor

def find_cq_objects(results : dict):

    return {k:SimpleNamespace(shape=v,options={}) for k,v in results.items() if isinstance(v,cq.Workplane)}

def to_compound(obj : Union[cq.Workplane, List[cq.Workplane], cq.Shape, List[cq.Shape], cq.Sketch]):

    vals = []

    if isinstance(obj,cq.Workplane):
        vals.extend(obj.vals())
    elif isinstance(obj,cq.Shape):
        vals.append(obj)
    elif isinstance(obj,list) and isinstance(obj[0],cq.Workplane):
        for o in obj: vals.extend(o.vals())
    elif isinstance(obj,list) and isinstance(obj[0],cq.Shape):
        vals.extend(obj)
    elif isinstance(obj, TopoDS_Shape):
        vals.append(cq.Shape.cast(obj))        
    elif isinstance(obj,list) and isinstance(obj[0],TopoDS_Shape):
        vals.extend(cq.Shape.cast(o) for o in obj)
    elif isinstance(obj, cq.Sketch):
        if obj._faces:
            vals.append(obj._faces)
        else:
            vals.extend(obj._edges)
    else:
        raise ValueError(f'Invalid type {type(obj)}')

    return cq.Compound.makeCompound(vals)

def to_workplane(obj : cq.Shape):

    rv = cq.Workplane('XY')
    rv.objects = [obj,]

    return rv

def make_AIS(obj : Union[cq.Workplane, List[cq.Workplane], cq.Shape, List[cq.Shape], cq.Assembly, AIS_Shape],
             options={}):

    shape = None

    if isinstance(obj, cq.Assembly):
        label, shape = toCAF(obj)
        ais = XCAFPrs_AISObject(label)
    elif isinstance(obj, AIS_Shape):
        ais = obj
    else:
        shape = to_compound(obj)
        ais = AIS_ColoredShape(shape.wrapped)
   
    if 'alpha' in options:
        ais.SetTransparency(options['alpha'])
    if 'color' in options:
        ais.SetColor(to_occ_color(options['color']))
    if 'rgba' in options:
        r,g,b,a = options['rgba']
        ais.SetColor(to_occ_color((r,g,b)))
        ais.SetTransparency(a)

    return ais,shape

def export(obj : Union[cq.Workplane, List[cq.Workplane]], type : str,
           file, precision=1e-1):

    comp = to_compound(obj)

    if type == 'stl':
        comp.exportStl(file, tolerance=precision)
    elif type == 'step':
        comp.exportStep(file)
    elif type == 'brep':
        comp.exportBrep(file)

def to_occ_color(color) -> Quantity_Color:
    
    if not isinstance(color, QColor):
        if isinstance(color, tuple):
            if isinstance(color[0], int):
                color = QColor(*color)
            elif isinstance(color[0], float):
                color = QColor.fromRgbF(*color)
            else:
                raise ValueError('Unknown color format')
        else:
            color = QColor(color)

    return Quantity_Color(color.redF(),
                          color.greenF(),
                          color.blueF(),
                          TOC_RGB)

def get_occ_color(ais : AIS_ColoredShape) -> QColor:
    
    color = Quantity_Color()
    ais.Color(color)
    
    return QColor.fromRgbF(color.Red(), color.Green(), color.Blue())

def reload_cq():
    
    # NB: order of reloads is important
    reload(cq.types)
    reload(cq.occ_impl.geom)
    reload(cq.occ_impl.shapes)
    reload(cq.occ_impl.importers.dxf)
    reload(cq.occ_impl.importers)
    reload(cq.occ_impl.solver)
    reload(cq.occ_impl.assembly)
    reload(cq.occ_impl.sketch_solver)
    reload(cq.hull)
    reload(cq.selectors)
    reload(cq.sketch)
    reload(cq.occ_impl.exporters.svg)
    reload(cq.cq)
    reload(cq.occ_impl.exporters.utils)
    reload(cq.occ_impl.exporters.dxf)
    reload(cq.occ_impl.exporters.amf)
    reload(cq.occ_impl.exporters.json)
    #reload(cq.occ_impl.exporters.assembly)
    reload(cq.occ_impl.exporters)
    reload(cq.assembly)
    reload(cq)
    
    
def is_obj_empty(obj : Union[cq.Workplane,cq.Shape]) -> bool:
    
    rv = False
    
    if isinstance(obj, cq.Workplane):
        rv = True if isinstance(obj.val(), cq.Vector) else False
        
    return rv
