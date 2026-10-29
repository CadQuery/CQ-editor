# Changelog

## Unreleased

* Changed: the ground grid is a floor on the XY plane at the model's
  lowest point, so the model stands on it. It stays flat in every view
  instead of turning into a wall in the Front and Right views, reaches the
  edge of the view and fades into the distance, and its spacing follows the
  size of the model (5 mm for a 60 mm screw) instead of the zoom; lines too
  close together on screen fade out
* Changed: the X, Y and Z axes in the view are dashed where the model hides
  them, so a part built around the origin no longer makes them disappear,
  and a white dot marks the origin
* Changed: an orbit started on the model turns around the point under the
  pointer, which stays in place on screen, even in a panned view; started
  on empty space it turns around the middle of the view as before
* Changed: right drag orbits like left drag instead of zooming; the wheel,
  pinch and Shift + left drag still zoom
* Changed: the installers leave out the Boost, Eigen and TBB headers, CMake
  files and import libraries that the VTK package brings for compiling
  against it, which nothing uses at run time: each installer is about 19 MB
  smaller, and an installation about 230 MB and 17,000 files smaller
* Changed: the editor is built with whole-program optimization (fat LTO,
  one codegen unit), and its icon stores the 64 to 192 px images as PNG
  (the same pixels) and is embedded once: the Windows executable is 3.6 MB
  (17%) smaller

## 0.9.0 (2026-10-27)

* New: a Presentation display mode (View menu, toolbar ✦, command palette)
  for showing a model: no edges, Studio lights, matte plastic for objects
  without a material, a light grey gradient and no axes, grid or scale bar
  by default, all set apart from the modeling view in *Preferences >
  Presentation*
* New: a soft shadow on the ground under the model in the Presentation
  mode, darkest where the model touches the ground, with its strength and
  softness in *Preferences > Presentation*. It is the same from every side
  and in large or transparent screenshots
* New: materials. `show_object(part, material="matte plastic")` shades an
  object as matte or glossy plastic, brushed steel, aluminum or wood, by
  metalness and roughness, with metals reflecting a soft studio around the
  model; the Objects panel's **Material** list changes it per object.
  Metals and wood have their own color for objects without one; Default
  keeps the plain shading
* New: *Preferences > Rendering > Edges* chooses which edges are drawn:
  all of them, **Sharp edges** (without the edges between faces that
  continue smoothly, such as fillet borders and cylinder seams, below a
  *Sharp edge angle*), or **Outline** (sharp edges plus the outline of
  curved surfaces seen from the camera, also in Hidden Line mode). The
  worker measures the angle at every edge (mesh format 2)
* New: *Preferences > Rendering* sets the lighting of the 3D view: key,
  fill, ambient and rim lights, highlights and shininess, the direction of
  the key light, exposure and contrast, with Default, Studio, Bright and
  Dramatic presets. Default keeps the earlier look
