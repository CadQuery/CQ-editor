# Worker protocol (version 12)

The GUI and the Python worker exchange **one JSON object per line**, encoded
as UTF-8:

* **requests** go from the GUI to the worker's **stdin** and carry a `cmd`
  field;
* **messages** go from the worker to the GUI on a private duplicate of the
  original **stdout** and carry a `type` field;
* the worker's **stderr** is free text, shown in the Log panel line by line.

Definitions: `src/worker/protocol.rs` (Rust, authoritative for the GUI) and
`worker/cq_worker/server.py` (Python).

## Process start-up

The GUI runs:

```
<python> -u -m cq_worker     with PYTHONPATH=<worker dir>[:<existing PYTHONPATH>]
                                  PYTHONIOENCODING=utf-8
                                  CQ_EDITOR_SESSION=<worker generation: 1, 2, …>
```

`CQ_EDITOR_SESSION` numbers the GUI's successive worker processes. The worker
uses it to namespace its scene keys (see [Scene nodes](#scene-nodes)).

The worker then:

1. duplicates descriptor 1 for the protocol, and points descriptor 1 at
   stderr. C++ code printing to stdout (OpenCascade) can then never corrupt
   the message stream;
2. replaces `sys.stdout`/`sys.stderr` with streams that send `output`
   messages, and `sys.stdin` with an empty stream (scripts must not consume
   requests);
3. removes a stale `CASROOT` variable and imports CadQuery;
4. sends **`ready`**, or **`fatal`** if the import failed.

```json
{"type":"ready","protocol":12,"python":"3.12.10","cadquery":"2.8.0","ocp":"7.9.3.1.1","executable":"C:\\…\\python.exe","completion":true,"pid":1234}
{"type":"fatal","message":"Cannot load CadQuery: ModuleNotFoundError: …","detail":"Traceback …","executable":"…"}
```

The GUI refuses a worker whose `protocol` differs from its own
`PROTOCOL_VERSION`. **Bump the version on both sides** whenever a change is
not backward compatible.

## Ids and replies

Most requests carry an `id` chosen by the GUI. The reply carries the same
`id`. `output` and `log` messages produced while a request is handled are
tagged with that request's id. This is how console output ends up in the
console and script output in the Log panel.

A failing request is answered with:

```json
{"type":"error","id":7,"message":"KeyError: \"Unknown object 's1-n3'; it may have been cleared or come from a previous Python session (render again)\""}
```

The worker handles one request at a time, in arrival order.

## Requests

| `cmd` | Fields | Reply |
| --- | --- | --- |
| `run` | `id`, `script`, `path` (or null), `debug`, `breakpoints` (main script lines), `project_root`, `module_breakpoints` (path to line numbers), `settings` (RunSettings), `scene` (SceneSettings), `known_geometry` (mesh ids), `profile` (time each line; ignored with `debug`) | `run_finished`, preceded by `debug_paused` messages when debugging |
| `debug` | `action` (`continue`, `step`, `step_in`, `step_out`, `stop`), `breakpoints`, `module_breakpoints` | Only while paused. Refreshes breakpoints and resumes the session |
| `release` | `keys` | None. Frees registry entries |
| `complete` | `id`, `source`, `line` (1-based), `column` (0-based, characters), `path`, `target` (`editor` or `console`) | `completions` |
| `describe` | `id`, `source`, `line` (1-based), `column` (0-based, characters), `path` | `description` |
| `inspect` | `id`, `key` | `inspection` |
| `inspect_geometry` | `id`, `key`, `frame`, `item` (or null for the whole frame), `scene` | `inspection_geometry` |
| `measure` | `id`, up to 64 `selections` (`key`, zero-based `part`, `kind`, one-based `identifier`), and `objects` (keys) measured when `selections` is empty | `measurement` |
| `measurement_code` | `id`, `selections`, `objects` as for `measure` | `measurement_code` |
| `selector` | `id`, `selections` of one object, part and kind | `selector` |
| `selector_preview` | `id`, `run_id`, 1-based `line` and character `column`, optional `preferred_key` | `selector_preview` |
| `console` | `id`, `source`, `scene`, `known_geometry`, `selection` (`entities`: picked topology references, `objects`: selected keys) for `selected()` | `display` for each result, `scene_add` (if objects were shown), then `console_result` |
| `console_reset` | `id` | `console_result` |
| `export` | `id`, `keys`, `format` (see Export dialog), `path`, tolerances, STEP mode/schema, STL ASCII, `flat_parts`, `unit`, `on_invalid` (`check`, `fix`, `ignore`), `on_interference` (`check`, `ignore`) | `export_finished` or `geometry_invalid` |
| `check` | `id`, `keys`, `flat_parts`, optional `interference_tolerance`, `clearance` (model units) | `geometry_checked` |
| `interference_geometry` | `id`, `first`, `second` (live pair references from a geometry report), `scene` | `interference_geometry` with exact shared-volume mesh |
| `upload` | `id`, `url`, `file_type` (`step`, `stl`, `py`), `keys`, `source` | `upload_finished` |
| `check_updates` | `id`, `editor` (the GUI's version: list CQ-editor's newer releases), `cadquery` (also list CadQuery's; true when missing) | `updates` |
| `imports` | `id`, `path` | `imports` (local module files imported by the script) |
| `cancel` | `target` (the id of a `run`) | None. Read even while the worker is busy; meshing stops at the next part and the run finishes with `cancelled` |
| `interrupt` | `target` (the id of a `run` or `console` request) | None. Read even while the worker is busy; raises `KeyboardInterrupt` in that request, which then finishes as usual (`interrupted` for a run, a traceback in the console's output) |
| `shutdown` | | The worker exits |

**RunSettings:** `reload_cq`, `add_script_dir_to_path`, `change_working_dir`,
`reload_imported_modules` (booleans, see [Preferences](../user-guide/preferences.md#execution)).

**SceneSettings:** `linear_tolerance`, `angular_tolerance` (display mesh),
`merge_assemblies`, `units`.

## Messages

| `type` | Fields | Meaning |
| --- | --- | --- |
| `ready` | `protocol`, `python`, `cadquery`, `ocp`, `executable`, `completion`, `pid` | Worker is ready; the version fields identify the exact CAD runtime |
| `fatal` | `message`, `detail`, `executable` | Worker cannot start |
| `output` | `id`, `stream` (`stdout`/`stderr`), `text` | Captured `print` / `sys.stderr` text |
| `log` | `id`, `level`, `source`, `message` | A Python logging record (including the script's `log()`) |
| `run_part` | `id`, `name`, `part` | A complete mesh part, sent before the final scene. The GUI draws received parts progressively and retains their meshes for this request. A single part exceeding the wire limit fails with an error naming the object. |
| `run_finished` | `id`, `ok`, `debug`, `stopped`, `cancelled`, `interrupted`, `objects`, `variables`, `error`, `checks`, `duration`, `timings`, `profile` | End of a run. Meshes in `objects` refer by id to preceding `run_part` messages. `objects` is null when a failed, cancelled or interrupted run should keep the current scene; an interrupted run's `error` is the `KeyboardInterrupt` and its line. `checks` contains script requirements. `timings` is described below. `profile`, for a run asked to profile (null otherwise): `lines` (`file` null for main, `line`, `seconds` with everything the line called, `hits`), `operations` (`file`, `line`, `name`, `seconds`, `calls`), at most 8 per line, and `sources` for project modules |
| `progress` | `id`, `stage` (`meshing`), `done`, `total` | How many of the run's parts (one per shape of an assembly) are meshed: first `0`, at most every 0.1 s, and `total` at the end |
| `debug_paused` | `id`, `line`, `path` (null for the main buffer), `variables`, `objects` | The debugger stopped in this document at `line`. `objects` are the CadQuery locals |
| `completions` | `id`, `available`, `items` (`name`, `insert`, `kind`), `signatures` | |
| `description` | `id`, `available`, `signatures` (call signatures, or `name: type` for a variable), `text` (the docstring, cut to 14 lines / 1200 characters); both empty when there is nothing to show | |
| `inspection` | `id`, `key`, `frames` (`type`, `value`, `line`, `items` (`type`, `value`, `displayable`, `line`)) | Workplane stack, newest first; `line` is the script line that made each step and object |
| `inspection_geometry` | `id`, `meshes`, `plane` (`origin`, `x_dir`, `z_dir`, `size`) | Overlay geometry |
| `measurement` | `id`, `identity`, `entities`, `pair`, `objects`, `bounding_box` | Exact properties of each entity, each with its `bounding_box` (`min`, `max`, `size`, `center`); `pair` (the relationship) only for exactly two. Without entities, `objects` describes the selected objects (`shape_type`, `volume`, `area`, `length`, center of mass, `bounding_box`, topology counts). `bounding_box` surrounds everything measured; boxes come from the exact geometry, not the mesh |
| `measurement_code` | `id`, `code`, `verified`, `problem` | Python repeating the measurement with the script's variables; `verified` when running it on a copy of the console namespace reproduced every value |
| `selector` | `id`, `method` (`faces`, `edges` or `vertices`), `candidates` | CadQuery selectors for the entities, best first. Each candidate has `expression` (to append to the object, e.g. `.faces(">Z")`), `selector` (`">Z"`, null for an index), `description` and `index_based`. Every candidate was verified with CadQuery on the object the script holds |
| `selector_preview` | `id`, `active`, `count`, `matches`, `message` | Exact topology references for a literal selector on a displayed variable from `run_id`; at most 64 highlights. Dynamic expressions are never evaluated for a preview |
| `scene_add` | `id`, `objects` | Objects shown from the console |
| `display` | `id`, `text`, `image` (null, or `format`: `png` or `jpeg`, `data`: base64) | A console result: the value of an expression, or each argument of `display()`. See [Rich console results](#rich-console-results) |
| `console_result` | `id`, `more` | `more` is true when the statement is incomplete |
| `export_finished` | `id`, `path` | |
| `geometry_checked` | `id`, `reports`, `interferences`, `assessment` | Topology per object and exact solid-pair results (`interference`, `contact`, `clearance`, `inconclusive`); declared pairs include `allowance_reason`, and assessment includes the accepted count and status (`clear`, `interference`, or `inconclusive`) |
| `interference_geometry` | `id`, `mesh` | Mesh of the exact positive shared volume for the selected pair |
| `geometry_invalid` | `id`, `reports`, `interferences` | Export wrote nothing and awaits the user's decision about topology or part interference |
| `upload_finished` | `id`, `status`, `body` | HTTP status and response body |
| `updates` | `id`, `current`, `newer`, `editor` | CadQuery release tags newer than `current` (empty when not asked); with `editor`, CQ-editor's newer releases (`current`, `newer`: `tag`, `name`, `url`, newest first, drafts and pre-releases left out) |
| `imports` | `id`, `paths` | |
| `error` | `id`, `message` | A request failed |

`error` (in `run_finished`) is a ScriptError: `type_name`, `message`,
`frames` (`file`, `line`, `function`, `code`), `line` (the last line of the
script involved), `text` (Python's formatted traceback), and `diagnostic`
(null or `operation`, `receiver`, `line`, `hints`). The editor
buffer appears as the file `<cq-editor>`.

## Scene nodes

```json
{
  "key": "s1-n4", "name": "bracket", "kind": "Workplane", "inspectable": true, "line": 12,
  "color": [0.27, 0.51, 0.71], "alpha": null, "edge_color": null, "finish": "matte_plastic",
  "parts": [{ "mesh": { … }, "transform": [16 floats, column-major], "color": null, "alpha": null }],
  "children": []
}
```

* `key` identifies the object in the worker's registry. It is used for
  `export`, `inspect`, `upload` and `release`. Keys are opaque to the GUI.
  They have the form `s<session>-n<counter>`, or `n<counter>` when
  `CQ_EDITOR_SESSION` is not set. A restarted worker therefore never reuses
  the keys of objects that are still on screen, and releasing such a stale
  key is a no-op.
* An assembly is a node whose `children` mirror the assembly tree. Each
  child carries its own parts and placement.
* A part may carry `face_attributes` (`id`, `name`, `layer`, `color`, `alpha`)
  indexed by its live OCCT face ids. These belong to the part instance, not
  the content-addressed mesh. `material` carries CadQuery's name, density and
  density unit; measurements report mass in grams when the density unit is
  supported.
* Colors are sRGB floats in 0–1. `null` means "use the default".
* `finish` is the `material` option of `show_object`: `matte_plastic`,
  `glossy_plastic`, `brushed_steel`, `aluminum`, `wood`, `default`, or
  `null` for the plain shading. The worker refuses unknown names with a
  warning; the GUI treats a name it does not know as `null`.
* `line` is the 1-based script line that created the object (for an
  assembly child, the line of its `add` call), or the line of the
  `show_object` call when the object was not created by this run; `null`
  for objects shown from the console. While a script runs, the worker
  records the innermost script frame of every new `Workplane`, `Shape`,
  `Assembly` and `Sketch` (`provenance.py`). Lines refer to the script text
  of the `run` request; the GUI maps them onto the edited text.

### Meshes

A full mesh:

| Field | Encoding |
| --- | --- |
| `id` | Content id: BLAKE2b of the shape's binary BREP, the tolerances and the mesh generator (`MESH_FORMAT`, the OCP version and its installed binary), so an id never names a mesh made by another environment's mesher |
| `positions`, `normals` | base64 of little-endian `f32` triples |
| `indices` | base64 of little-endian `u32`, three per triangle |
| `edges` | base64 `f32` triples, two points per segment |
| `segment_angles` | base64 `f32`, one per segment: degrees between the faces that meet at its edge, at the edge's middle (0 smooth, 90 a box corner, 180 an edge of one face or a wire). The GUI's Sharp edges and Outline styles leave out the edges below an angle. Absent in older meshes, which then count as all 180 |
| `points` | base64 `f32` triples (free vertices) |
| `bbox` | `[[min x, y, z], [max x, y, z]]` |
| `topology` | `triangle_faces` (OCCT face id per pick triangle), `segment_edges` (OCCT edge id per segment), `vertices`, and `face_positions` / `face_indices`: separate pick triangles, normally empty so that `triangle_faces` follows the display `indices` |

A **reference** is just `{"id": "…"}`. The worker sends it instead of the
arrays when the id is in the request's `known_geometry` or was already sent
earlier for the same request, which includes the previous `debug_paused`
messages of a debugging session. The GUI resolves it from its mesh library
and therefore pins every advertised and received mesh until the request
ends (`run_finished`, `console_result` or a worker restart), even when a
paused view no longer shows it. `scene_add` carries its request `id` for the
same reason.

`known_geometry` lists every mesh the GUI still holds: the shown ones and
up to 128 MB of recently used ones, so a render after Clear or a debugging
session can also receive references.

The line transport has explicit memory ceilings. Ordinary control messages
are limited to 2 MiB; the four replies that may carry mesh arrays
(`run_finished`, `debug_paused`, `inspection_geometry` and `scene_add`) have
a separate 512 MiB limit. The reader drains an oversized line without
retaining more than its limit and reports it as `Event::ProtocolError`,
which is not a failed start: nothing later from that process is read, so
the GUI ends the waiting run, says what happened (for a mesh reply, that the
model is too large and how to coarsen it) and starts a new worker. Native
stderr lines retain at most 256 KiB and are marked when truncated.

Captured `stdout` and `stderr` are flushed in chunks of 64 Ki characters,
even without a newline, and keep at most 1 MiB per stream and request. A
single Python logging record keeps at most 256 Ki characters. Both paths add
an explicit truncation marker rather than silently consuming unbounded
memory; repeated logging records share a 1 MiB per-request ceiling.

Binary arrays are decoded on the GUI's reader thread, so large models never
stall the interface. Base64 inside JSON is kept on purpose: for a message
with 5 MB of mesh arrays, encoding and serializing take about 35 ms in the
worker and parsing about 6 ms in the GUI, against more than a second of
meshing. Unchanged geometry is not sent at all (see references above).

`timings` in `run_finished` reports where the time went, in seconds for the
whole request (a debugging session included): `script` (equal to
`duration`), `identify` (content ids), `tessellate` (meshing of geometry not
sent as a reference), `encode` (base64), the mesh counts `meshed`, `cached`
(served from the worker's cache) and `referenced`, `mesh_bytes` (raw arrays
sent in full) and `sent_at` (worker wall clock, seconds since the Unix
epoch). The GUI adds the transfer time, its own parsing and scene decoding,
and writes one *Run timings* line to the Log panel; GPU uploads are logged
at the debug level.

### Topology identity and measurements

The topology arrays associate display primitives with the 1-based OCCT face,
edge and vertex order of one live registry entry. A `measure` request resolves
those references with `RegistryEntry.subshape`; it never measures the display
mesh. Registry shapes are already placed in world coordinates, including
assembly locations.

`measurement.identity` is always
`{"scope":"scene_entry","persistent":false}`. This is an explicit contract:
an id remains usable only while its scene key is alive. OCCT subshape order is
not a persistent naming scheme across rebuilds or remodeling.

Each entity returns `geometry_type`, `center` and applicable `position`,
`length`, `area`, `radius` and `diameter` values as JSON doubles. A two-entity
response adds exact minimum `distance`, the two closest points and, when the
relationship is well-defined, `angle_degrees`/`angle_kind`. Angles currently
cover straight edge/edge, planar face/face and straight edge/planar face.

## Debugging sessions

```
GUI                                   worker
 │  run {debug: true, breakpoints}      │
 │ ───────────────────────────────────► │ runs under sys.settrace
 │            debug_paused {line: 9}    │ pauses, waits for a command
 │ ◄─────────────────────────────────── │
 │  complete / inspect / measure / export │ served while paused
 │ ───────────────────────────────────► │
 │  debug {action: "step"}              │
 │ ───────────────────────────────────► │ resumes
 │            debug_paused {line: 10}   │
 │ ◄─────────────────────────────────── │
 │  debug {action: "continue"}          │
 │ ───────────────────────────────────► │
 │            run_finished              │
 │ ◄─────────────────────────────────── │
```

While paused, only `complete`, `inspect`, `inspect_geometry`, `measure`, `selector`, `export`, `check`,
`imports`, `release` and `check_updates` are served. Anything else is
answered with an `error` (*Not available while debugging; continue or stop
first*).

## Rich console results

While a `console` request runs, `sys.displayhook` and the console's
`display()` gather each value's representations as Jupyter does
(`_repr_mimebundle_`, `_repr_png_`, `_repr_jpeg_`, and matplotlib figures
through `savefig`), in `worker/cq_worker/display.py`. A representation that
raises is skipped; the value still gets its `repr`.

Only what the console shows leaves the worker: the plain text and one image.
HTML, Markdown, SVG, LaTeX and JSON are never sent, so no markup or script
from user code reaches the GUI. The worker bounds each message: the text to
64K characters, the image to 4 MB, recognized by its PNG or JPEG signature,
at most 20 images and 100 rich results per command; what is left out is
described in the text. Oversized base64 is rejected before decoding. Standard
output and error are flushed before each `display`, so the console keeps the
order in which things happened. A dedicated 8 MB wire ceiling accepts a
maximum-size rich result without granting it the 512 MB mesh-message limit.

The GUI does not trust the image either. The reader thread decodes it, never
the thread that draws: only `png` and `jpeg`, at most 4096 pixels on either
side and 128 MB of memory; an image that fails is replaced by *[image not
shown: …]* in the text. The console keeps its newest 50 images; older ones
leave an *[image]* note, and decoded pixels also share a 128 MB retention
budget. Results are accepted only while their console request is still
pending. A slow `_repr_*_` only takes the worker's time, and Stop interrupts
it like any other code.

## Stopping

**Cancelling** only stops meshing: before each part the worker reads, without
waiting, whatever the GUI has sent (`PeekNamedPipe` on Windows, `select`
elsewhere), records `cancel` requests and keeps the other requests, in order,
for later. A thread blocked reading stdin is not used: on Windows, loading a
DLL (an `import` of a native module) would wait for that read. The parts
meshed so far are dropped and their registry entries released.

**Stop** interrupts first. While a `run` or `console` request is handled, a
watcher thread polls the same way every 50 ms; on an `interrupt` for that
request it sends SIGINT to the main thread (`signal.raise_signal` on
Windows, `pthread_kill` elsewhere), so Python raises `KeyboardInterrupt`
there and waits such as `time.sleep` wake up. `_thread.interrupt_main` is
not enough on Windows: it does not wake those waits. The run reports
`interrupted`, the console prints the traceback, and the process keeps its
console namespace and caches. An interrupt that lands after its request
ended is absorbed, so it never reaches the next one, and the worker sets
the default SIGINT handler at start-up, since an inherited "ignore" would
disable all this.

Python cannot interrupt C++ code: an interrupt is only raised when it
returns to Python. If the request has not answered 3 seconds after the
interrupt, or Stop is pressed again, the GUI kills the worker process and
starts a new one. Events from the old process are discarded by their
generation number.
