import cadquery as cq

from typing import List, Union
from imp import reload

from OCC.Core.AIS import AIS_ColoredShape
from OCC.Core.Quantity import \
    Quantity_TOC_RGB as TOC_RGB, Quantity_Color

def find_cq_objects(results : dict):

    return {k:v for k,v in results.items() if isinstance(v,cq.Workplane)}

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

def make_AIS(obj : cq.Workplane):

    shape = to_compound(obj)
    ais = AIS_ColoredShape(shape.wrapped)

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

    return Quantity_Color(color.redF(),
                          color.greenF(),
                          color.blueF(),
                          TOC_RGB)

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