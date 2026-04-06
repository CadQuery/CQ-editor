# Example 2: Selectors and Fillets
#
# CadQuery uses selectors to pick specific edges, faces, or vertices from a
# solid so you can apply operations to them. This example rounds the four
# vertical edges of a box using a selector string.
#
# Common edge selectors:
#   "|Z"  — edges parallel to the Z axis
#   "|X"  — edges parallel to the X axis
#   ">Z"  — the edge(s) furthest in the +Z direction
#   "#Z"  — edges perpendicular to Z (i.e. lying in a horizontal plane)
#
# .fillet(radius) rounds the selected edges.
# .chamfer(length) cuts a 45-degree bevel instead.

import cadquery as cq

result = (
    cq.Workplane("XY")
    .box(10, 10, 5)
    # Select the four edges running vertically (parallel to Z) and fillet them.
    .edges("|Z")
    .fillet(1.5)
)

show_object(result)
