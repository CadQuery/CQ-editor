# Example 5: Loft
#
# A loft connects two or more cross-section profiles on different workplanes
# with a smoothly blended solid. It is useful for organic shapes, aerodynamic
# forms, and transitions between different cross-sections.
#
# The workflow is:
#   1. Sketch a profile on one workplane.
#   2. Move to another workplane (offset along an axis).
#   3. Sketch a second profile.
#   4. Call .loft() to blend between them.
#
# This example transitions from a square base to a circular top, like a
# funnel or adapter fitting.
#
# For more examples and the full API reference, see https://cadquery.readthedocs.io/en/latest/

import cadquery as cq

base_size   = 30   # side length of the square base (mm)
top_radius  = 10   # radius of the circular top (mm)
height      = 40   # overall height of the loft (mm)

result = (
    cq.Workplane("XY")

    # First profile: a square on the XY plane at Z=0.
    .rect(base_size, base_size)

    # Move up to Z=height and sketch the second profile.
    # workplane(offset=height) creates a new XY-parallel plane at that height.
    .workplane(offset=height)

    # Second profile: a circle.
    .circle(top_radius)

    # Loft between the two profiles.
    # ruled=False (default) gives a smooth blend.
    # ruled=True would give straight ruled-surface sides instead.
    .loft()
)

show_object(result)
