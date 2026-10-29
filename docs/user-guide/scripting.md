# Writing scripts

A CQ-editor script is a normal CadQuery Python file. It needs no special
imports and no special structure. The same file runs with plain `python`, in
CQ-editor, or in other CadQuery tools that support `show_object`. CQ-editor
adds a few names to the script's namespace so you can display results.

## The script API

| Name | Purpose |
| --- | --- |
| `show_object(obj, name=None, options=None, **options)` | Display `obj` in the 3D view and the Objects panel |
| `debug(obj, name=None)` | Display `obj` as a translucent red overlay, handy for checking a selection or a construction shape |
| `expect(obj, name=None)` | Record CAD requirements for the Checks panel and headless CI |
| `log(value)` | Write `str(value)` to the Log panel |
| `rand_color(alpha=1.0, cfloat=False)` | A random, not-too-bright color for `show_object` options |
| `cq` | The `cadquery` module, even if the script did not import it |

The interactive console offers the same names, plus `show` as a short alias
of `show_object`.

## `show_object`

```python
show_object(result)                                   # name taken from the variable: "result"
show_object(result, name="housing")                   # explicit name
show_object(result, options={"color": "steelblue"})   # options as a dict…
show_object(result, color="steelblue", alpha=0.5)     # …or as keyword arguments
```

**Naming.** When `name` is omitted, the editor uses the variable that holds
the object in the calling code. If there is none (for example
`show_object(box.faces(">Z"))`), it uses the type and a counter, such as
*Workplane 1*. Showing a second object with the same name replaces the first
one.

**What can be shown:**

* `cq.Workplane`: every shape on its stack (sketches included);
* any `cq.Shape` (`Solid`, `Compound`, `Face`, `Wire`, `Edge`, `Vertex`, …);
* `cq.Sketch`, with its placement applied;
* `cq.Assembly`, with its hierarchy, locations and colors;
* a raw OCP `TopoDS_Shape`;
* a list of any of the above, fused into one object.

Non-shape stack entries (vectors, locations) are skipped. If an object
cannot be displayed, the reason goes to the Log panel and the other objects
are still shown.

### Display options

