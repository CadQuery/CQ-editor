# Architecture

CQ-editor is two programs that talk over a pipe:

```
┌─────────────────────────────────────┐   requests (JSON lines, stdin)   ┌──────────────────────────────────┐
│ cq-editor  (Rust)                   │ ───────────────────────────────► │ python -m cq_worker              │
│                                     │                                  │                                  │
│ egui + egui_dock   window, panels   │   messages (JSON lines, stdout)  │ runs scripts with CadQuery       │
│ wgpu               3D renderer      │ ◄─────────────────────────────── │ tessellates shapes (OCCT)        │
│ scene, picking, camera, documents   │                                  │ debugger, inspector, console     │
│ recovery, file watching, settings   │   stderr → Log panel             │ export, upload, completion (jedi)│
└─────────────────────────────────────┘ ◄─────────────────────────────── └──────────────────────────────────┘
```

**Why two processes?** CadQuery and OpenCascade are Python and C++ code that
can block for a long time or crash on bad geometry. Running them in a child
process means:

* the interface never freezes, even during a long boolean operation;
* *Stop* is reliable: the GUI kills the process and starts a fresh one;
* a segmentation fault inside OpenCascade costs a restart, not your
  unsaved work;
* the GUI does not depend on Python at build time. Any environment with
  CadQuery can be used at run time.

The worker package (`worker/cq_worker`) and the example scripts are compiled
into the executable by `build.rs`. At run time `embedded::worker_dir`
extracts the package into the data folder (`worker-<content hash>/`) and puts
it on `PYTHONPATH`. Debug builds use `worker/` from the source tree directly,
and `CQ_EDITOR_WORKER_DIR` overrides both.

## Rust side (`src/`)

