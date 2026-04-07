# Example 1: Hello Box
#
# The simplest possible CadQuery model: a rectangular box.
#
# Every CadQuery model starts with a Workplane. The string argument ("XY")
# selects the plane the first sketch or operation is relative to:
#   "XY" — the horizontal plane (Z points up)
#   "XZ" — front face of the box
#   "YZ" — side face of the box
#
# .box(length, width, height) creates a box centered on the workplane origin.
#
# For more examples and the full API reference, see https://cadquery.readthedocs.io/en/latest/

import cadquery as cq

result = cq.Workplane("XY").box(10, 10, 5)

show_object(result)
