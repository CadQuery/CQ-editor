# The interface

![Main window, light theme](../images/main-window-light.png)

The window has one idea: **the script is the model**. The code editor sits on
the left and the 3D view in the center. The panels around them help you
understand what the code produced. Each open file has a tab in the editor,
and an open folder is shown in the Project panel, left of the editor (see
[Projects and multiple files](projects.md)).

```
┌───────────────────────────── menu bar · toolbar ─────────────────────────────┐
│ Editor             │ 3D View                         │ Objects               │
│ (your script)      │ (navigation cube, axes)         ├───────────────────────┤
│                    │                                 │ Variables · Inspector │
├────────────────────┴───────────────┬─────────────────┴───────────────────────┤
│ Console                            │ Traceback · Log                         │
├────────────────────────────────────┴─────────────────────────────────────────┤
│ status messages              Ln, Col · line ending · UTF-8 · Python/CadQuery │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Panels

Every panel is a dockable tab. Drag a tab to move it, drop it on another
panel to stack it, or close it. **View** lists all panels as toggles, and
**View > Reset Layout** restores the arrangement above. The layout is saved
between sessions.

The Editor panel can be closed too, for example to write the script in
another editor with automatic reload. It comes back, left of the 3D view,
as soon as a file or a line is shown: opening or creating a file, switching
tabs, a traceback line, *Go to Source*. A layout saved without it gets it
back at the next start, and while it is closed the empty 3D view says so.

* **Double-click a panel's tab** (or **View > Maximize Panel**, Ctrl+Shift+M,
  for the panel you last clicked in) to let it fill the window, for example
  the editor while you write or the 3D view while you inspect a model.
  Double-click the tab again, or press Ctrl+Shift+M, to bring the layout
  back. Going to a line, showing a panel or resetting the layout also brings
  it back, and the layout saved between sessions is always the whole one.
* **Right-click a panel's tab** for **Maximize** / **Restore Layout**,
  **Reset Layout**, **Float in a Window** (the panel moves to a window of its
  own inside CQ-editor; drag its tab back to dock it again) and **Close
  Panel**.

| Panel | Purpose | Details |
| --- | --- | --- |
| **Editor** | The script, with highlighting, breakpoints and completion; a tab per open file | [Code editor](editor.md), [tabs](projects.md#tabs) |
| **Project** | The open folder's files: open, create, rename and delete them, choose the main script | [Projects and multiple files](projects.md#the-project-panel) |
| **3D View** | The rendered model, navigation cube, axes indicator | [3D view](viewer.md) |
| **Objects** | Tree of shown objects and helpers (X/Y/Z axes), with visibility toggles and a property sheet (type, visibility, color, opacity, size, triangle count) | [below](#objects-panel) |
| **Measurements** | Exact OCCT properties for one selected face, edge or vertex, and distance/angle for two | [3D view](viewer.md#exact-measurements) |
| **Parameters** | The script's top-level numbers as spin boxes and sliders; a change rewrites the line and renders again | [Code editor](editor.md#parameters-panel) |
| **Variables** | Name, type and value of the script's variables after a run, or of the locals while debugging | [Console, variables and logs](console.md#variables) |
| **CQ Inspector** | The construction stack of the selected workplane | [Debugging and inspection](debugging.md#cq-inspector) |
| **Console** | Interactive Python sharing the last run's variables | [Console, variables and logs](console.md) |
| **Traceback** | The last error, frame by frame; click a row to jump to the line, in the script or in an imported module | [Console, variables and logs](console.md#traceback) |
| **Log** | `print` output, `log()` messages, warnings from Python and OpenCascade | [Console, variables and logs](console.md#log) |

### Objects panel

* **CQ models** lists what the script showed. Assemblies appear as a tree
  with one node per part (or as a single object with *Merge assemblies*).
* **Helpers** holds the X, Y and Z axes.
* The checkbox toggles visibility. The color square is the object's color.
* Click to select, Ctrl+click to add to the selection, double-click to zoom
  to the object.
* Right-click opens a menu with **Zoom to**, **Go to Source**,
  **Show/Hide**, **Isolate** / **End Isolation**, **Show All**, export and
  removal (see [Exploded view and
  isolation](viewer.md#exploded-view-and-isolation)). Objects hidden by an
  isolation are grayed. **Go to Source** (also *Edit > Go to
  Source*, Ctrl+Shift+G, and the **Source** row of the property sheet)
  shows the script line that created the object: the line of the last
  operation of a chain, the line inside a function of the script that made
  it, or the line of the `add` call for an assembly part. Lines follow the
  edits made since the run; when the line itself was edited, the status
  bar says so.
* The toolbar at the top has **Clear all**, **Remove selected** (×) and
  **Export** (the selection, or everything when nothing is selected).
* The property sheet below the tree edits the selected object: visibility,
  **Color** (with **Reset** to go back to the script's colors), **Opacity**,
  **Edges** (this object's edges in the Shaded, X-Ray and Presentation
  modes, with **Reset** to follow the preference), **Material** (see
  [Materials](viewer.md#materials), with **Reset** to go back to the
  script's), and shows its **Size** and
  **Triangles**. When a face, edge or vertex is
  selected, its id is shown too (for example *Face 6 · part 1*) with its
  exact measurements: the surface or curve type, the **Area** of a face,
  the **Length** of an edge, a **Radius** when there is one, and the
  **Center** (the **Position** of a vertex). Its **Selector** row has
  buttons to copy the CadQuery selector of the picked entities or insert it
  at the editor's cursor (see [Selection to code](viewer.md#selection-to-code)).

Colors and visibility changed here last until the next run. Turn on
*Preferences > Objects > Preserve properties on reload* to keep them across
runs (objects are matched by name).

## Toolbar

From left to right:

| Group | Buttons |
| --- | --- |
| File | New, Open, Save, Save as, Automatic reload and preview |
| Objects | Clear all objects, Clear current object |
| Run | Render, Profile, Debug, Step, Step in, Step out, Continue, Stop |
| Inspect | Inspect CQ object |
| View | Fit, then Iso, Top, Bottom, Front, Back, Left, Right, Named views |
| Display | Wireframe, Shaded, X-ray, Hidden line, Presentation, Section view, Exploded view, Screenshot |
| Selection | Auto, Object, Face, Edge or Vertex drop-down |

Buttons that cannot be used right now (for example *Step* when no debugging
session is paused) are disabled. Hover a button to see its shortcut. The
green ▷ **Render** button is the one to start with.

When the window is too narrow for every group, the last groups move to the
**»** button at the end of the toolbar, which shows them in a menu.

## Command palette

**View > Command Palette…** (Ctrl+Shift+P) searches every command: type a
few letters of its name, in order, such as `exp stl` for
*Export as STL…* or `max` for *Maximize Panel*. Use ↑/↓ and Enter, or click;
Esc closes it. Each command shows its menu and its shortcut, so the palette
also teaches the shortcuts. Besides the menus' commands, it finds the open
tabs, named views, recent files and folders, examples and panels. Commands
that cannot be used right now, such as *Step* when no debugging session is
paused, are not offered.

## Menus

| Menu | Items |
| --- | --- |
| **File** | New · Open… · Open Folder… · Open Recent · Save · Save As… · Save All · Local History… · Close Tab · Close Folder · Automatic Reload and Preview · Examples · Quit |
| **Edit** | Undo · Redo · Toggle Comment · Auto-Complete · Search · Replace · Go to Line… · Copy Selector · Insert Selector · Go to Source · Preferences… |
| **Tools** | Export as STEP… · Export as STL… · Export as BREP… · Inspect CQ Object · Screenshot… · Upload Model… |
| **Run** | Render · Set as Main Script / Clear Main Script · Profile · Clear Profile · Debug / Stop Debugging · Step · Step In · Step Out · Continue · Stop · Clear Console · Clear Log · Restart Python |
| **View** | Command Palette… · Fit · Zoom to Selection · Iso · Top · Bottom · Front · Back · Left · Right · Named Views · Wireframe · Shaded · X-Ray · Hidden Line · Presentation · Perspective Projection · Section View · Exploded View · Ground Grid · Scale Bar · Isolate Selection / End Isolation · Show All · panel toggles · Maximize Panel · Reset Layout |
| **Help** | Documentation · CadQuery Documentation · Download CQ-editor *x.y.z*… (when a newer release is out) · Check for Updates · About |

All shortcuts are listed in the [keyboard and mouse reference](shortcuts.md).

## Status bar

* **Left:** the latest status message with an icon: success (*Rendered 1
  object in 0.09 s*), information, warning or error (*AttributeError: …
  (line 4)*).
* **Right:** the project's main script (▶ *main.py*) while another file of
  the project is shown, since F5 renders it; an *Autoreload* indicator when
  automatic reload is on, the cursor position (*Ln, Col*), the file's line ending (*LF*, *CRLF* or
  *CR*), the encoding (*UTF-8*) and the state of Python: *Starting Python…*,
  *Python 3.x · CadQuery 2.x*, *Python not started* or *Python unavailable*.
  Hover the Python state to see the CadQuery and OCP versions and which
  interpreter is used.

The window title shows the file name and folder, for example
*bracket.py — demo — CQ-editor*. An asterisk after the name in the title,
and a dot on the editor's tabs, mark unsaved changes. A dot on the Traceback
tab means the last run failed.

## Themes and scale

*Preferences > General > Theme* chooses **System** (default), **Light** or
**Dark**. The editor's syntax colors follow the theme.

![Main window, dark theme](../images/main-window-dark.png)

*Interface scale* (0.75–2.0) zooms the whole interface on top of the system
scale. Ctrl + plus and Ctrl + minus do the same while you work.

## Python status and long runs

Scripts run in a separate Python process, so the window always stays
responsive:

* **Run > Stop** (Ctrl+Shift+F5) interrupts a script or console command that
  takes too long, keeping Python, its console and its caches; Python is
  restarted only if the script does not stop within 3 seconds, or when Stop
  is pressed again (see [Debugging](debugging.md#stopping-a-runaway-script)).
* If Python crashes (for example inside OpenCascade), the status bar says so
  and a fresh Python process starts on its own.
* **Run > Restart Python** starts a fresh process on demand, for example
  after installing a package.

A restart forgets the console's variables. The objects stay on screen, but
the new Python process does not know them: render again before exporting
or inspecting them.
