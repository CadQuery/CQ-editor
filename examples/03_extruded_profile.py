# Example 3: Extruded Profile
#
# Rather than starting with a primitive shape, you can draw a 2D profile on a
# workplane and extrude it into a 3D solid. Sketch the cross-section, then
# give it thickness.
#
# The sketch is built by chaining 2D drawing commands:
#   .moveTo(x, y)       - move the pen without drawing
#   .lineTo(x, y)       - draw a line to an absolute position
#   .close()            - connect back to the starting point
#
# .extrude(depth) extrudes the closed profile in the workplane's normal direction.
#
# This example draws an L-shaped bracket profile and extrudes it.

import cadquery as cq

# Dimensions (mm)
flange_width  = 30
flange_height = 5
web_height    = 25
web_thickness = 5

result = (
    cq.Workplane("XY")
    # Trace the L-shape profile clockwise starting at the origin.
    .moveTo(0, 0)
    .lineTo(flange_width, 0)              # bottom edge of flange
    .lineTo(flange_width, flange_height)  # right edge of flange
    .lineTo(web_thickness, flange_height) # step up to the web
    .lineTo(web_thickness, flange_height + web_height)  # top of web
    .lineTo(0, flange_height + web_height)              # top of web (left)
    .close()                              # back to origin
    .extrude(40)                          # give the profile a 40 mm depth
)

show_object(result)
