import cadquery as cq

from typing import List, Union, Tuple
from imp import reload
from types import SimpleNamespace

from OCC.Core.AIS import AIS_ColoredShape
from OCC.Core.Quantity import \
    Quantity_TOC_RGB as TOC_RGB, Quantity_Color
    
from PyQt5.QtGui import QColor

def find_cq_objects(results : dict):

    return {k:SimpleNamespace(shape=v,options={}) for k,v in results.items() if isinstance(v,cq.Workplane)}

def to_compound(obj : Union[cq.Workplane, List[cq.Workplane]]):

    vals = []

    if isinstance(obj,cq.Workplane):
        vals.extend(obj.vals())
    else:
        for o in obj: vals.extend(o.vals())

    return cq.Compound.makeCompound(vals)

def to_workplane(obj : cq.Shape):

    rv = cq.Workplane('XY')
    rv.objects = [obj,]

    return rv

def make_AIS(obj : Union[cq.Workplane, cq.Shape], options={}):
    
    if isinstance(obj, cq.Shape):
        obj = to_workplane(obj)

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
        comp.exportStl(file,precision=precision)
    elif type == 'step':
        comp.exportStep(file)
    elif type == 'brep':
        comp.exportBrep(file)

def to_occ_color(color):
    
    if not isinstance(color, QColor):
        if isinstance(color, tuple):
            if isinstance(color[0], int):
                color = QColor(*color)
            elif isinstance(color[0], float):
                color = QColor.fromRgbF(*color)
        else:
            color = QColor(color)

    return Quantity_Color(color.redF(),
                          color.greenF(),
                          color.blueF(),
                          TOC_RGB)

def get_occ_color(ais : AIS_ColoredShape):
    
    color = Quantity_Color()
    ais.Color(color)
    
    return QColor.fromRgbF(color.Red(), color.Green(), color.Blue())

def reload_cq():
    
    # NB: order of reloads is important
    reload(cq.occ_impl.geom)
    reload(cq.occ_impl.shapes)
    reload(cq.occ_impl.importers)
    reload(cq.occ_impl.exporters)
    reload(cq)
    reload(cq.selectors)
    reload(cq.cq)
    reload(cq)
    
    
def is_obj_empty(obj : Union[cq.Workplane,cq.Shape]) -> bool:
    
    rv = False
    
    if isinstance(obj, cq.Workplane):
        rv = True if isinstance(obj.val(), cq.Vector) else False
        
    return rv