* Fixed: one NaN in a `show_object` option or in an object's placement lost
  the whole run, every valid object included, with *ValueError: Out of range
  float values are not JSON compliant*. A non-finite option is now ignored
  with a warning (the object's other options still apply), and an object
  that cannot be sent is left out and named in the log (BUG-40)
* Fixed: a model too large to transfer from Python (over 512 MiB of mesh,
  about ten million triangles) was reported as *CadQuery is not
  available*, and the run never finished. It is now explained, with how to
  coarsen the mesh, and Python is restarted with the previous model kept
  on screen (BUG-39)
* Fixed: closing the tab of a script paused in the debugger left the
  session paused on a document that was gone; it now stops it (BUG-38)
* Fixed: a part or sub-assembly selected inside an assembly and exported to
  STEP lost the placement of its parents (STL and BREP were right), and
  parts lost the color inherited from their parents, which the view shows
  but CadQuery's STEP writer leaves out (BUG-37)
* Fixed: with the Editor panel closed, opening a file or going to a
  traceback line showed it in an editor nobody could see, and the closed
  panel was restored that way at every start. The Editor now comes back,
  left of the 3D view, whenever a file or a line is shown and at start-up,
  and the empty view says when it is closed (BUG-36)
* Fixed: in a folder where case matters (Linux, case-sensitive macOS
  volumes or Windows folders), renaming `part.py` to `Part.py` in the
  Project panel replaced an existing `Part.py`; it is now refused, while
  changing only the case of a name still works (BUG-35)
* Fixed: keyboard shortcuts ran behind modal dialogs. Ctrl+Tab under the
  File changed on disk dialog made it describe another file, and its answer
  was then dropped, leaving the conflict unresolved. Shortcuts now wait
  until a dialog is answered, and a conflict is always resolved on the
  file it concerns (BUG-34)
* Fixed: on Windows, the local history of a file depended on whether its
  path was written with `/` or `\`, so some versions could be missing from
  File > Local History (BUG-41)
* Fixed: saving replaced a version of the file that another program had
  written since it was opened or saved: with automatic reload off (the
  default), a Ctrl+S could undo a `git pull`. Saving now asks first, with
  the File changed on disk dialog, and keeps the replaced version in the
  local history (BUG-33)
* New: View > Command Palette (Ctrl+Shift+P) finds any command, tab,
  recent file, example, panel or named view by a few letters of its name,
  and shows its shortcut
* New: a panel can fill the window: double-click its tab, use View >
  Maximize Panel (Ctrl+Shift+M) or its tab's menu; the same brings the
  layout back, and the saved layout stays the whole one
* New: a panel tab's right-click menu has Maximize and Reset Layout, and
  names its other items Float in a Window and Close Panel
* Fixed: in a narrow window the toolbar's last groups were cut off; they now
  move to a » menu at its end
* Changed: the Render button is green, and with several files open the
  editor panel's tab reads "Editor" (their names are on the tab strip)
* New: multiple files in tabs (FEAT-D1). Each tab keeps its caret,
  scrolling, undo history and breakpoints; Ctrl+Tab / Ctrl+Shift+Tab switch,
  Ctrl+W closes, and File > Save All saves them all. Opening a file no
  longer replaces the one shown, so it no longer asks to save it first
* New: File > Open Folder (Ctrl+Shift+O) shows the folder in a Project panel
  that opens, creates, renames and deletes files and folders; renames keep
  open tabs, breakpoints and named views in step
* New: a project's main script, guessed when clear and chosen from the
  panel, a tab or the Run menu: while a file of the project is shown, F5,
  Profile and Debug run the main script, whose errors, debug line and
  parameters stay in its tab. The status bar names it, and a render warns
  about unsaved modules that Python will not see
* New: the Traceback panel opens the imported module of a frame at its line
* New: the open tabs and the project folder are restored at the next start;
  closing or quitting asks about every unsaved tab at once, and crash
  recovery and automatic reload cover every tab
* Fixed: text and partially visible rows in docked panels are clipped to their
  own tab body instead of painting over split separators or neighbouring tabs
* Fixed: rich console results are tied to the command that produced them;
  retained or background `display()` calls can no longer appear as output of
  a later command
* Fixed: upload and release-check responses have explicit limits, preventing
  an oversized HTTP response from consuming unbounded worker memory or
  exceeding the GUI protocol limit
* Security: updated Tokio to 1.54.1 to address RUSTSEC-2026-0384, and made CI
  reject dependency-lock drift and Rust compiler warnings
* Quality: the worker CI now rejects Python syntax/import errors with Ruff
  and enforces at least 85% branch-aware test coverage

## 0.8.0 (2026-10-22)

The interface was rebuilt in Rust (egui + wgpu); CadQuery now runs in a
separate Python worker process (`worker/cq_worker`). The core script contract,
legacy shortcuts and mouse bindings are preserved. Full GUI equivalence is
tracked in the compatibility matrix instead of being assumed.

* New: a running or crashing script no longer freezes or closes the editor;
  Run > Stop (Ctrl+Shift+F5) aborts it and a crashed worker restarts itself
* New: self-contained installers for Windows, Linux, macOS Apple Silicon and
  macOS Intel bundle the tested Python 3.12, CadQuery 2.8, OCP 7.9 and Jedi
  runtime; every installer is smoke-tested after installation and published
  with SHA-256 checksums and its resolved dependency lockfile
* New: installed applications add a native launcher; Windows and Linux also
  register CQ-editor as an option for opening Python files without replacing
  the user's chosen editor inside CQ-editor
* New: GPU renderer with MSAA, smooth camera animations, zoom at the mouse
  pointer, hover highlighting and double-click to zoom to an object
* New: section view (View > Section View); a plane across X, Y or Z, with a
  position slider and a flip, cuts the model and fills the cut faces of
  solids; picking goes through what the plane removes
* New: CQ-editor looks for a newer release of itself once a day (Preferences
  > General) and offers it in the Help menu; Help > Check for Updates now
  covers CQ-editor and CadQuery
* New: environments from the preferences: pick a Python found on the
  computer, create a virtual environment with CadQuery and jedi, or install
  packages into the Python in use, with pip's output shown and Cancel
* New: the console shows pictures, as Jupyter does: a matplotlib figure or
  an object with `_repr_png_` / `_repr_jpeg_` is drawn in the console, and
  `display()` shows several values; images are checked and limited before
  they reach the editor, and HTML is shown as plain text
* New: Run > Profile (Alt+F5) shows in the gutter the time each line took,
  with what it called, and in a tooltip its CadQuery operations by time
* New: Stop interrupts the script or console command first, keeping Python,
  the console's variables and the mesh cache, and reports the line it
  stopped at; Python is restarted only if the script does not stop within
  3 seconds or Stop is pressed again
* New: the original editor's stereo projections (Stereo, Mono Left Eye,
  Mono Right Eye) and stereo modes (anaglyph, row/column interlaced, chess
  board, side by side, over-under); Quad Buffer shows the anaglyph with a
  note, since no current renderer backend can present it
* New: ground grid on the XY plane (View > Ground Grid) whose spacing follows
  the zoom, and a scale bar (View > Scale Bar) in the units chosen in the
  preferences
* New: large models; parts outside the view are not drawn, parts small on
  screen use a simplified mesh (Preferences > 3D View > Large models), and
  meshing shows its progress in the view and can be cancelled while the
  previous model stays
* New: screenshots at the view's size ×1/×2/×4 or any size up to 8192
  pixels, with a transparent background if wanted, and turntable animations
  as an animated PNG or numbered PNG files (Tools > Screenshot…)
* New: named views; save the camera under a name from the toolbar or View >
  Named Views and come back to it later; views are remembered per file
* New: X-Ray (Shift+F11) and Hidden Line (Shift+F12) display modes; hidden
  edges are dashed and sections hatched in Hidden Line, and each object can
  show or hide its own edges in the Objects panel
* New: exploded view (View > Exploded View) with a 0–200% slider, and
  Isolate Selection / End Isolation / Show All in the View menu and the
  Objects panel; picking and framing follow the exploded parts, isolation
  follows objects by name across runs
* New: the Measurements panel reports exact bounding boxes, measures the
  selected objects (volume, area, center of mass) when no entity is picked,
  and copies a measurement as Python using the script's variables and
  selectors, checked by running it
* New: Go to Source (Ctrl+Shift+G) from an object, its property sheet or a
  CQ Inspector step to the script line that created it, following the
  edits made since the run
* New: Parameters panel; the script's top-level numbers as spin boxes and
  sliders that rewrite their literal and render again, with ranges from
  `# [min, max, step]` comments and one Undo step per drag
* New: `selected()` in the console returns the picked faces, edges and
  vertices (or the selected objects' shapes) as live CadQuery shapes, for
  measurements and experiments
* New: selection to code; picked faces, edges or vertices get a CadQuery
  selector (`.faces(">Z")`, `.edges("|Z and >Y")`, `.edges("|Z")` for
  several), verified with CadQuery, to copy or insert at the cursor
  (Ctrl+Shift+I), with alternatives and a flagged index fallback; Ctrl+click
  now gathers up to 64 entities
* New: precise OCCT face, edge and vertex selection with subshape highlighting
  and live-scene topology identifiers retained by the Python worker
* New: Auto/Object/Face/Edge/Vertex selection modes and an exact Measurements
  panel for OCCT distance, closest points, angle, length, area, surface/curve
  type, radius and diameter; topology ids are explicitly scoped to one render
* New: navigation cube redrawn natively, with hover feedback and the 45° roll
  arrows; view icons follow the theme
* New: debugger "Step out" (Ctrl+Shift+F11) and a cleaner stepping model
* New: editor auto-indentation, current-line highlight, match-case search,
  completion popup filtered while typing and call signature tips
* New: object colors and opacity can be changed in the Objects panel, which
  also shows sizes and triangle counts
* New: Open Recent, Reset Layout, Restart Python, BREP export; single
  assemblies export to STEP with their names and colors
* New: Log panel with level filters; console with history
* New: atomic saves that preserve line endings, the UTF-8 BOM, permissions
  and file metadata; files that are not valid UTF-8 are never rewritten
  without asking
* New: crash recovery (checkpoints every 5 seconds while a buffer is
  modified, restore on the next start) and File > Local History with the
  newest 20 versions per document
* New: automatic reload never discards unsaved edits; a dialog offers to
  keep them, reload, save a copy or compare
* New: Edit > Undo and Redo (Ctrl+Z, Ctrl+Y or Ctrl+Shift+Z) with an
  editor history of its own: typing is undone a burst at a time, and an
  indent, a comment, an accepted completion, a Replace all or a reload is
  one step; undo no longer brings back the text of a previous document
* New: Replace (Ctrl+H), whole-word search and Go to Line (Ctrl+G)
* New: completions are offered after the first characters of a name
  (three by default, configurable), and resting the pointer on a name
  shows its signature and docstring, or the type of a variable
* New: breakpoints are remembered per file across sessions and follow
  the text of their lines when the file changed meanwhile
* New: the Objects property sheet shows the type, area or length, radius
  and center of a selected face, edge or vertex
* New: when Python or CadQuery is missing, the dialog lists the
  interpreters found on the computer, and offers to choose one or to open
  the installation guide (also in the Help menu)
* New: content-addressed mesh cache and assembly instancing; unchanged
  meshes are not sent or uploaded again
* New: hover and click selection stay fast on large assemblies; a retained
  spatial index of scene parts and per-mesh edge and vertex indexes replace
  the linear scan (about 0.1 ms instead of 150 ms per pointer move with
  16,384 parts)
* New: documentation in `docs/` (user guide, development guide, roadmap)
* New: a ninth example, a welded 20 x 20 mm square-tube workbench built as
  an assembly, with its cutting plan and a tube summary in the Log panel
* New: a tested compatibility matrix inventories original features, guards
  legacy shortcuts and mouse gestures, and runs every pre-rewrite example
* New: CI tests the worker against the supported Python/CadQuery/OCP release
  matrix and the current CadQuery development branch; test, benchmark and
  application logs now identify the exact runtime versions
* Changed: theme follows the system by default; the separate editor color
  scheme preference was merged into the application theme
* Changed: the Quad Buffer stereo mode cannot be presented by the new
  renderer and shows the red-cyan anaglyph, with a note saying so
* Fixed: the 3D view failed on systems where wgpu falls back to OpenGL
  (no Vulkan, or software rendering), which cannot view the scene's sRGB
  texture in another format; it now gets a plain copy encoded by a small pass
* Fixed: with OCP 8, bounding boxes in the Measurements panel and in the
  code copied from it failed (`Bnd_Box.Get()` returns a type OCP 8 cannot
  convert); the box corners are read instead
* Fixed: `rand_color()` defaulted to a fully transparent color
* Fixed: after Stop, Restart Python or a worker crash, the first run could
  free its own objects, so the CQ Inspector, export and upload failed; scene
  keys are now unique per worker process
* Fixed: the CI format check (`black`) failed on three worker files
* Fixed: after switching Python environments or upgrading OCP, a render
  could reuse meshes made by the previous mesher; mesh ids now include
  the mesh format and the OCP build, and conda installs report their OCP
  version instead of `unknown`
* Fixed: the CQ inspector could draw the geometry of a previously
  selected step or workplane when its reply arrived late
* Fixed: the first Enter or F3 after typing a search skipped the first
  match
* Fixed: OCP 8 moved generated collection types and removed the `_s` suffix
  from `TopoDS` downcasts, preventing the worker from importing with current
  CadQuery `master`; one compatibility shim now supports both OCP 7 and 8
* Fixed: equal per-face triangle counts could conceal a changed glTF face
  order and assign the wrong OCCT face id; every proposed face group is now
  checked geometrically before display triangles are accepted for picking
* Fixed: script output without a newline and JSON protocol lines could grow
  without a bound; output is now chunked and truncated per request, while
  control, mesh and native-stderr messages have separate reader limits
* Fixed: in a short Objects pane, the property sheet grew over the object
  tree; it now scrolls and leaves the tree its room
* Fixed: breakpoints and the error line stayed on their line numbers when
  lines were added or removed above them; they now follow their statements
* Fixed: while debugging, a paused view that no longer showed an object made
  the next pauses and the final result fail with "Cannot update the 3D
  scene" once the object reappeared; meshes now stay available for the
  whole session
* New: after each run the Log panel reports the time spent in the script,
  meshing, encoding, transfer and decoding, and how many meshes were reused;
  GPU uploads are logged at the debug level
* New: faster meshing for selection; the display triangles carry their OCCT
  face ids instead of a second set of pick triangles, so detailed parts mesh
  up to 3x faster and their messages are about a third smaller
* New: recently used meshes and their GPU buffers are kept (up to 128 MB),
  so rendering after Clear, debugging and repeated runs transfer and upload
  only geometry that changed
* Fixed: the Previous match tooltip now shows its actual Alt+F3 shortcut
  instead of conflicting with the Shift+F3 Top view shortcut
* Fixed: Ctrl+left-drag explicitly selects Pan even when a platform event does
  not also set egui's synthesized Command modifier
* Fixed: worker replies, automatic reload and recovery checkpoints are
  handled while the window is minimized, instead of waiting for a repaint
* Fixed: icons in dialog titles (Preferences, Upload Model) showed as empty
  boxes
* Fixed: Help > Documentation opens these docs instead of the old PyQt wiki
* Fixed: with *Autoreload: watch imported modules*, modules imported after
  watching started were never watched; the list is refreshed after each save
  and successful run, without duplicates or modules no longer imported
* Fixed: the Variables panel showed every value as "…"; the value column now
  gets the room the other columns leave, and clicking a row shows the whole
  value (up to 2000 characters), ready to copy
* Fixed: after a failed run, the empty view and Objects panel point to the
  Traceback instead of asking to press F5 again; empty-panel texts are no
  longer justified with uneven word spacing
* Fixed: opening another file cleared nothing, so the previous file's model
  looked like the result of the new script; a run or debugging session still
  going for the previous file is discarded
* Fixed: `debug()` in the console named objects *Workplane 1* instead of
  after their variable
* Fixed: `--help` and command-line errors printed nothing from the Windows
  release executable
* Fixed: every release left its extracted worker folder behind; a new one
  removes the others
* Fixed: each recovery checkpoint read every history file of every document;
  history now lives in one folder per document and is pruned by age (90
  days) and total size (64 MB), which also clears the history of deleted
  files
* Fixed: files that are not UTF-8 could not be opened; they can now be read
  as Windows-1252 and are converted to UTF-8 on save
* Fixed: the Windows executable had no icon or version information in
  Explorer
* Fixed: building with the Windows GNU toolchain needed a hand-made
  `libshlwapi.a`; links are opened with the `open` crate
* Fixed: GUI panics and the messages before them were lost; the log is also
  written to `logs/cq-editor.log` in the data folder
* Fixed: a worker stopped while meshing left `cq-editor-*.glb` files in the
  temporary folder; meshing uses one scratch folder per worker and the next
  worker removes leftovers
* Fixed: quitting with *Don't save* offered the discarded edits as unsaved
  work at the next start
* Fixed: *Stop debugging* could be swallowed by an `except Exception` in the
  script
* Fixed: the editor could crash when an edit moved the completion popup's
  anchor inside a multi-byte character
* Fixed: a link in an upload service's reply could open a local file or
  program; only http and https links are opened
* Fixed: a long status message pushed the Python state, cursor position and
  encoding out of the status bar
* Fixed: the Log filter matched message sources case-sensitively
* Fixed: a scene that could not be decoded also dropped the run's variables
  and a queued render; a failed run request left the debugger arrow and
  dropped a queued render
* Fixed: clicking the text of a Traceback row did not jump to its line, only
  the empty part of a cell did
* Removed: the PyQt GUI, PyInstaller and conda constructor packaging

## Release 0.5.0

* Implemented a dark theme and tweaked it to work better with icons (#490)
* Fixed bug causing the cursor to jump when autoreload was enabled (#496)
* Added a max line length indicator (#495)
* Mentioned work-around for Linux-Wayland users in the readme (#497)
* Fixed bug where dragging over an object while rotating would select the object (#498)
* Changed alpha setting in `show_object` to be consistent with other CadQuery alpha settings (#499)
* Added ability to clear the Log View and Console (#500)
* Started preserving undo history across saves (#501)
* Updated run.sh script to find the real path to the executable (#505)

## Release 0.4.0

* Updated version pins in order to fix some issues, including segfaults on Python 3.12
* Changed to forcing UTF-8 when saving (#480)
* Added `Toggle Comment` Edit menu item with a `Ctrl+/` hotkey (#481)
* Fixed the case where long exceptions would force the window to expand (#481)
* Add stdout redirect so that print statements could be used and passed to log viewer (#481)
* Improvements in stdout redirect method (#483 and #485)
* Fixed preferences drop downs not populating (#484)
* Fixed double-render calls on saves in some cases (#486)
* Added a lint check to CI and linted codebase (#487)
