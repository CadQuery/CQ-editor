import cadquery as cq

from OCC.AIS import AIS_ColoredShape
from OCC.Quantity import \
    Quantity_TOC_RGB as TOC_RGB, Quantity_Color
    
def find_cq_objects(results : dict):
    
    return {k:v for k,v in results.items() if isinstance(v,cq.Workplane)}

def to_compound(obj : cq.Workplane):
    
    return cq.Compound.makeCompound(obj.vals())

def make_AIS(obj : cq.Workplane):
    
    ais = AIS_ColoredShape(to_compound(obj).wrapped)
    
    return ais

def export(obj : cq.Workplane, type : str, file, precision=1e-1):
    
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