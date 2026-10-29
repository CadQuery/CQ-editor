# Compatibility with the original CQ-editor

This document is the compatibility contract for the Rust rewrite. It prevents
"implemented" from being used as a synonym for "equivalent" and makes every
known removal or changed behavior explicit.

## Baseline and rules

The baseline is commit [`71903d5`](https://github.com/igors93/CQ-editor/tree/71903d5),
the last PyQt/OCCT revision immediately before the Rust rewrite. The primary
legacy surfaces are
[`viewer.py`](https://github.com/igors93/CQ-editor/blob/71903d5/cq_editor/widgets/viewer.py),
[`occt_widget.py`](https://github.com/igors93/CQ-editor/blob/71903d5/cq_editor/widgets/occt_widget.py),
[`editor.py`](https://github.com/igors93/CQ-editor/blob/71903d5/cq_editor/widgets/editor.py)
and
[`main_window.py`](https://github.com/igors93/CQ-editor/blob/71903d5/cq_editor/main_window.py).
The eight scripts `01` to `08` in `examples/` are byte-for-byte unchanged from
that commit and are executable compatibility fixtures; later examples are
additions.

Compatibility means the same user-visible result or script contract. It does
not require the same toolkit, process model or rendering implementation. A
legacy defect is not preserved merely for fidelity; a corrected behavior is
marked **Enhanced** and protected by a regression test.

The states below have precise meanings:

* **Verified**: an automated test exercises the compatible result.
* **Enhanced**: the original result remains available and a tested superset or
  deliberate defect correction is provided.
* **Changed**: a deliberate, documented product decision replaces the old
  behavior.
* **Gap**: the original capability is not currently delivered.
* **Unverified**: it is implemented, but no automated test proves the complete
  user interaction yet. It must not be described as equivalent.

## Compatibility matrix

The first column is a stable contract id. `compatibility_matrix_is_complete_and_evidenced`
checks that no row disappears, every Verified/Enhanced row points to a real
test, and every Changed/Gap/Unverified row points to an explicit decision or
follow-up. A passing matrix test therefore proves the inventory is maintained;
it does not turn the open gap into a supported feature.

| Contract id | Original capability and current result | State | Evidence or decision |
| --- | --- | --- | --- |
| `file.lifecycle` | New, Open, Save and Save As preserve the editable Python file workflow. Saves are now atomic, and files open in tabs, so opening one no longer replaces the current file. | Enhanced | `tests/rust/editor/test_document.rs::preserves_line_endings_and_tracks_modification` |
| `file.autoreload` | Automatic reload and imported-file watching remain; local edits now require an explicit conflict decision. | Enhanced | `tests/rust/editor/test_document.rs::reload_refuses_to_discard_unsaved_changes` |
| `file.recovery` | Unsaved buffers gain crash recovery and local history, for every open tab. | Enhanced | `tests/rust/editor/test_recovery.rs::checkpoint_restores_unsaved_work_after_a_simulated_crash` |
| `editor.syntax` | Python syntax highlighting and the injected CQ-editor names remain recognized. | Verified | `tests/rust/editor/test_highlight.rs::recognizes_python_constructs` |
| `editor.editing` | Line numbers, breakpoints, indentation and Toggle Comment remain; markers now follow edits. | Enhanced | `tests/rust/editor/test_text_ops.rs::comments_blocks_at_the_common_indentation` |
| `editor.search` | Find and repeated match navigation remain available. | Verified | `tests/rust/editor/test_editor.rs::finds_matches_with_and_without_case` |
| `editor.completion` | Jedi completion and call signatures remain optional and use the live namespace. | Verified | `tests/worker/execution/test_completion.py::test_completion_uses_live_namespace_values` |
| `script.examples` | All eight projects shipped before the rewrite still execute and produce a visible model. | Verified | `tests/worker/execution/test_examples.py::test_legacy_example_executes_and_produces_a_model` |
| `script.show-object` | `show_object(obj, name, options, **options)` and variable-derived names remain supported. | Verified | `tests/worker/execution/test_execution.py::test_show_object_names_from_variables` |
| `script.discovery` | CadQuery values are still displayed when a script omits `show_object`. | Verified | `tests/worker/geometry/test_scene.py::test_find_cq_objects_ignores_private_and_plain_values` |
| `script.logging` | `print` output and `log()` messages still reach their panels. | Verified | `tests/worker/protocol/test_server.py::test_script_log_reaches_the_gui` |
| `script.execution-settings` | Reload CadQuery, script directory on `sys.path`, script working directory and imported-module reload remain configurable. | Verified | `tests/worker/execution/test_environment.py::test_script_environment_respects_disabled_path_and_cwd_changes` |
| `debugger.controls` | Debug, Step, Step in and Continue remain; Step out and Stop were added. | Enhanced | `tests/worker/execution/test_debugger.py::test_step_in_and_out` |
| `debugger.breakpoints` | Gutter breakpoints remain and now follow inserted or deleted lines. | Enhanced | `tests/rust/editor/test_editor.rs::breakpoints_and_the_error_line_follow_inserted_and_deleted_lines` |
| `debugger.errors` | Tracebacks retain script lines and nested call frames; variables remain described separately. | Verified | `tests/worker/execution/test_execution.py::test_errors_inside_script_functions_keep_the_call_chain` |
| `console.state` | The Python console remains interactive, multiline and persistent across commands. | Verified | `tests/worker/execution/test_console.py::test_console_keeps_variables_between_commands` |
| `console.rich-output` | Expression results and `display()` show images (matplotlib figures, `_repr_png_`, `_repr_jpeg_`) and plain text, checked and bounded; HTML, SVG and LaTeX show their plain text. | Changed | Decision: [Rich console](#rich-console); tested by `tests/worker/execution/test_display.py::test_the_console_sends_results_and_images_after_what_was_printed` |
| `inspector.workplane` | The CQ Inspector still traverses Workplane parents and can display a stack item, plane and grid. | Verified | `tests/worker/protocol/test_server.py::test_inspection` |
| `objects.tree` | Objects and assembly children retain visibility, selection, removal and clearing semantics. | Verified | `tests/rust/scene/test_scene.rs::hiding_a_parent_hides_the_subtree` |
| `objects.properties` | Visibility and colors can be preserved by name; color and opacity editing are a tested superset. | Enhanced | `tests/rust/scene/test_scene.rs::properties_survive_a_reload_by_name` |
| `objects.assemblies` | Assemblies can remain hierarchical or be merged, retaining per-part transforms and colors. | Verified | `tests/worker/geometry/test_scene.py::test_builder_explodes_assemblies` |
| `export.step-stl` | Selected objects still export to STEP and STL; BREP was added. | Enhanced | `tests/worker/protocol/test_server.py::test_export` |
| `tools.upload` | Python or exported model bytes can still be uploaded to the configured service. | Verified | `tests/worker/protocol/test_server_commands.py::test_step_upload_sends_actual_model_bytes` |
| `tools.update` | CadQuery release checks remain available; CQ-editor's own releases are checked too, by hand and once a day. | Enhanced | `tests/worker/protocol/test_server_commands.py::test_update_checks_can_cover_cq_editor_and_skip_cadquery` |
| `viewer.standard-views` | Fit, isometric, top, bottom, front, back, left and right camera results remain. | Verified | `tests/rust/viewer/test_camera.rs::standard_views_look_where_expected` |
| `viewer.display-modes` | Shaded and wireframe keep their commands and shortcuts; X-ray and hidden-line modes and per-object edge visibility were added. Each mode is rendered offscreen and its pixels are checked. | Enhanced | `tests/rust/viewer/test_renderer.rs::display_modes_render_faces_edges_and_fills_as_documented` |
| `viewer.projection` | Orthographic and perspective cameras remain and produce consistent pick rays. | Verified | `tests/rust/viewer/test_camera.rs::projection_round_trips_through_rays` |
| `viewer.mouse` | Left orbit/select, middle or Ctrl+left pan, Shift+left zoom and wheel zoom retain their bindings; right drag now orbits instead of zooming, and an orbit begun on the model turns around the point under the pointer. | Changed | Decision: [Mouse navigation](#mouse-navigation); tested by `tests/rust/viewer/test_navigation.rs::mouse_gestures_choose_their_navigation_modes` |
| `viewer.navigation-cube` | Face/edge/corner views and 45-degree roll remain, with native hover feedback and animation. | Enhanced | `tests/rust/viewer/test_overlays.rs::arrows_are_hit_on_both_sides` |
| `viewer.selection` | Object selection remains and now resolves stable OCCT face, edge and vertex identities. | Enhanced | `tests/rust/viewer/test_picking.rs::prioritizes_vertices_then_edges_then_faces` |
| `viewer.screenshot` | Tools > Screenshot still saves the view as PNG; larger sizes (drawn in tiles), a transparent background and turntable animations were added. Capture, tiling, transparency and PNG writing are tested; only the native save dialog is not. | Enhanced | `tests/rust/viewer/test_renderer.rs::captures_leave_out_a_transparent_background_and_scale_edges` |
| `viewer.stereo` | The Stereo, MonoLeftEye and MonoRightEye projections and the Anaglyph, RowInterlaced, ColumnInterlaced, ChessBoard, SideBySide and OverUnder modes are rendered and tested; QuadBuffer cannot be presented by wgpu and visibly falls back to the anaglyph. | Changed | Decision: [Stereo projection](#stereo-projection) |
| `panels.docking` | All legacy panels are dockable and resizable, but full drag/detach/layout equivalence has no automated UI test. | Unverified | Follow-up: [GUI-only validation](#gui-only-validation) |
| `preferences.persistence` | Supported preferences and the dock layout persist across sessions. | Verified | `tests/rust/app/test_settings.rs::preferences_round_trip_through_json` |
| `preferences.defaults` | Configurable font and tessellation settings remain, but three numeric defaults differ from the PyQt values. | Changed | Decision: [Preference defaults](#preference-defaults) |
| `shortcuts.legacy` | Every legacy file, editor, debugger and view shortcut is retained; additions do not replace them. | Verified | `tests/rust/app/test_commands.rs::legacy_shortcuts_are_preserved` |
| `behavior.rand-color` | `rand_color()` now defaults to visible opacity instead of reproducing the original invisible-color defect. | Enhanced | `tests/worker/execution/test_execution.py::test_rand_color_is_reproducible` |
| `ui.theme` | Separate application/editor schemes became one System/Light/Dark application theme. | Changed | Decision: [Theme settings](#theme-settings) |
| `packaging.distribution` | Native installers again bundle Python and CadQuery; they add platform launchers, install without a pre-existing environment and are smoke-tested after installation. | Enhanced | `tests/packaging/test_release.py::test_installer_contains_a_pinned_cadquery_runtime` |
| `worker.isolation` | Script execution moved out of process; crashes no longer take down the editor and stale scene ids cannot collide after restart. | Enhanced | `tests/worker/protocol/test_worker_process.py::test_worker_can_be_restarted_after_a_native_process_exit` |

## Exact interaction inventory

### Legacy shortcuts

The compatibility test asserts the complete set below. Ctrl means the
platform command modifier where appropriate.

| Area | Action | Original | Current |
| --- | --- | --- | --- |
| File | New / Open / Save / Save As / Quit | Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+Shift+S / Ctrl+Q | Preserved |
| Edit | Toggle Comment / Auto-Complete / Search | Ctrl+/ / Alt+/ / Ctrl+F | Preserved; Ctrl+Space also completes |
| Run | Render / Debug / Step / Step in / Continue | F5 / Ctrl+F5 / Ctrl+F10 / Ctrl+F11 / Ctrl+F12 | Preserved; Step out and Stop added |
| View | Fit / Iso / Top / Bottom / Front / Back / Left / Right | Shift+F1 through Shift+F8 | Preserved |
| View | Wireframe / Shaded | Shift+F9 / Shift+F10 | Preserved; X-Ray and Hidden Line added on Shift+F11 / Shift+F12 |

### Legacy mouse behavior

| Gesture | Original result | Current result |
| --- | --- | --- |
| Left drag | Orbit using Turntable or Trackball | Preserved; a drag begun on the model now holds its picked point on screen |
| Left click without a drag | Select | Preserved; exact topology is also returned |
| Middle drag or Ctrl+left drag | Pan | Preserved |
| Right drag or Shift+left drag | Zoom | Right drag now orbits around the point under the pointer; Shift+left still zooms |
| Wheel | Zoom | Preserved; anchored at the pointer |
| Navigation cube or roll arrow click | Reorient or roll by 45 degrees | Preserved |

### Legacy preferences

| Original group | Original setting | Current mapping |
| --- | --- | --- |
| General | Light/Dark Theme | System/Light/Dark application theme; see the decision below |
| Viewer | Fit automatically, navigation cube, gradient and two background colors | Preserved |
| Viewer | Deviation and angular deviation | Preserved as tessellation tolerances; the defaults changed as documented below |
| Viewer | Projection Type | Orthographic, Perspective, Stereo, MonoLeftEye and MonoRightEye preserved, under the same setting names |
| Viewer | Stereo Mode | Anaglyph, RowInterlaced, ColumnInterlaced, ChessBoard, SideBySide and OverUnder preserved; QuadBuffer falls back to the anaglyph, see the decision below |
| Viewer | Orbit Method | Turntable and Trackball preserved |
| Editor | Font size, autoreload, reload delay, imported-file watch, line wrap and maximum line length | Preserved; the font-size default changed as documented below |
| Editor | Color scheme | Merged into the application theme |
| Debugger | Reload CQ, add script directory, change working directory and reload imported modules | Preserved as Execution settings |
| Objects | Preserve properties, clear before run, merge assemblies and STL precision | Preserved |

Default shape/edge colors, edge visibility/width, axes, animation and STL
angular tolerance are additions rather than legacy settings.

## Known differences and decisions

### Stereo projection

The legacy modes were thin bindings to OCCT's `Graphic3d_Camera` and
`Graphic3d_StereoMode`. The wgpu renderer now draws them itself (`FEAT-C8`):
the two eyes are perspective cameras 5% of the target distance apart (OCCT's
default relative interocular distance), with off-axis frustums that converge
on the target's plane, and a composition pass combines them.

* **Stereo**, **MonoLeftEye** and **MonoRightEye** keep their meaning; mono
  eyes also pick from their eye.
  `tests/rust/viewer/test_camera.rs::the_eyes_of_the_stereo_pair_agree_on_the_target_plane`
  and `mono_eye_projections_draw_and_pick_from_one_eye` check zero parallax
  at the target, crossed disparity in front of it and consistent pick rays.
* **Anaglyph** (OCCT's default optimized red-cyan filter), **RowInterlaced**,
  **ColumnInterlaced**, **ChessBoard**, **SideBySide** and **OverUnder** are
  composed on the GPU for the view and on the CPU for screenshots;
  `tests/rust/viewer/test_stereo.rs` checks each mode's layout, and
  `tests/rust/viewer/test_renderer.rs::stereo_modes_compose_on_the_gpu_as_the_reference_does`
  checks that the GPU composition gives the same pixels as the CPU one
  (exactly, or within one level for the anaglyph's mixing), on the software
  adapter in CI.
* **QuadBuffer** needs a stereo swap chain (one back buffer per eye) and
  stereo glasses synchronized by the driver. wgpu exposes no such surface on
  any backend, and most current drivers dropped it, so this mode is
  **Changed**: it is still offered, under its name, and the view shows the
  red-cyan anaglyph with a note saying so, instead of silently showing
  something else.

Side-by-side and over-under draw each eye at half the width or height
without stretching it (for free viewing and for displays that take that
layout), where OCCT could also stretch them for 3D televisions.

### Mouse navigation

The original editor zoomed with a right drag. Right drag now orbits like a
left drag. Both turn around the model point under the pointer when the drag
begins on the model, and that point stays in place on screen, also in a
panned view; begun on empty space, they turn around the middle of the view
as before. Zoom keeps the wheel, the touchpad pinch, Shift + left drag and
the zoom controls, so only the right drag changed. The pointer point is
picked after any camera transition has ended, in the view the drag turns
(`tests/rust/viewer/test_navigation.rs::an_orbit_turns_around_the_model_point_under_the_pointer`).

### Rich console

The old console embedded an in-process Jupyter kernel and could render rich
IPython output. The isolated worker keeps user code out of the GUI process,
so rich results travel as serialized MIME bundles (`FEAT-E6`): expression
results and `display()` show their image (matplotlib figures, anything with
`_repr_png_` or `_repr_jpeg_`) and their plain text. The worker only sends
PNG or JPEG images it has checked, within size and count limits, and the
GUI decodes them away from the drawing thread, within its own limits. HTML,
Markdown, SVG and LaTeX are deliberately not rendered: they would need a web
view and would let user code put markup or scripts in the GUI; their plain
text is shown instead. Plain Python evaluation, multiline input, completion,
history, `show_object`, `debug`, stdout and errors remain covered.

### Theme settings

The editor and application no longer have independent Light/Dark choices.
One System/Light/Dark theme prevents unreadable mixed combinations and follows
the operating system by default. This is an intentional settings migration,
not a missing editor feature.

### Preference defaults

The PyQt baseline used font size `12`, deviation `1e-5` and angular deviation
`0.1`. The current defaults are `13`, `1e-3` and `0.2`. Font units and raster
metrics differ between Qt and egui, so equal numbers do not imply equal visual
size. Tessellation now occurs in a separate process and its mesh crosses the
worker protocol; the looser defaults bound routine render cost. The legacy
values remain configurable, and worker tests verify that a finer tolerance
produces a finer mesh and that large structural dimensions retain small
cross-sections. These defaults must not be described as identical.

### Distribution

The old PyInstaller and conda-constructor paths bundled the Python runtime and
dependencies. The new constructor installers retain that property while the
Rust executable keeps its worker source embedded: Windows receives an `.exe`,
Linux a self-extracting `.sh`, and macOS a `.pkg` for each supported CPU
architecture. The application prefers the bundled runtime unless the user
explicitly chooses another interpreter. Release CI installs every artifact,
imports CadQuery/OCP/Jedi, starts the installed executable with `--help`, and
publishes checksums plus the resolved package lockfile. Signing still depends
on platform certificates and is not claimed when release credentials are not
configured.

### GUI-only validation

Free-form dock dragging crosses native window boundaries. It remains
**Unverified**, not presumed equivalent. `ENG-03` tracks a headless UI
harness. The display modes and screenshots are no longer in this group:
`tests/rust/viewer/test_renderer.rs` renders them offscreen on any wgpu
adapter and checks pixels that each must produce (tiled captures included),
and `tests/rust/viewer/test_capture.rs` checks the PNG and animated PNG
files; CI requires the software adapter (WARP) on Windows, so the pixel
checks always run there. The native save dialog is the only untested part
of a screenshot.

## Change policy

Any change to a legacy capability must do one of the following in the same
commit:

1. preserve or enhance it and add/update automated evidence;
2. mark it Changed with the product decision and migration behavior; or
3. mark it as a Gap with a tracked roadmap item.

A feature may move to Verified only when its named test exercises the stated
result. Passing unrelated unit tests or merely retaining a menu label is not
equivalence evidence.
