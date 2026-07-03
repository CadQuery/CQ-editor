# Example 4: Holes and Counterbores
#
# Holes are one of the most common features in mechanical parts. CadQuery
# provides dedicated methods for the hole types a machinist would recognise:
#
#   .hole(diameter)                            - straight through-hole
#   .cboreHole(diameter, cbore_d, cbore_depth) - counterbored hole (flat-bottomed recess)
#   .cskHole(diameter, csk_d, csk_angle)       - countersunk hole (conical recess)
#
# Holes are placed at the CURRENT STACK POSITIONS. A common pattern is to
# put points on a construction rectangle, select its vertices, and drill
# all four at once.
#
# .workplane() switches the active workplane to the selected face so that
# subsequent operations happen relative to it.
#
# For more examples and the full API reference, see https://cadquery.readthedocs.io/en/latest/

import cadquery as cq

# Plate dimensions
length    = 80.0
width     = 60.0
thickness = 10.0

# Fastener dimensions (M4 socket-head cap screw)
bolt_dia    = 4.4   # clearance hole diameter
cbore_dia   = 8.0   # counterbore diameter
cbore_depth = 4.5   # counterbore depth

result = (
    cq.Workplane("XY")
    .box(length, width, thickness)

    # Switch to the top face of the box.
    .faces(">Z")
    .workplane()

    # Create a rectangle for construction only - its vertices define hole centres.
    # forConstruction=True means the rectangle is not extruded or cut; it just
    # acts as a layout guide for the hole locations.
    .rect(length - 16, width - 16, forConstruction=True)
    .vertices()

    # Drill a counterbored hole at each of the four corner vertices.
    .cboreHole(bolt_dia, cbore_dia, cbore_depth)
)

show_object(result)
