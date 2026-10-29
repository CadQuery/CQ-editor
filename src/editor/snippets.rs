//! Built-in CadQuery code snippets for common modeling operations.

/// A reusable CadQuery code template that can be inserted into the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snippet {
    pub title: &'static str,
    pub description: &'static str,
    pub code: &'static str,
    pub needs_import: bool,
}

pub const SNIPPETS: &[Snippet] = &[
    Snippet {
        title: "Workplane",
        description: "Create a CadQuery Workplane on the XY plane",
        code: "result = cq.Workplane(\"XY\")\n",
        needs_import: true,
    },
    Snippet {
        title: "Box",
        description: "Create a centered rectangular box on the XY plane",
        code: "result = (\n    cq.Workplane(\"XY\")\n    .box(40, 30, 10)\n)\n\nshow_object(result)\n",
        needs_import: true,
    },
    Snippet {
        title: "Cylinder",
        description: "Create a cylinder defined by height and radius",
        code: "result = (\n    cq.Workplane(\"XY\")\n    .cylinder(30, 10)\n)\n\nshow_object(result)\n",
        needs_import: true,
    },
    Snippet {
        title: "Sphere",
        description: "Create a sphere centered at the workplane origin",
        code: "result = (\n    cq.Workplane(\"XY\")\n    .sphere(15)\n)\n\nshow_object(result)\n",
        needs_import: true,
    },
    Snippet {
        title: "Fillet & Chamfer",
        description: "Round vertical edges with a fillet and bevel the top edges with a chamfer",
        code: "result = (\n    cq.Workplane(\"XY\")\n    .box(40, 30, 15)\n    .edges(\"|Z\")\n    .fillet(3)\n    .faces(\">Z\")\n    .edges()\n    .chamfer(1)\n)\n\nshow_object(result)\n",
        needs_import: true,
    },
    Snippet {
        title: "Extrude Profile",
        description: "Sketch a 2D rectangular profile and extrude it into a 3D solid",
        code: "result = (\n    cq.Workplane(\"XY\")\n    .rect(40, 25)\n    .extrude(12)\n)\n\nshow_object(result)\n",
        needs_import: true,
    },
    Snippet {
        title: "Revolve Profile",
        description: "Revolve a 2D profile around an axis",
        code: "result = (\n    cq.Workplane(\"XZ\")\n    .moveTo(10, 0)\n    .lineTo(20, 0)\n    .lineTo(20, 15)\n    .lineTo(10, 15)\n    .close()\n    .revolve(360, (0, 0, 0), (0, 1, 0))\n)\n\nshow_object(result)\n",
        needs_import: true,
    },
    Snippet {
        title: "Loft Sections",
        description: "Loft smoothly between two cross-section profiles",
        code: "result = (\n    cq.Workplane(\"XY\")\n    .rect(40, 30)\n    .workplane(offset=25)\n    .circle(10)\n    .loft()\n)\n\nshow_object(result)\n",
        needs_import: true,
    },
    Snippet {
        title: "Sweep Along Path",
        description: "Sweep a circular cross-section along a spline path",
        code: "path = cq.Workplane(\"XZ\").spline([(0, 0), (15, 20), (40, 25)])\nresult = (\n    cq.Workplane(\"XY\")\n    .circle(4)\n    .sweep(path)\n)\n\nshow_object(result)\n",
        needs_import: true,
    },
    Snippet {
        title: "Select Faces & Edges",
        description: "Select a face to cut a pocket and round parallel edges",
        code: "result = (\n    cq.Workplane(\"XY\")\n    .box(40, 30, 10)\n    .faces(\">Z\")\n    .workplane()\n    .rect(20, 15)\n    .cutBlind(-4)\n    .edges(\"|Z\")\n    .fillet(2)\n)\n\nshow_object(result)\n",
        needs_import: true,
    },
    Snippet {
        title: "Holes (Through & Counterbore)",
        description: "Create a center through-hole and corner counterbored mounting holes",
        code: "result = (\n    cq.Workplane(\"XY\")\n    .box(50, 40, 10)\n    .faces(\">Z\")\n    .workplane()\n    .hole(12)\n    .faces(\">Z\")\n    .workplane()\n    .rect(36, 26, forConstruction=True)\n    .vertices()\n    .cboreHole(4.5, 8.0, 3.0)\n)\n\nshow_object(result)\n",
        needs_import: true,
    },
    Snippet {
        title: "Assembly",
        description: "Combine multiple parts with locations and colors into a CadQuery Assembly",
        code: "base = cq.Workplane(\"XY\").box(40, 40, 8)\npeg = cq.Workplane(\"XY\").cylinder(20, 6)\n\nassembly = cq.Assembly()\nassembly.add(base, name=\"base\", color=cq.Color(\"steelblue\"))\nassembly.add(\n    peg,\n    name=\"peg\",\n    loc=cq.Location((0, 0, 14)),\n    color=cq.Color(\"orange\"),\n)\n\nshow_object(assembly, name=\"assembly\")\n",
        needs_import: true,
    },
    Snippet {
        title: "Sketch (2D)",
        description: "Construct a 2D Sketch with filleted vertices and a subtracted circle",
        code: "sketch = (\n    cq.Sketch()\n    .rect(40, 25)\n    .vertices()\n    .fillet(4)\n    .circle(6, mode=\"s\")\n)\n\nresult = cq.Workplane(\"XY\").placeSketch(sketch).extrude(8)\nshow_object(result)\n",
        needs_import: true,
    },
    Snippet {
        title: "show_object()",
        description: "Display a CadQuery object in the 3D view with custom name and color options",
        code: "show_object(result, name=\"result\", options={\"color\": (65, 130, 200), \"alpha\": 1.0})\n",
        needs_import: false,
    },
];
