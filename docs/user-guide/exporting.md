# Exporting and sharing

## Export formats

| Format | Menu | Best for |
| --- | --- | --- |
| **STEP** (`.step`) | Tools > Export as STEP… | Exact CAD geometry with names, hierarchy and colors, including multiple objects |
| **STL** (`.stl`) | Tools > Export as STL… | 3D printing, mesh tools |
| **BREP** (`.brep`) | Tools > Export as BREP… | OpenCascade's native format: lossless, for other OCCT-based tools |
| **3MF** (`.3mf`) | Tools > Export as 3MF… | 3D printing with part names and colors |
| **glTF / GLB** (`.gltf`, `.glb`) | Tools > Export as glTF / GLB… | Web viewers and rendering |
| **XBF** (`.xbf`) | Tools > Export as XBF… | OpenCascade assemblies with names and colors |
| **AMF, VRML, VTK.js, SVG, DXF** | Tools > Export as… | Mesh or drawing interchange |

The same commands are in the **Export** button of the Objects panel and in
the right-click menu of an object.

**What gets exported.** The Export dialog offers the selection (including
its children), visible parts, or everything. STEP exports named assembly
children by default, with world placement and colors even when several
objects were shown separately. You may choose fused STEP output and AP214
or AP242. Other formats preserve the properties their file format supports;
the dialog calls out limitations before export. A broken or empty VTK.js
archive is reported as an error.

**Model units.** Choose what one number in the script means. The dialog uses
the 3D View preference and asks you to choose when it is *None*. STEP records
that unit, 3MF records it in the model, and glTF/GLB convert coordinates to
meters. STL and DXF cannot store a unit; tell the recipient what you chose.

**Geometry check.** Before writing, CQ-editor checks each exported shape for
invalid topology, open shells and non-manifold edges. If it finds a problem,
you can cancel, export anyway, or try OpenCascade ShapeFix on an export copy.
The script and displayed model are unchanged by that repair. **Tools > Check
Geometry** also compares the exact solids of the assembled model. Its report
distinguishes interferences, contacts, clearances and inconclusive calculations;
**Select pair** highlights both pieces and draws their exact shared volume in
red when they interfere. The Objects panel marks checked objects
with topology problems.

An export with interfering pieces pauses for review. **Export anyway** keeps
the geometry exactly as scripted; ShapeFix is offered only for topology defects
and does not resolve an overlap between distinct parts.
For a known press fit or weld, declare the specific pair and reason with
`allow_interference(...)` in the script. Other overlaps remain blocked.

The file dialog proposes the script's name (`bracket.step`) and adds the
extension if you leave it out. The status bar shows *Exporting …* while the
file is written.

**Mesh precision.** The dialog starts with the two values from
*Preferences > Objects* and lets you adjust them for each export. They are
separate from the display mesh. STL can be binary or ASCII.

| Setting | Default | Meaning |
| --- | --- | --- |
| STL precision | 0.1 (model units) | Maximum distance between the mesh and the true surface |
| STL angular precision | 0.1 rad | Maximum angle between neighboring facets |

Lower values give smoother prints and bigger files.

### Exporting from the script

For repeatable output, export from the script with CadQuery:

```python
cq.exporters.export(bracket, "bracket.step")
cq.exporters.export(bracket.section(), "bracket.dxf")
assy.export("assembly.step")
```

With *Change working dir to script dir* on (the default), relative paths are
written next to the script.

## Screenshots

**Tools > Screenshot…** (camera icon in the toolbar) saves the 3D view, at
its current size, as a PNG image.

## Upload Model

**Tools > Upload Model…** sends the model to a web service, for example to
order a part from a manufacturing service.

* **URL**: the endpoint that receives the file. **Presets** fills it in for
  known services. **PCBWay** is built in.
* **File type**: **STEP**, **STL** or **CadQuery script** (the editor's
  source code).
* The selected objects are uploaded, or all objects when nothing is
  selected.

The file is sent as a multipart form field named `file`. If the service
answers with a link (as the PCBWay preset expects), the dialog shows it so
you can continue on the service's website. Otherwise the raw HTTP response
is shown.

> **Privacy.** Uploading sends your model, or your script, to the URL in the
> dialog. Nothing is sent unless you press **Upload**.

## Network access

CQ-editor works offline. It contacts the network only when you ask it to,
and once a day to look for a newer CQ-editor, which you can turn off:

| Action | Contacts |
| --- | --- |
| Tools > Upload Model… | The URL you enter |
| Help > Check for Updates | `api.github.com`, to list CQ-editor and CadQuery releases newer than the ones you have |
| The daily check (*Preferences > General > Check for updates*, on by default) | `api.github.com`, once a day when Python starts, to list CQ-editor's releases; nothing about you or your files is sent, and a failure (offline) is only logged |
| Preferences > General > Create Environment… / Install Packages… | The Python package index, through pip, for the packages you name |
| Help > Documentation / CadQuery Documentation | Opens your web browser |