| Option | Value | Effect |
| --- | --- | --- |
| `color` | a color (see below) | Face color. A color with an alpha below 1 also sets the opacity |
| `alpha` | float, 0–1 | Opacity: `1` is opaque, `0` invisible. Values are clamped to 0–1 |
| `rgba` | `(r, g, b, a)` | Color and opacity in one tuple |
| `edgecolor` | a color | Color of the edges |
| `material` | `"matte plastic"`, `"glossy plastic"`, `"brushed steel"`, `"aluminum"`, `"wood"` or `"default"` | How the faces are shaded (see [Materials](viewer.md#materials)). Case, spaces, hyphens and underscores do not matter; `"aluminium"` works too. Metals and wood have a color of their own for objects without a `color` |

```python
show_object(organizer, color="green", material="matte plastic")
show_object(shaft, material="brushed steel")      # steel grey
```

Accepted color forms:

```python
"red", "steelblue", "lightgray"     # names known to OpenCascade
"#4682b4", "#48b", "#4682b480"      # hex: #rgb, #rgba, #rrggbb, #rrggbbaa
(70, 130, 180)                      # 0–255 integers
(0.27, 0.51, 0.71, 0.8)             # 0–1 floats, optional alpha
cq.Color("steelblue")               # CadQuery colors
```

An invalid option, such as a NaN alpha, is ignored with a warning in the
Log panel, and the other options of the object still apply. It never stops
the script. An object that cannot be drawn, such as one placed at a NaN
coordinate, is left out with an error in the Log panel; the other objects
are shown. Objects without a color use *Preferences > 3D View > Default
object color* (gold by default).

### Scripts without `show_object`

If a script never calls `show_object`, CQ-editor shows **every CadQuery
object bound to a public variable** (a name not starting with `_`). This
keeps quick experiments short:

```python
import cadquery as cq
plate = cq.Workplane().box(30, 20, 4)
boss = plate.faces(">Z").workplane().circle(5).extrude(6)
# plate and boss are both displayed
```

## Assemblies

```python
assy = (
    cq.Assembly()
    .add(plate, name="plate", color=cq.Color("lightgray"))
    .add(bolt, name="bolt", color=cq.Color("steelblue"),
         loc=cq.Location(cq.Vector(0, 0, 14)))
)
show_object(assy)
```

* The Objects panel mirrors the assembly tree. Each part can be hidden,
  recolored or exported on its own.
* Parts inherit the color of their parent when they have none.
* Repeated parts share one mesh, so large assemblies stay light.
* *Preferences > Objects > Merge assemblies* shows the whole assembly as one
  object instead.
* Exporting a single assembly to STEP keeps its names, structure and colors.

Example *08 Assembly* (**File > Examples**) is a complete bolted joint.

### Dimensional fit checks

Call `require_fit` in the script to make rendering, headless checking and
building fail when the stated worst-case cylindrical fit cannot meet its
requirements:

```python
require_fit(
    hole_diameter=6.5,
    pin_diameter=6.0,
    hole_lower=-0.1,
    hole_upper=0.1,
    pin_lower=-0.05,
    pin_upper=0.05,
    center_offset=0.1,
    required_clearance=0.1,
)
```

All numbers use the same model unit. Lower and upper deviations are signed;
`center_offset` is the maximum radial misalignment and consumes twice that
value of diametral clearance. Optional pairs `insertion_depth`/`hole_depth`,
`head_diameter`/`counterbore_diameter` and
`head_height`/`counterbore_depth` check blind-hole and head seating dimensions.
`cylindrical_fit(...)` returns the report without raising. This dimensional
calculation is conservative and does not identify an actual hole in a shape;
**Check Geometry** separately verifies the solids as modeled.

### Intentional interference

An intentional press fit or welded overlap can be declared in the model script:

```python
allow_interference("plate", "pin", reason="specified press fit")
```

The two names must each identify one solid in the checked scene. Give parts
unique names; merged assemblies also accept the full path shown in
**Check Geometry** if a basename is ambiguous. The
check still reports the exact overlap and its reason. Only that pair is
accepted for export; other collisions and inconclusive Boolean calculations
still block it. A new script run replaces the declarations, so an old
allowance cannot silently apply to a new model.

### Assembly motion paths

For insertion or moving mechanisms, declare a path in the script. Each pose is
`(x, y, z, rx, ry, rz)`: translation in model units and XYZ Euler rotation in
degrees. The moving shape is defined around its local origin; obstacles remain
fixed in world coordinates.

```python
require_clear_path(
    pin,
    [plate, bracket],
    [(0, 0, -20, 0, 0, 0), (0, 0, 0, 0, 0, 0)],
    required_clearance=0.1,
)
```

`trace_path(...)` returns `clear`, `blocked` or `inconclusive` without raising.
`require_clear_path(...)` stops the run unless the *whole* path is certified
clear. A pose budget and subdivision limit prevent an indefinite check; contact
or numerical uncertainty can therefore remain inconclusive. Paths are
explicit because the final assembly cannot reveal how a part was inserted.

### Assembly constraint diagnostics

After calling CadQuery's `assy.solve()`, use `assembly_diagnostics(assy)` for
the constraint list, unconstrained components, solver status and residual,
a conservative lower bound on remaining degrees of freedom, and exact
interference results. `require_constrained_assembly(assy)` fails the script
unless the report is clear. A positive degree-of-freedom lower bound proves
underconstraint; zero does not prove full constraint because redundant or
dependent constraints can hide further freedom. Nested assemblies return an
inconclusive degree-of-freedom result instead of a false assurance.

### Material, beam and tool-access checks

For prismatic parts, declare engineering material properties and a section in
the script. The following check includes self-weight, an end load, yielding,
deflection and a minimum wall thickness:

```python
steel = EngineeringMaterial("Steel", density_g_cm3=7.85,
                            youngs_modulus_mpa=210000, yield_strength_mpa=250)
tube = BeamSection.hollow_rectangle(20, 20, wall_mm=1.5)
require_beam(section=tube, material=steel, length_mm=700,
             transverse_load_n=100, required_safety_factor=2,
             maximum_deflection_mm=10, required_wall_mm=1.2)
```

Dimensions are millimeters, forces newtons and stresses megapascals. Supported
sections are `rectangle`, `hollow_rectangle` and `round_tube`; support cases are
`cantilever` (end point load) and `simply_supported_center_load` (center point
load). `beam_check` returns the calculation without raising. Compression is
checked against Euler buckling; free thermal expansion or restrained thermal
stress can be included explicitly. The model is a one-dimensional linear
elastic beam with ideal supports. It does not evaluate local buckling, welds,
joint stiffness or a general three-dimensional stress field.

For tool access, `tool_access(diameter, length, obstacles, poses)` checks a
cylindrical tool envelope along the declared approach path;
`require_tool_access(...)` stops the script if blocked or inconclusive. Model
the actual wrench, drill or driver envelope and its approach when a cylinder
is insufficient.

### CAD requirements

Use `expect` to keep measurable requirements next to the model. Every call
records a result; a failure does not stop subsequent checks. The **Checks**
panel lists passes and failures, and clicking a row opens its script line.

```python
part = cq.Workplane().box(20, 10, 4)
pin = cq.Workplane().circle(2).extrude(4).translate((30, 0, 0))
expect(part, "housing").is_valid().volume_between(790, 810)
expect(part, "envelope").dimensions_within(x=(19.99, 20.01), z=(3.99, 4.01))
expect(part, "pin gap").clearance_at_least(pin, 5)
show_object(part)
show_object(pin)
```

Ranges are inclusive and use the script's model units (volume uses cubed
model units). `is_valid()` checks topology, open shells and non-manifold
edges; `volume_between()` requires a solid. `dimensions_within()` accepts
any combination of `x`, `y` and `z` ranges. `clearance_at_least()` measures
the minimum exact distance between two shapes; touching or overlapping
shapes have zero clearance. Leave room for geometric tolerance in exact
dimension ranges. Invalid arguments such as a reversed range raise a script
error. A general minimum-wall-thickness test is not provided: the beam
section check above applies only to the declared beam geometry.

`expect` is a CQ-editor script helper. Scripts intended to run in other
CadQuery tools can guard these optional requirements or provide their own
compatible helper.

## `debug`

`debug(obj)` displays `obj` in translucent red (`color="red"`, `alpha=0.2`)
on top of the model:

```python
edges = part.edges("|Z")
debug(edges)            # are these the edges I meant?
part = edges.fillet(1)
```

## Output: `print` and `log`

* `print(...)` output from a script goes to the **Log** panel (source
  `print`). Text written to `stderr` appears there as a warning.
* `log(value)` writes an *INFO* entry (source `script`).
* Python `logging` records at level INFO and above are forwarded too.
* Messages printed by native libraries (OpenCascade warnings) are captured
  and shown as well.

In the **Console**, output appears in the console itself instead.

## The script environment

Each run starts from a fresh module called `__cq_main__`, so no variable
leaks from one run into the next. Some behavior is configurable in
*Preferences > Execution*:

| Behavior | Default | Notes |
| --- | --- | --- |
| The script's folder is added to `sys.path` | on | `import my_helpers` works for a file next to the script |
| The working directory is the script's folder | on | Relative paths such as `cq.importers.importStep("part.step")` resolve next to the script |
| Local modules are re-imported on every run | on | Edits to helper modules are picked up. Installed packages (standard library, site-packages) are never reloaded |
| CadQuery itself is re-imported before each run | off | Only useful when developing CadQuery |

More details:

* `__file__` is set to the script's path once the script has been saved.
* `rand_color()` is seeded the same way before every run, so "random"
  colors are stable from run to run.
* `input()` raises `EOFError`: scripts cannot read from the terminal.
* `sys.exit()` ends the script early without an error.
* Folder and environment settings apply only while the script runs.

## Errors

When a script raises an exception:

* the line is highlighted in the editor, and the status bar shows the
  exception and the line number;
* the **Traceback** panel lists the frames from your script onwards (frames
  inside CQ-editor are hidden). Click a row in your script to jump to it;
* the full traceback is also written to the Log panel;
* the model of the previous successful run stays on screen.

Syntax errors are reported the same way, with the line of the error.

## Patterns that work well

**Keep parameters at the top.** Dimensions as variables make the model easy
to tweak, and they show up in the Variables panel:

```python
length, width, thickness = 80, 60, 10
hole_d = 8.5
```

**Split big models into modules.** Put shared parts in `parts.py` next to the
script and `import parts`. With *Reload imported modules* on, edits to
`parts.py` apply on the next run. With *Autoreload: watch imported modules*,
saving `parts.py` even triggers the run. Open the folder (**File > Open
Folder…**) to edit the modules in tabs while F5 keeps rendering the main
script: see [Projects and multiple files](projects.md).

**Show intermediate steps while you work** and remove them later:

```python
show_object(sketch_profile, name="profile", alpha=0.3)
show_object(result)
```

**Use the console to experiment** on the last run's objects without editing
the script: `result.faces(">Z").val().Area()`.
