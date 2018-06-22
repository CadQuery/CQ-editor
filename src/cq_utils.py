import cadquery as cq

from OCC.AIS import AIS_ColoredShape

def find_cq_objects(results : dict):
    
    return {k:v for k,v in results.items() if isinstance(v,cq.Workplane)}

def make_AIS(obj : cq.Workplane):
    
    ais = AIS_ColoredShape(cq.Compound.makeCompound(obj.vals()).wrapped)
    
    return ais