# Build and check without a window

CQ-editor can run the same CadQuery script and export handlers from a terminal:

```sh
cq-editor build part.py --export out.step --set width=40
cq-editor check part.py --json
```

`build` writes the format named by the output extension: STEP, STL, BREP,
3MF, glTF, GLB, AMF, VRML, VTKJS, XBF, SVG or DXF. It checks geometry before
writing and returns a nonzero exit status if the script, topology, interference
check or export fails. `--allow-interference` explicitly permits export despite
an interfering or inconclusive pair, while invalid topology remains blocked.
For a specific intentional overlap, `allow_interference("first", "second",
reason="press fit")` in the script accepts only that pair. `check` reports each displayed object's exact geometry and checks pairs of
solids for interference. It exits nonzero for invalid topology, open shells,
non-manifold edges, interference or an inconclusive exact comparison.
`--clearance DISTANCE` also lists nearby parts below that distance in model
units. Both commands use the
same `show_object` calls, assembly hierarchy and script directory as the GUI.
Requirements declared with `expect(...)` also appear in `check --json` under
`checks`, with their script line and result. A failed requirement makes
`check` exit nonzero and stops `build` before creating an output file. The GUI
shows the same results in its **Checks** panel.

Use `--set name=value` more than once to override top-level literal assignments
without editing the file. Values are Python scalar literals, such as `40`,
`2.5`, `True` or `'small'`; an unknown parameter is an error. `--unit` sets
the interpretation of model units for export (`millimeter` by default).
`--json` prints one machine-readable result to stdout; script output goes to
stderr. Script failures include their traceback. `--python PATH` selects the
same CadQuery Python environment as the GUI and can appear before or after
the command.

```sh
cq-editor --python /path/to/python check part.py --set width=40 --json
```

The command exits with status 0 on success, 1 for a model or environment
failure, and 2 for invalid command-line syntax.
