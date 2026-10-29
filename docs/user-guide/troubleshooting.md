# Troubleshooting

Start with the **Log** panel and the status bar: most problems are explained
there. Hover the Python state at the right of the status bar to see which
interpreter is in use.

## Python and CadQuery

**The status bar says *Python unavailable* or *Python not started*.**
CQ-editor could not start the interpreter. Hover the status to read the
reason. Check the path in *Preferences > General > Python interpreter*, or
leave it empty to auto-detect. [How CQ-editor finds
Python](../getting-started/installation.md#how-cq-editor-finds-python) lists
the search order.

**A *Python was not found* or *CadQuery is not available* dialog appears.**
Either no interpreter could be started (on Windows, `python` may only be
the Microsoft Store placeholder), or it starts but cannot import CadQuery.
The dialog shows the interpreter it tried and lists the other interpreters
found on this computer (virtual and conda environments, python.org installs
and `python` on `PATH`): click one to restart Python with it. **Choose
interpreter…** picks any other one, and **Installation guide** opens
[Installation](../getting-started/installation.md). The interpreter you
choose is kept in *Preferences > General*. Alternatively, install CadQuery
into the environment it tried (`python -m pip install cadquery`) and click
**Retry**.

**The wrong environment is used.** Start CQ-editor from a terminal in which
the right environment is activated, set `CQ_EDITOR_PYTHON`, or pass
`--python PATH` on the command line.

**F5 does nothing for a few seconds after start-up.** CadQuery takes a few
seconds to import. The status bar shows *Starting Python…*. A run requested
meanwhile starts automatically when Python is ready.

**Completion does nothing.** Install `jedi` in the environment that runs
CadQuery (*Preferences > General > Install Packages…*, or
`python -m pip install jedi`) and use **Run > Restart Python**.

**CadQuery is not installed in any Python I have.** *Preferences > General >
Create Environment…* makes one with CadQuery and jedi, and switches to it.

**A package I just installed is not found.** Use **Run > Restart Python**:
modules already imported stay loaded in the running process.

## Scripts

**The script runs forever.** Press **Run > Stop** (Ctrl+Shift+F5). The script
is interrupted where it is, and the status bar says at which line. If it is
stuck inside OpenCascade and does not stop within 3 seconds, Python is
restarted; the editor stays responsive either way.

**The status bar says *Python crashed while running the script; it was
restarted*.** A native error (usually inside OpenCascade) killed the Python
process. CQ-editor restarted it. The Log panel shows what the process printed
before it died. Isolate the operation with the debugger (set a breakpoint
just before it).

**My model does not appear.**
* Did the run succeed? Check the status bar and the Traceback panel. A
  failed run keeps the previous model on screen.
* Is the object hidden? Check its box in the Objects panel.
* Was it shown? Without any `show_object` call, only CadQuery objects bound
  to public variables are displayed. With a call, only what you pass to
  `show_object`.
* Is it off-screen? Press **Shift+F1** (Fit).

**Relative paths in my script do not work.** Save the script first: an
untitled script has no folder to use as working directory. Also check that
*Preferences > Execution > Change working dir to script dir* is on.

**Edits to my helper module are ignored.** Keep *Reload imported modules*
on, and keep helpers next to the script (or on `sys.path`) rather than in
site-packages: installed packages are never reloaded.

**`input()` raises `EOFError`.** Scripts cannot read from a terminal. Use
parameters at the top of the script instead.

## Files

**"Not a UTF-8 file".** The script was saved in another encoding, typically
by an older Windows editor. Choose **Open as Windows-1252** to read it; the
file is not changed until you save, and saving converts it to UTF-8 (the
status bar shows *Windows-1252 → UTF-8* until then). If the text looks wrong,
the file uses yet another encoding: cancel, convert it with your editor's
*Save with encoding* command, then open it again.

**Automatic reload does not trigger.** The script must be saved to a file.
Check that the *Autoreload* indicator is shown in the status bar. If your
editor saves through a temporary file, raise *Autoreload delay*.

**I lost unsaved work after a crash.** Restart CQ-editor and choose **Restore
session**. If you already discarded it, look in **File > Local History…**.

## Display

**The view is slow or uses a lot of memory.** Large or highly curved models
produce many triangles (the Objects panel shows the count). Raise
*Deviation* and *Angular deviation* in *Preferences > 3D View*, or hide
objects you do not need. Turn off *Show edges* for very large assemblies.

**Rendering takes long.** After each run the Log panel shows a *Run
timings* line: how long the script itself took, the meshing, encoding and
transfer of the geometry, and how many meshes were meshed, taken from the
cache or reused unchanged. If *script* dominates, the time is spent in
CadQuery; if *meshing* does, raise the deviations as above. Parts that did
not change since the last run are reused, not meshed or sent again.

**Curved surfaces look faceted.** Lower *Deviation* (for example to 0.0005)
and *Angular deviation*, then render again.

**The window opens off-screen or larger than the screen.** Close CQ-editor
and delete `app.ron` from the [application data
folder](../getting-started/installation.md#where-cq-editor-keeps-its-files).
This also resets the preferences.

**Panels are lost or squeezed.** Use **View > Reset Layout**.

**The window does not open, or the 3D view stays black.** The graphics
driver must support Vulkan, Metal or Direct3D 12 (through wgpu). Update the
GPU driver. On Linux, check that Vulkan works (`vulkaninfo`).

## Reporting a problem

Please include the information listed in [Reporting a
bug](../../README.md#reporting-a-bug), and copy the relevant lines of the Log
panel (copy button in its toolbar). The **Help > About** dialog shows the
CQ-editor, Python and CadQuery versions.

Everything the Log panel shows is also written to `logs/cq-editor.log` in
the [application data
folder](../getting-started/installation.md#where-cq-editor-keeps-its-files),
with the date of each line. If CQ-editor itself crashed, the file keeps the
messages that led to it and the panic report. It is rotated at 2 MB; the
previous part is `cq-editor.1.log`.
