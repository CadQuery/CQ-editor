# Example 6: Sweep
#
# A sweep extrudes a 2D cross-section along a 3D path (the "spine"). Unlike a
# straight extrude, the path can curve, making sweep ideal for pipes, channels,
# handles, and wiring conduits.
#
# The workflow:
#   1. Build the path (spine) as a Wire using CadQuery's 2D drawing commands,
#      then convert it with .wire().
#   2. On a workplane perpendicular to the start of the spine, draw the profile.
#   3. Call .sweep(path) to extrude the profile along the spine.
#
# This example creates a curved pipe — a circular cross-section swept along
# a quarter-circle arc.
#
# For more examples and the full API reference, see https://cadquery.readthedocs.io/en/latest/

import cadquery as cq

pipe_radius   = 3    # outer radius of the pipe cross-section (mm)
sweep_radius  = 30   # radius of the quarter-circle sweep path (mm)

# Build the sweep path: a quarter-circle arc in the XZ plane.
# makeLine/makeArc helpers return Edge objects; Wire.assembleEdges()
# joins them into a single Wire the sweep can follow.
path = (
    cq.Workplane("XZ")
    .moveTo(sweep_radius, 0)
    .radiusArc((0, sweep_radius), -sweep_radius)  # quarter-circle arc
    .wire()
)

result = (
    # Start the profile on the XY plane at the beginning of the path.
    cq.Workplane("XY")
    .workplane(offset=0)
    .moveTo(sweep_radius, 0)   # centre of the profile matches path start
    .circle(pipe_radius)       # circular cross-section
    .sweep(path)               # follow the arc
)

show_object(result)