| Module | Responsibility |
| --- | --- |
| `main.rs` | Command line, window options, start-up |
| `app/` | Application state (`AppState`), the frame loop and action dispatch (`mod.rs`); `CqEditorApp` wraps it with the dock layout and implements `eframe::App`. One `impl AppState` module per concern: `documents.rs` (open, save, reload, recovery, history), `tabs.rs` (the open documents, the session, the script F5 runs), `projects.rs` (the project folder, its main script, file operations), `runs.rs` (render, debug, stop), `worker_events.rs` (worker lifecycle, message routing), `watching.rs` (automatic reload), `services.rs` (selection, measurement, inspector, export, console, upload). `execution.rs` is the pure state machine of runs and pending requests |
| `commands.rs` | `Action`, everything the user can ask for, and the keyboard shortcut table |
| `document.rs` | An open script: UTF-8 decoding (Windows-1252 on request, saved back as UTF-8), BOM and line-ending preservation, atomic saves, external change classification |
| `recovery.rs` | Crash-recovery session (every tab with unsaved changes) and bounded local history |
| `project.rs` | The project folder: cached listings of its tree, the main-script guess, checked names, create/rename/delete |
| `file_watcher.rs` | Polling watcher behind *Automatic reload* |
| `settings.rs` | Preferences, persisted by eframe |
| `scene.rs` | What the view shows: nodes, parts, meshes, visibility, colors, selection, topology selection, inspector overlay |
| `spatial.rs` | Bounding-volume hierarchies for picking: triangles, and generic boxes for scene parts, edge segments and vertices |
| `viewer/` | 3D view: `camera.rs` (orbit camera, fixed-point orbit, standard views, animation), `renderer.rs` (wgpu pipelines), `shaders.wgsl`, `picking.rs`, `overlays.rs` (navigation cube, axes indicator), `section.rs` (section plane), `views.rs` (named views), `capture.rs` (screenshots, turntables), `culling.rs` (frustum culling, level of detail), `ground.rs` (floor grid under the model with fading edges, scale bar), `shadow.rs` and `shadow.wgsl` (the Presentation mode's ground shadow), `stereo.rs` and `stereo.wgsl` (stereo composition) |
| `editor/` | Code editor: `highlight.rs` (Python tokenizer), `text_ops.rs` (pure text edits: indent, comment, auto-indent, replace, names, *Go to line*, breakpoint relocation), `history.rs` (the undo history: typing grouped into steps, one step per command or reload) |
| `panels/` | Objects, Measurements, Variables, CQ Inspector, Console, Traceback, Log, Project, and the default dock layout |
| `dialogs/` | Preferences, Upload, About, Screenshot, Python environments, unsaved changes, external change, recovery, local history, messages |
| `ui/` | Menu bar, toolbar, status bar, the editor's tab strip |
| `worker/` | Child process management and the reason a start failed (`mod.rs`), interpreter discovery and the interpreters offered when it fails (`launch.rs`), protocol types (`protocol.rs`) |
| `environments.rs` | Creating virtual environments and running `pip install` from the preferences: the commands, package checks, and a cancellable background job that collects their output |
| `embedded.rs` | Access to the files embedded by `build.rs` |
| `logging.rs` | The bounded, application-wide log behind the Log panel |
| `theme.rs` | Light/dark visuals, fonts, editor and viewer colors |

### The frame loop and actions

egui is an immediate-mode toolkit: the whole interface is rebuilt every
frame. To keep that manageable, widgets **never change application state
directly**. They push an `Action` (see `commands.rs`), and `app/` runs the
queued actions after the UI of the frame has been built:

```
logic         process worker events · poll file watcher · recovery checkpoint
begin_frame   apply theme/scale · collect shortcuts
build UI      menu bar · toolbar · dock (editor, view, panels) · status bar · dialogs   → push Actions
execute       for each Action: open/save/run/select/export/…  → may send worker Requests
end_frame     handle window close requests (unsaved changes guard)
```

`logic` is eframe's `App::logic`: it runs before every frame and also while
the window is minimized or hidden, whenever a repaint was requested. Worker
replies, automatic reload and recovery checkpoints therefore never wait for
the window to be drawn.

Menus, toolbar buttons, shortcuts and context menus therefore share one code
path per command, and the shortcut shown in a menu comes from the same table
that handles the key press.

### Documents and tabs

`AppState::document` and `AppState::editor` are always the **shown** tab's,
so the code that works on "the document" (editing, completion, search,
saving, the window title) needs no notion of tabs. The other open documents
are *parked* in `EditorTabs` (`app/tabs.rs`) with their own `CodeEditor`;
showing a tab swaps it with the shown one. Each editor has its own egui id,
so egui keeps the caret and scrolling of every tab, and the editor itself
keeps its undo history, breakpoints and markers.

Every document has a *generation*, its identity for as long as it is open.
A run records the generation of the script it executes (`RunScript`), which
is not always the shown one: in a project, F5 runs the main script
(`AppState::run_target`). Error and debug lines, profiles, the Parameters
panel and *Go to Source* use that generation to reach the right editor,
shown or parked, and completions answered for a tab that is no longer shown
are dropped.

### Talking to the worker

`worker::Worker` owns the child process and three helper threads:

* a **writer** thread that serializes requests to stdin;
* a **reader** thread that parses stdout lines into `Message`s and decodes
  the base64 mesh arrays, so the UI thread never parses large payloads;
* a **stderr** thread that forwards lines to the Log panel.

Events reach the UI through a channel and wake egui up with
`request_repaint`, which runs `App::logic` even when nothing is drawn. Each process gets a *generation* number, so late events
of a killed process are ignored. Requests that expect an answer get an id,
which `Execution` (`app/execution.rs`) maps to what the answer is for (run,
completion, inspection, export, …). The same state machine decides whether a
render starts, waits for Python, is queued behind the running one or belongs
to a document that was replaced since; `tests/rust/app/test_execution.rs`
covers its transitions without a window or a process.

### Rendering

`viewer/renderer.rs` draws the scene off screen into an sRGB texture with 4×
MSAA when available. egui then shows it like an image, through a non-sRGB
view of the same texture so colors are not encoded twice. Devices that
cannot view a texture in another format (wgpu's OpenGL backend, used where
Vulkan is missing and by Mesa's software llvmpipe) get a plain copy instead,
which a small pass (`present.wgsl`) fills after each frame by encoding the
colors by hand; offscreen targets (captures, stereo eyes) need neither.
Multisampling is used only when both the color and the depth-stencil format
support it. Draw order:
background (flat or gradient), opaque faces, edges and points, transparent
faces back to front, then the inspector overlay on top. The same code renders
into textures of its own for screenshots (`render_pixels`), which also lets
the tests draw on any adapter without a window. A capture larger than
2048 pixels on a side is drawn in tiles: each pass scales and shifts clip
space after the projection (`Tile::projection`), so both projections work
and screen-space widths stay in image pixels, and the camera uniform's
`frame` places the tile in the whole image for the background gradient. A
transparent capture draws over transparent black; blending then leaves
premultiplied colors, which `viewer/capture.rs` turns into the straight
alpha of PNG (in linear space, since the colors are sRGB). Turntable frames
are written one per frame of the view, with the `png` crate's animated PNG
encoder.

A **section plane** (`viewer/section.rs`) is a uniform: the fragment shaders
discard what lies on the removed side of it for objects marked `clipped`. The
cut is filled per object with the stencil buffer: every remaining surface of
the object inverts the stencil, without depth test, so the count is odd
exactly where the plane passes through material; a quad on the plane is then
drawn where the stencil is set, and resets it (pass and depth-fail) for the
next object. Picking applies the same plane to the ray (`ClipPlane::kept_range`)
and ignores edges and vertices it removes.

`shaders.wgsl` lights faces with a camera-attached three-point setup (key,
fill, rim) and lights both sides of a face, so open shells look right. The
strengths, the key light's direction, the highlights, exposure and contrast
come from `settings::Lighting` (*Preferences > Rendering*) through the
camera uniform; `lighting_uniform` bounds them, so a damaged settings file
cannot produce negative light or NaN.

Materials (`settings::Finish`) reach the shader as the item's `surface`:
the plain model of `lit`, or `lit_material`, a microfacet model (GGX,
Smith, Schlick) driven by metalness and roughness under the same lights,
which also reflects `studio`, an analytic photo studio attached to the
camera, so metals look like metals without an environment map. Wood adds
value noise in the part's own coordinates (`MeshOut::local`), so the grain
moves with the part.

The Presentation mode's **ground shadow** (`viewer/shadow.rs`,
`shadow.wgsl`) does not depend on the camera, so it is drawn only when the
shadowed items or the settings change (`shadow_key`), and is the same in
every tile of a capture and for both stereo eyes. The opaque faces are drawn
from straight above, with the scene's own object uniforms, into a 256²
two-channel texture that keeps, by max blending, how close to the ground the
lowest face over each texel is. Two separable Gaussian passes, read texel by
texel, blur it a little in the red channel (the contact shadow) and much in
the green one (the ambient shadow); both spreads are fractions of the
model's width, so the square of ground under the model is as many times as
wide as the model. In the scene pass, after the opaque faces, a quad on the
ground darkens the background by both, the ambient one through a curve with
a long tail; the depth test keeps it behind the model and back-face culling
hides it from below.

Which edges are drawn is decided on the GPU, so changing the edge style
uploads nothing. Every segment carries the angle at which the faces of its
B-rep edge meet, measured by the worker (`edge_angles` in
`tessellation.py`), and `vs_edge` collapses the segments below the item's
`edge_filter`. The outline of curved surfaces comes from the triangles:
`Mesh::silhouette_edges` lists, once, the interior mesh edges between two
triangles that do not lie in one plane, with both triangles' normals
(corners at the same place are welded, since the worker meshes each face
apart). `vs_silhouette` draws one where one triangle faces the camera and
the other does not; the buffer is made the first time an item needs it.
Edges and points are camera-facing quads expanded in screen space, which
gives constant pixel widths at any zoom. Faces get a depth bias so edges
stay visible on top of them.

`viewer/culling.rs` decides what is drawn of a large model. Every pass skips
items whose bounds are outside its view frustum (per tile in a tiled
capture), widened by the widest edge or point, which are expanded on
screen. In large-model mode `Viewer::draw_items` swaps the mesh of a part
that is small on screen for `Mesh::simplified`, a vertex-clustered copy made
once per mesh, and drops the edges of specks; picking keeps the full mesh.

Stereo projections (`viewer/stereo.rs`) draw the scene once per eye
(`Camera::eye_view_projection`: the eye moved sideways, its clip space
shifted so both eyes agree on the target's plane) into textures of their
own, then `stereo.wgsl` composes them into the view's texture, reading
texels without filtering. Captures draw each eye with `render_pixels` and
compose on the CPU with the same rules; a test compares the two.

Display modes are styles, not separate renderers (`DisplayMode::style`):
X-ray lowers the face opacity, so faces take the transparent path and
write no depth; hidden-line fills faces with the background color
computed per pixel (gradient included), and draws edges a second time
where the depth test fails (`Greater`), dashed by their screen-space
length.

Assembly parts are **instances**: one mesh plus a transform per part.
Identical geometry is uploaded to the GPU once.

The exploded view and isolation belong to the `Scene`, not to the viewer:
`Scene::part_transform` adds the explosion offset to a part's placement and
`Scene::visible_nodes` leaves out what an isolation hides, so drawing,
picking, framing and the section plane all see the same parts in the same
places. Both changes bump the pick revision, which rebuilds the picking
index. Placements themselves never change, so measurements and exports,
which the worker computes from the real shapes, are unaffected.

### Profiling

`worker/cq_worker/profiling.py` times a run with `sys.settrace`, which also
works on supported Python versions: the global trace function follows the
frames of the editor buffer line by line, adding the time between two events
of a frame to the line that was running, so a line's time includes its callees.
Calls from the script straight into CadQuery's files are timed per function; their own
lines are not traced (`f_trace_lines = False`), and other code is not traced
at all, which keeps the overhead to one Python call per function call. The
GUI maps the profile from the text that ran onto the edited document, and
`CodeEditor::sync_markers` moves it with later edits like the breakpoints.

### Picking

`viewer/picking.rs` hit-tests faces exactly with a ray, accelerated by the
BVH in `spatial.rs`. Edges and vertices are matched in screen space within 6
pixels, and a face in front of them occludes them. The worker sends, with
every mesh, which triangle belongs to which OCCT face and which segment to
which edge (`MeshTopology`). A click can therefore name *Face 6* rather than
"triangle 1234".

Picking never scans the whole scene. It runs in two phases:

* **Broad phase:** the viewer keeps a `SceneIndex`, a BVH over the
  world-space bounds of every visible part. A part is kept when the pointer
  ray hits its box or, when edges or vertices may be picked, when its
  projected box lies within the pixel tolerance of the pointer. The index is
  rebuilt only when `Scene::pick_revision` changes (parts added, removed or
  shown/hidden), never on camera moves or selection changes.
* **Narrow phase:** each remaining mesh tests its triangles, edge segments
  and vertices through its own lazily built BVHs, shared by every instance
  of the mesh. Edge and vertex boxes are pruned in screen space.

Both phases return candidates in scene and index order, so ties between
coincident entities resolve exactly as a linear scan would. The camera
matrices are computed once per pick (`camera::project_with`).
`pick_latency_on_growing_assemblies` measures hover latency as the part
count grows (see [Testing](testing.md#benchmarks)).

GPU picking (rendering entity ids and reading a pixel back) was considered
and not adopted for now: the readback adds at least a frame of latency and
a GPU-to-CPU synchronization, while the CPU path above stays near 0.1 ms per
hover in the benchmark's assembly of 16,384 parts. Revisit it only if a
measured scene makes the CPU path the bottleneck again.

## Python side (`worker/cq_worker/`)

| Module | Responsibility |
| --- | --- |
| `__main__.py` | Entry point: takes over stdout, imports CadQuery, reports `ready` or `fatal` |
| `protocol.py` | JSON-lines transport. Captures `print` output and native writes to descriptor 1 |
| `environment.py` | Reproducible Python, CadQuery, OCP, platform and interpreter version report |
| `server.py` | Request dispatch, the run loop, debug pause loop, session state |
| `execution.py` | Running the editor buffer as `__cq_main__`: injected API, environment, module reloading, tracebacks |
| `debugger.py` | Line stepping with `sys.settrace`, tracing only the script's own frames |
| `scene.py` | Shown objects → scene nodes: colors, assemblies, the `Registry` of live objects by key |
| `tessellation.py` | Meshing, the content-addressed `GeometryCache`, `TopologyIndex` |
| `inspector.py` | Workplane stack description and per-step geometry |
| `measurement_code.py` | *Copy as code*: Python that repeats a measurement with the script's variables and selectors, run on a copy of the console namespace to verify it |
| `provenance.py` | While a script runs, records on each new Workplane, Shape, Assembly and Sketch the script line that created it, for *Go to Source* |
| `selection.py` | Selectors for picked entities: simple selectors evaluated with CadQuery's rules on cached entity features, combined as bit masks, then verified with CadQuery itself |
| `console.py` | The persistent interactive console |
| `completion.py` | jedi completions, call signatures and the documentation shown on hover |
| `exporting.py` | STEP / STL / BREP export |
| `network.py` | Model upload and CadQuery release checks, standard library only |

The worker is **single threaded**: it handles one request at a time. While a
debugging session is paused, `Server.wait_for_debug_action` keeps serving
read-only requests (completion, inspection, export, …), so the GUI stays
useful at a breakpoint.

### The registry

The GUI never holds CadQuery objects. It holds **keys** (`s1-n1`, `s1-n2`,
…). The worker's `Registry` maps each key to the live object, its
world-space shape and its topology index. Export, inspection, measurement and upload
refer to objects by key. When the GUI drops objects it sends `release`, so
the worker can free them.

Keys carry the worker's session number (`CQ_EDITOR_SESSION`, the GUI's
worker generation). Objects stay on screen when Python is restarted, and the
next run releases their keys. With per-session keys, that release can never
free the new worker's objects.

### Tessellation and the geometry cache

1. BRepMesh triangulates the faces in parallel.
2. OCCT's glTF writer (`RWGltf_CafWriter`) emits positions, exact normals and
   indices from C++ in one pass. numpy reads the binary back, which is much
   faster than walking `Poly_Triangulation` from Python.
3. The writer emits each face's triangles in explorer order. Per-face counts
   propose the OCCT face id of each display triangle (`triangle_faces`), then
   each group is checked against the independent OCCT triangulation bounds
   and sampled centroids. Picking and face highlights use the display
   triangles only after that validation; a count or geometry mismatch
   extracts separate pick triangles instead.
4. Edges are discretized with `GCPnts_TangentialDeflection`. Free vertices
   are sent as points.
5. Each mesh gets a **content id**: a BLAKE2b hash of the shape's binary BREP,
   the tolerances and the mesh generator (`mesh_generator()`: the
   `MESH_FORMAT` of `tessellation.py`, the OCP version and the installed OCP
   binary). The GUI keeps meshes across Python restarts, so after a switch to
   another environment or an OCP upgrade the ids change and no mesh of the
   previous mesher is reused.

`GeometryCache` is an LRU of 256 MB keyed by that id. On top of it, the GUI
sends the ids of meshes it already has (`known_geometry`), and the worker
answers with a bare reference instead of the arrays. Re-running an unchanged
model, or showing the same part many times in an assembly, sends each mesh
only once: after editing one part of an assembly, only that part is meshed,
sent and uploaded again.

On the GUI side, `MeshLibrary` (`scene.rs`) holds the meshes by content id.
It pins the meshes a running request may reference and retains up to
128 MB of recently used ones beyond the shown scene. A reference, or a full
mesh for geometry it still holds, resolves to the same `Arc<Mesh>`, and the
renderer keeps GPU buffers per mesh for as long as it lives, so a render
after Clear, a debugging session and repeated runs reuse both the CPU mesh
and its GPU buffers.

## Persistence

| Data | Where | How |
| --- | --- | --- |
| Preferences, layout, window, open tabs, project folders | `app.ron` | eframe persistence (`App::save`) |
| Scripts | wherever you save them | `document.rs`: temp file + atomic replace (`ReplaceFileW`/`MoveFileExW` on Windows, `rename` elsewhere), synchronized to disk |
| Recovery session | `recovery/active-session.json` | Every tab with unsaved changes, in tab order; written every 5 s while one is modified. The single snapshot of earlier releases is still read |
| Local history | `recovery/history/<document>/version-<timestamp>-<n>.json` | One folder per document (FNV-1a hash of its path, `untitled` for buffers without a file); file names carry the timestamps, so a checkpoint lists one small folder and reads no JSON. 20 newest versions per document; once per session, versions older than 90 days and the oldest beyond 64 MB in total are removed |
| Log | `logs/cq-editor.log`, `logs/cq-editor.1.log` | `logging.rs` appends every Log panel line and GUI panics; rotated at 2 MB |

## Design principles

* **The script is the source of truth.** The GUI shows and explains what
  the code produced. Changes made in the GUI (colors, visibility) never
  rewrite the script.
* **Compatibility.** Scripts written for the original PyQt CQ-editor run
  unchanged: same `show_object` options, same namespace, same execution
  settings. User-interface equivalence and intentional differences are
  recorded separately in the [compatibility matrix](compatibility.md).
* **The UI thread never blocks** on Python, on the disk for long, or on the
  network.
* **No silent data loss.** Atomic saves, recovery checkpoints, conflict
  dialogs.
* **One code path per command** (`Action`), shared by menus, toolbar and
  shortcuts.
