# Example 8: Assembly
#
# cq.Assembly lets you combine multiple parts into a single object, position
# them relative to each other, and assign colours for visual clarity. This is
# useful for checking fit, visualising a mechanism, or exporting a multi-body
# STEP file.
#
# Parts are added with .add(), which accepts:
#   - A CadQuery solid or Workplane result
#   - name=        a label used to reference the part later
#   - color=       a cq.Color for display
#   - loc=         a cq.Location specifying position and orientation
#
# cq.Location(cq.Vector(x, y, z)) translates the part.
# cq.Location(cq.Vector(...), cq.Vector(ax, ay, az), angle_degrees) rotates it.
#
# This example bolts two plates together with a gasket between them.
# Stack order, top to bottom:
#
#   bolt head          (head bottom at z = +14)
#   ─────────────────  upper plate (z = +6 to +14)
#   · · · · · · · · ·  gasket      (z = +4 to +6)
#   ─────────────────  lower plate (z = −4 to +4)
#   nut
#
# For more examples and the full API reference, see https://cadquery.readthedocs.io/en/latest/

import cadquery as cq

# --- Plate ------------------------------------------------------------------
# Both plates share the same geometry; they are added to the assembly twice
# at different Z positions.
plate_l, plate_w, plate_t = 50, 40, 8
bolt_clearance = 6.5   # clearance hole

plate = (
    cq.Workplane("XY")
    .box(plate_l, plate_w, plate_t)
    .faces(">Z").workplane()
    .hole(bolt_clearance)
    .edges("|Z").fillet(2)
)

# --- Gasket -----------------------------------------------------------------
# A thin flat ring seated between the two plates.
gasket_t = 2.0

gasket = (
    cq.Workplane("XY")
    .box(plate_l, plate_w, gasket_t)
    .faces(">Z").workplane()
    .hole(bolt_clearance)
)

# --- Bolt (hex-head cap screw, simplified) ----------------------------------
# Start with the hex head, then select its bottom face and extrude the shank
# downward from there - the idiomatic CadQuery approach for connected features.
bolt_shank_dia    = 6.0
bolt_shank_length = 25.0   # long enough to pass through both plates and gasket
bolt_head_dia     = 10.0   # across-flats approximated as a circle here
bolt_head_height  = 4.0

bolt = (
    cq.Workplane("XY")
    .polygon(6, bolt_head_dia)    # 6-sided hex, circumscribed by bolt_head_dia circle
    .extrude(bolt_head_height)
    .faces("<Z")                  # bottom face of the head
    .workplane()
    .circle(bolt_shank_dia / 2)
    .extrude(bolt_shank_length)   # shank grows downward from the head
)

# --- Nut (hex, simplified) --------------------------------------------------
nut_dia    = 10.0   # across-flats
nut_height = 5.0
nut_bore   = bolt_shank_dia + 0.2   # slight clearance on the thread

nut = (
    cq.Workplane("XY")
    .polygon(6, nut_dia)
    .extrude(nut_height)
    .faces(">Z").workplane()
    .hole(nut_bore)
)

# --- Assemble ---------------------------------------------------------------
# The lower plate is centred at the origin (z = 0).
# Everything else is offset relative to it.
upper_plate_z = plate_t + gasket_t              # center of upper plate (= 10 mm)
bolt_z        = upper_plate_z + plate_t / 2     # top face of upper plate (= 14 mm)

assy = (
    cq.Assembly()

    # Lower plate at the origin.
    .add(plate,
         name="plate_lower",
         color=cq.Color("lightgray"))

    # Gasket sits directly on top of the lower plate.
    .add(gasket,
         name="gasket",
         color=cq.Color("olivedrab"),
         loc=cq.Location(cq.Vector(0, 0, plate_t / 2 + gasket_t / 2)))

    # Upper plate sits on top of the gasket.
    .add(plate,
         name="plate_upper",
         color=cq.Color("lightgray"),
         loc=cq.Location(cq.Vector(0, 0, upper_plate_z)))

    # Bolt: head bottom rests on the upper plate's top face.
    # The shank passes through both plates and the gasket.
    .add(bolt,
         name="bolt",
         color=cq.Color("steelblue"),
         loc=cq.Location(cq.Vector(0, 0, bolt_z)))

    # Nut: sits on the underside of the lower plate.
    # Rotate 30° so a flat faces forward, then translate below the lower plate.
    .add(nut,
         name="nut",
         color=cq.Color("steelblue"),
         loc=cq.Location(
             cq.Vector(0, 0, -plate_t / 2 - nut_height),
             cq.Vector(0, 0, 1),    # rotation axis (Z)
             30,                    # rotation angle in degrees
         ))
)

show_object(assy)
