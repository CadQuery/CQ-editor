# Example 9: Square Tube Workbench
#
# A simple welded workbench made of 20 x 20 mm square steel tube, with a
# wooden top. Every tube is cut square (90 degrees), which makes it a good
# first welding project.
#
#   Frame outside dimensions: 880 x 420 x 720 mm
#   Suggested top:            950 x 500 x 25 mm (final height 745 mm)
#
# Each tube is a hollow box: a solid box minus a slightly longer inner box,
# so both ends are open. A helper function places every tube in a
# cq.Assembly with a cq.Location and records its length, so the script can
# print a summary of the tube it uses (see the Log panel).
#
# Cutting plan (five 2000 mm bars):
#   Bar 1: 880 + 700 + 380 = 1960 mm
#   Bar 2: 880 + 700 + 380 = 1960 mm
#   Bar 3: 840 + 700 + 380 = 1920 mm
#   Bar 4: 700 + 380 + 380 = 1460 mm
#   Bar 5: uncut (2000 mm spare)
#
# Weld beads and the fasteners of the top are not modelled. Do not use this
# design for heavy loads without a proper structural check.
#
# For more examples and the full API reference, see https://cadquery.readthedocs.io/en/latest/

import cadquery as cq

# --- Dimensions (mm) --------------------------------------------------------
TUBE = 20.0                # outside size of the square tube
WALL = 1.5                 # for display only; check your tube
LENGTH = 880.0             # outside length of the top frame
DEPTH = 420.0              # outside depth of the top frame
LEG = 700.0                # from the floor to the underside of the top frame
LOWER_RAIL_HEIGHT = 180.0  # axis of the lower rails above the floor

TOP_WIDTH = 950.0
TOP_DEPTH = 500.0
TOP_THICKNESS = 25.0

METAL_COLOR = cq.Color(0.14, 0.16, 0.18)
WOOD_COLOR = cq.Color(0.77, 0.58, 0.39)

bench = cq.Assembly(name="Workbench - 20x20 square tube")
parts = []  # (name, length) of every tube


def add_tube(name, axis, length, center):
    """Add a hollow square tube along X, Y or Z, with square-cut ends."""
    outer = [TUBE, TUBE, TUBE]
    inner = [TUBE - 2 * WALL] * 3
    index = {"x": 0, "y": 1, "z": 2}[axis]
    outer[index] = length
    inner[index] = length + 2.0  # opens both ends
    tube = cq.Workplane("XY").box(*outer).cut(
        cq.Workplane("XY").box(*inner)
    )
    bench.add(
        tube,
        name=name,
        color=METAL_COLOR,
        loc=cq.Location(cq.Vector(*center)),
    )
    parts.append((name, length))


# --- Top frame: 880 x 420 mm ------------------------------------------------
# Two long rails, with the short side rails welded between them.
# Every frame tube is centred at z = 710.
frame_z = LEG + TUBE / 2
corner_x = LENGTH / 2 - TUBE / 2  # 430
corner_y = DEPTH / 2 - TUBE / 2   # 200

for side, sy in (("front", 1), ("back", -1)):
    add_tube(
        "top_frame_" + side, "x", LENGTH,
        (0, sy * corner_y, frame_z),
    )

for side, sx in (("left", -1), ("right", 1)):
    add_tube(
        "top_frame_" + side, "y", DEPTH - 2 * TUBE,
        (sx * corner_x, 0, frame_z),
    )

# An extra cross member supports the middle of the top.
add_tube(
    "center_top_support", "y", DEPTH - 2 * TUBE,
    (0, 0, frame_z),
)

# --- Legs: four 700 mm tubes, welded under the frame corners ----------------
for sx, side in ((-1, "left"), (1, "right")):
    for sy, end in ((-1, "back"), (1, "front")):
        add_tube(
            "leg_" + side + "_" + end,
            "z", LEG, (sx * corner_x, sy * corner_y, LEG / 2),
        )

# --- Lower bracing: a U, open at the front so knees do not hit a rail -------
# 180 mm is the height of the rails' CENTRE, not of their top face.
add_tube(
    "lower_rail_back", "x", LENGTH - 2 * TUBE,
    (0, -corner_y, LOWER_RAIL_HEIGHT),
)
for sx, side in ((-1, "left"), (1, "right")):
    add_tube(
        "lower_rail_" + side, "y", DEPTH - 2 * TUBE,
        (sx * corner_x, 0, LOWER_RAIL_HEIGHT),
    )

# --- Wooden top, resting on the frame (fasteners not modelled) --------------
top = cq.Workplane("XY").box(TOP_WIDTH, TOP_DEPTH, TOP_THICKNESS)
bench.add(
    top,
    name="wooden_top_950x500x25",
    color=WOOD_COLOR,
    loc=cq.Location(cq.Vector(0, 0, LEG + TUBE + TOP_THICKNESS / 2)),
)

show_object(bench, name="Compact 20x20 square tube workbench")

# Optional: export from the script itself (may need extra packages):
# bench.save("square_tube_workbench.step")

# This summary appears in the Log panel.
total_length = sum(length for _, length in parts)
print("Square tube pieces:", len(parts))
print("Tube length used:", total_length, "mm")
print("Final height:", LEG + TUBE + TOP_THICKNESS, "mm")
