# Example 7: Pillow Block Bearing Housing
#
# A pillow block is a standard mechanical component: a rectangular base with
# a cylindrical bore that holds a bearing, and mounting holes at the corners.
# It is a classic CadQuery example because it exercises most of the core
# concepts in a single, practical part:
#
#   - Parametric dimensions
#   - Box primitive
#   - Face and edge selectors
#   - Workplane switching
#   - Through-hole (bore)
#   - Counterbored mounting holes via a construction pattern
#   - Fillets
#
# All dimensions are in millimetres.
#
# For more examples and the full API reference, see https://cadquery.readthedocs.io/en/latest/

import cadquery as cq

# --- Parameters ---
length          = 80.0   # overall length of the base
width           = 60.0   # overall width of the base
thickness       = 10.0   # thickness of the base plate
bore_dia        = 22.0   # diameter of the bearing bore
bolt_dia        = 4.4    # mounting bolt clearance hole diameter
cbore_dia       = 8.0    # counterbore diameter
cbore_depth     = 4.5    # counterbore depth
corner_fillet   = 2.0    # radius for the vertical corner fillets
edge_margin     = 8.0    # distance from bolt centres to the edge

result = (
    cq.Workplane("XY")

    # Start with a solid rectangular base.
    .box(length, width, thickness)

    # ── Bearing bore ────────────────────────────────────────────────────────
    # Switch to the top face and drill a through-hole on the centre axis.
    .faces(">Z")
    .workplane()
    .hole(bore_dia)

    # ── Mounting holes ───────────────────────────────────────────────────────
    # Return to the top face. Place a construction rectangle inset from the
    # edges; its four vertices define the bolt-hole centres.
    .faces(">Z")
    .workplane()
    .rect(length - edge_margin * 2, width - edge_margin * 2, forConstruction=True)
    .vertices()
    .cboreHole(bolt_dia, cbore_dia, cbore_depth)

    # ── Corner fillets ───────────────────────────────────────────────────────
    # Select the four vertical edges (parallel to Z) and fillet them for a
    # more finished appearance and to remove stress concentrations.
    .edges("|Z")
    .fillet(corner_fillet)
)

show_object(result)
