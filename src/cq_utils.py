import cadquery as cq

def find_cq_objects(results : dict):
    
    return [(k,v) for k,v in results.items() if isinstance(v,cq.Workplane)]