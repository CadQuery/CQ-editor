# Roadmap

This is the plan for CQ-editor after `0.8.0`. It comes from a review of the
whole source tree (commit `74c1535`, October 2026) and of the libraries it
builds on, checked with CadQuery 2.7 / OCP 7.8 (the development
environment; the installers ship CadQuery 2.8 / OCP 7.9). It has four
parts:

1. **[Bugs](#1-bugs-and-problems)**: defects that were reproduced, not
   suspected. Each one was confirmed by running code against the current
   tree. A suspicion that did not reproduce is not listed.
2. **[Required improvements](#2-required-improvements)**: work that follows
   from what CQ-editor is, an IDE whose product is CAD data made by code, or
   from a maintenance duty. These are not optional.
3. **[Suggested improvements](#3-suggested-improvements)**: work that fits the
   code and would make it better, but that the program can live without.
4. **[Direction](#4-direction)**: where CQ-editor can go to become the
   reference tool for building 3D objects with code, based on what CadQuery
   and OpenCascade already offer.

The items of the previous roadmap (BUG-01…BUG-32, PERF-01…PERF-04 and the
finished FEAT items) are all resolved. They are summarized under
[Completed](#completed), and described in the [changelog](changelog.md). Items
that were still open have moved into the sections below.

**Evidence** says how each bug was confirmed:

* **test**: a throwaway Rust test, run against the tree. It asserted the
  faulty behaviour and passed. Each bug says what the test did, so it can
  become the regression test of the fix, with its assertion turned around.
* **script**: a Python script run against the worker package, with
  CadQuery 2.7 / OCP 7.8.
* **seen**: observed in the running application.

**Priorities:** **P1** fix before the next release · **P2** should fix soon ·
**P3** fix when the code is touched.

Want to help? Pick an item, open an issue that mentions its id (for example
`REQ-04`), and see [Contributing](development/contributing.md).

## Guiding principles

CQ-editor is, first of all, **an IDE for running Python CadQuery code that
builds 3D models**. Everything on this roadmap has to respect that:

1. **The code is the model.** The script stays the single source of truth.
   GUI features may *read* the model (measure, inspect, select) and may
   *write code* (insert a selector, change a parameter), but they never keep
   design intent in a hidden GUI-only state.
2. **One tool for beginners and experts.** Beginners need a guided path,
   clear errors and things they can click. Experts need speed, keyboard
   control, multi-file projects and automation. Advanced features are
   available but stay out of the way until needed (progressive disclosure).
3. **Compatibility.** Scripts written for the original CQ-editor, and the
   `show_object` conventions shared with other CadQuery tools, keep working.
   The [compatibility matrix](development/compatibility.md) distinguishes
   tested equivalence, intentional changes and open gaps.
4. **Never block, never lose work.** Python runs in its own process. Saves
   are atomic. Unsaved work is recoverable, and no file is replaced with
   contents the user has not seen.
5. **What leaves the editor is right.** An exported file carries the shapes,
   placements, names, colors and units the code produced, or the editor says
   what it could not keep.

## Where we are

The modeling workflow is complete and well tested. Scripts run in an
isolated worker. The view picks faces, edges and vertices exactly, and the
GUI writes selectors, parameters and measurements back as code. Projects
with several files, crash recovery and local history are in place. The
weak points found by this review are at the seams: the file system shared
with other programs, keyboard focus under dialogs, the pipe between the two
processes, and the export of what the view shows.

The libraries offer much more than the editor uses. This table was checked
against the installed packages (CadQuery 2.7.0, OCP 7.8.1):

| Capability of CadQuery / OCP | In CQ-editor today |
| --- | --- |
| `Workplane`, `Shape`, `Sketch`, `Assembly` display, nested assemblies, instancing | ✅ Shown, picked, measured |
| Free-function API (`cadquery.func`: `box`, `extrude`, `loft`, `sweep`, `fillet`, `offset`, `text`, `imprint`, `split`, `fill`, `cap`, `check`, `closest`…) | Usable in scripts; no snippets or help of its own |
| Per-face names, colors and layers (`Assembly.addSubshape`) | ✅ Drawn and shown when picking ([REQ-07](#req-07--show-everything-the-cadquery-api-can-specify)) |
| Materials (`cq.Material`: name, density, visual material) | ✅ Name, density, supported visual base color and measured mass |
| Assembly constraints (`constrain`, `solve`, casadi) and sketch constraints (nlopt) | Only the solved result is shown |
| Assembly export: STEP (default or fused), OCAF XBF/XML, glTF/GLB, VRML, VTK.js, STL | STEP, only when one assembly is exported alone |
| Shape export: STEP, STL, BREP, 3MF, AMF, SVG and DXF (2D), VRML, VTP, TJS, BIN | STEP, STL, BREP |
| Import: STEP (also as an assembly with names and colors), BREP, BIN, DXF | Through code only |
| `cqgi` script interface (`describe_parameter`, typed parameters) used by `cq-cli` and servers | ❌ `describe_parameter` is not defined in scripts |
| OCP: `BRepExtrema` (distances) | ✅ Measurements |
| OCP: `BRepGProp` (mass properties), `BRepCheck` / `ShapeFix` (validity, healing), `HLRBRep` (hidden-line projection), `XCAFDimTolObjects` (PMI in STEP AP242), `TNaming` (topological naming), `BRepFeat`, `GeomPlate` | Not used |

---

## 1. Bugs and problems

| Status | Id | Problem | Priority |
| :---: | --- | --- | --- |
| ✅ | [BUG-33](#bug-33--saving-replaces-a-newer-version-on-disk-without-a-word) | Saving replaces a newer version on disk without a word | P1 |
| ✅ | [BUG-34](#bug-34--shortcuts-act-behind-modal-dialogs-and-a-conflict-can-vanish) | Shortcuts act behind modal dialogs, and a conflict can vanish | P1 |
| ✅ | [BUG-35](#bug-35--renaming-a-file-only-by-case-replaces-another-file) | Renaming a file only by case replaces another file | P1 |
| ✅ | [BUG-36](#bug-36--a-closed-editor-panel-never-comes-back) | A closed Editor panel never comes back | P2 |
| ✅ | [BUG-37](#bug-37--step-export-of-a-sub-assembly-loses-its-placement) | STEP export of a sub-assembly loses its placement | P2 |
| ✅ | [BUG-38](#bug-38--closing-the-tab-of-a-paused-script-leaves-the-debugger-paused) | Closing the tab of a paused script leaves the debugger paused | P2 |
| ✅ | [BUG-39](#bug-39--a-model-too-large-to-transfer-is-reported-as-missing-cadquery) | A model too large to transfer is reported as missing CadQuery | P3 |
| ✅ | [BUG-40](#bug-40--one-invalid-display-option-loses-the-whole-run) | One invalid display option loses the whole run | P3 |
| ✅ | [BUG-41](#bug-41--the-local-history-depends-on-how-a-path-is-written) | The local history depends on how a path is written | P3 |

### BUG-33 · Saving replaces a newer version on disk without a word

✅ **Resolved.** `Document::unseen_disk_version` reads the file before a
save and returns it when it is neither the saved baseline nor the buffer's
text; a deleted file is simply written again. `AppState::save` then writes
nothing and opens the **File changed on disk** dialog (`on_save`), with
**Save my version**, **Reload from disk**, **Save a copy…** and **Cancel**;
Escape cancels. Keeping one's version, from a save or from the watcher,
archives the disk version in the local history and adopts it as the
baseline (`adopt_disk_baseline`), so the next save goes ahead. Tests:
`a_save_finds_the_version_another_program_wrote`,
`saving_never_replaces_a_version_another_program_wrote_unseen`.

*Evidence: test.* **Where:** `src/document.rs` (`Document::save`, `write`),
`src/app/documents.rs` (`AppState::save`).

Automatic reload is off by default (`Editor::autoreload`). In that state
nothing compares the file on disk with what the editor loaded. When another
program changes the file (a `git pull` or `checkout`, VS Code, a formatter),
**File > Save** writes the editor's older text over it. It does so even when
the buffer was never edited, so a habitual Ctrl+S is enough. The overwritten
version is not archived in the local history. This contradicts the user
guide ("CQ-editor never silently throws away your edits").

**Reproduce:** open `part.py`, change it with another program, press Ctrl+S
in CQ-editor. The other program's change is gone. The test also checked
that `RecoveryStore::history` holds no copy of it.

**Fix:** the document remembers a fingerprint of the file it loaded or
saved: length, modification time and the hash it already keeps. `save`
reads the disk version first. If it differs from the saved baseline, save
does not write. It opens the **File changed on disk** dialog (Keep mine and
overwrite, Reload, Save a copy, Compare) and archives the disk version with
`RecoveryStore::archive_path` before any overwrite. A file deleted since it
was opened gets the same question. See [REQ-01](#req-01--saving-checks-the-disk-first).

### BUG-34 · Shortcuts act behind modal dialogs, and a conflict can vanish

✅ **Resolved.** `begin_frame` collects global shortcuts only while
`Dialogs::modal_open()` is false (about, unsaved changes, file changed on
disk, recovery, Project prompts, command palette, local history, legacy
encoding and messages). `ExternalChange` carries the generation of its
document: the dialog shows that document's name and text, and
`resolve_external_change` shows its tab and acts on it, or drops the
conflict when the tab was closed. Tests:
`shortcuts_wait_while_a_modal_dialog_is_open`,
`a_conflict_is_resolved_on_its_own_document_whichever_tab_is_shown`.

*Evidence: test.* **Where:** `src/commands.rs` (`collect_shortcuts`, called
from `begin_frame` before the dialogs), `src/dialogs/mod.rs` (`show`,
`external_change`), `src/app/documents.rs` (`resolve_external_change`).

An egui `Modal` keeps the pointer away from the rest of the window, but
global shortcuts are read from the input before any dialog is drawn. So
every shortcut still runs while a modal dialog is open: Ctrl+Tab, Ctrl+W,
Ctrl+N, F5, Ctrl+Q. A test drew a `Modal` and pressed Ctrl+Tab, and
`NextTab` was queued.

The **File changed on disk** dialog then breaks. It describes
`state.document`, the tab that is shown now, not the file in conflict. After
Ctrl+Tab it names the other file and compares its text with the conflicting
file's disk version. **Reload from disk** and **Keep my version** reach
`resolve_external_change`, which returns early because the shown path
differs. The dialog closes, nothing is reloaded, and the conflict is
forgotten. The test confirmed this: the buffer kept its edits, and no dialog
or queued conflict was left. A later save then overwrites the other
program's change (BUG-33).

**Fix:** ignore global shortcuts while a modal dialog is open (a
`Dialogs::modal_open()` check in `collect_shortcuts`; Escape stays with the
dialog). `ExternalChange` carries the document's generation, and the dialog
and its resolution act on that document, whichever tab is shown. See
[REQ-02](#req-02--dialogs-own-the-keyboard-and-their-document).

### BUG-35 · Renaming a file only by case replaces another file

✅ **Resolved.** `rename` skips the "already exists" check only when the new
name differs by case alone *and* names the same file
(`same_file::is_same_file`: volume and file index on Windows, device and
inode elsewhere). Test: `changing_only_the_case_never_replaces_another_file`,
which takes the refusing branch where the temporary folder is case-sensitive
(checked with `TMP` in a folder made case-sensitive by `fsutil`) and the
case-changing branch elsewhere.

*Evidence: test, in a folder made case-sensitive with
`fsutil file setCaseSensitiveInfo`.* **Where:** `src/project.rs` (`rename`).

To allow `part.py` → `Part.py` on Windows and macOS, `rename` skips the
"already exists" check whenever the old and new names differ only by case.
It assumes the file system ignores case. On Linux, on case-sensitive macOS
volumes and in case-sensitive Windows folders, both files can exist. The
Project panel's **Rename…** then calls `std::fs::rename`, which replaces the
existing `Part.py`. The test created `part.py` ("lower") and `Part.py`
("UPPER, another file") and renamed the first. One file was left, holding
"lower".

**Fix:** skip the check only when the target is the same file as the
source: compare file identities (volume and file index on Windows, device
and inode elsewhere), for example with the `same-file` crate. Otherwise
refuse, as for any existing name.

### BUG-36 · A closed Editor panel never comes back

✅ **Resolved.** `ensure_editor` adds the Editor left of the 3D view when the
layout has none. `show_editor_panel` uses it, and `CqEditorApp::execute`
calls it after every action that changes the shown document (New, Open,
examples, recent files, tabs, recovered work). The saved layout goes
through `restored_layout`, which keeps the user's arrangement and adds the
Editor back. While the panel is closed, the empty view's hint ends with
*the editor is closed: View > Editor*. Tests:
`a_closed_editor_comes_back_whenever_a_file_or_a_line_is_shown`,
`a_layout_saved_without_the_editor_gets_it_back`,
`the_empty_view_says_when_the_editor_is_closed`.

*Evidence: test; seen (a session restored without the Editor panel).*
**Where:** `src/app/mod.rs` (`CqEditorApp::show_editor_panel`,
`CqEditorApp::new`), `src/panels/mod.rs` (`is_closeable`).

The Editor panel can be closed like any other, and the layout without it is
saved and restored at the next start. Opening a file, a traceback line,
*Go to Source* and *Go to Line* call `show_editor_panel`, which only brings
an **existing** Editor tab to the front. With the panel closed, the file is
opened into an editor nobody can see. The test closed the panel, opened
`a.py` and went to a line: `a.py` became the shown document and the dock
still had no Editor. The only way back is **View > Editor** or
**View > Reset Layout**, and nothing on screen hints at it. The view keeps
saying *Press F5 to render the script* about a script the user cannot see.

**Fix:** when the Editor is missing, `show_editor_panel` adds it back where
the default layout has it (left of the 3D view). **File > Open**, **New**
and the examples do the same. The saved layout is accepted only when it
has both the Editor and the 3D View; today only the 3D View is required.
See [REQ-03](#req-03--the-editor-can-never-be-lost).

### BUG-37 · STEP export of a sub-assembly loses its placement

✅ **Resolved.** The registry keeps, for each node below the top of an
assembly, its parents' world location and the color it inherits
(`RegistryEntry.parent_location`, `inherited_color`). `placed_assembly`
exports a copy placed with that location. Fixing it also showed that
CadQuery's STEP writer colors only nodes that have their own color, while
the view draws children in their parents' color: the copy gives every node
its inherited color, for whole assemblies too. The script's assembly is not
changed. Tests in `tests/worker/geometry/test_exporting.py` compare STEP
with BREP at every level and read the colors back.

*Evidence: script.* **Where:** `worker/cq_worker/exporting.py` (`export`).

When exactly one key is exported to STEP and it holds a `cq.Assembly`,
`export` calls `obj.export(path, "STEP")` on it. A key below the top of an
assembly holds the child `Assembly`, whose `loc` is relative to its parent.
The STEP file therefore places it without its parents' locations and
without the color it inherits. STL and BREP use the registry's world-space
shapes and are right. With a sub-assembly at `y = 500` inside its parent,
STEP gave `y = −5…5` and BREP `y = 495…505`. The same selection exported to
two formats ends up in two places.

**Fix:** export a wrapper assembly whose only child is the sub-assembly,
with the accumulated location and inherited color. Better, build every
STEP export as an assembly with world placements (see
[REQ-04](#req-04--exports-keep-what-the-code-built)).

### BUG-38 · Closing the tab of a paused script leaves the debugger paused

✅ **Resolved.** `Phase::run_id()` names the run whether it is running or
paused, and `running_document` uses it. Closing the paused script's tab now
reaches `Execution::abandon`, which sends *Stop* to the debugger and marks
the run stale, so what it still sends is discarded. Tests:
`closing_the_script_paused_in_the_debugger_ends_its_session`,
`a_paused_session_resumes_as_a_running_debug_session`.

*Evidence: test.* **Where:** `src/app/tabs.rs` (`forget_document`,
`running_document`), `src/app/execution.rs` (`Phase::running_id`).

`forget_document` should end a debugging session of the document being
closed. It asks `running_document()`, which uses `Phase::running_id()`, and
that is `None` while the session is **paused**. A paused session is the
usual state while debugging. Closing that tab therefore leaves the session
paused on a document that no longer exists. The test confirmed the phase
stayed `Paused` after the tab closed. The debug toolbar stays active.
**Continue** sends no breakpoints, because the editor is gone, and the run
finishes into the scene with its error line lost.

**Fix:** give `Phase` a `run_id()` that includes the paused state and use
it in `running_document`. `Execution::abandon` already handles the paused
case.

### BUG-39 · A model too large to transfer is reported as missing CadQuery

✅ **Resolved.** The reader thread reports an oversized message as
`Event::ProtocolError`, which `Worker::poll` does not treat as a failed
start. `AppState::worker_protocol_failed` explains it in a dialog (for a mesh
reply: *the model is too large to transfer: raise Deviation in
Preferences > 3D View, or show fewer objects*) and restarts the worker,
which also ends the waiting run; the model shown before stays. Tests:
`an_unreadable_message_is_not_a_failed_start`,
`a_result_too_large_to_transfer_ends_the_run_and_restarts_python`. Sending
meshes per part, so that no limit is reached at all, stays in
[REQ-09](#req-09--large-and-unusual-results-travel-safely).

*Evidence: test.* **Where:** `src/worker/mod.rs` (reader thread,
`oversized_message`, `Worker::poll`).

A reply above the transport limit (512 MiB for mesh replies, about ten
million triangles) is turned into `Message::Fatal`. `Worker::poll` treats
every `Fatal` as a failed start and sets `Problem::NoCadQuery`. The test
made the worker ready, then fed it the oversized message. The status became
`Failed` and the first-run popup says **CadQuery is not available. Install
CadQuery in that environment**, which is false. The reader then ignores
every later message while the process lives. The run never finishes, and
the view shows *Running script…* until Stop restarts Python.

**Fix:** report oversized and unreadable messages as their own event. Say
that the model is too large to transfer and suggest a coarser tolerance or
the large-model mode. Restart the worker cleanly. See
[REQ-09](#req-09--large-and-unusual-results-travel-safely).

### BUG-40 · One invalid display option loses the whole run

✅ **Resolved.** `DisplayOptions.parse` reads each option on its own, so an
invalid one no longer drops the others, and refuses NaN and infinities
(`finite`) with the usual warning. Fixing it showed that a NaN placement
(`moved(cq.Location((math.nan, 0, 0)))`) broke the reply the same way:
`SceneBuilder` now checks every node with `non_finite`, and leaves out an
object that holds a NaN, releasing its keys and naming it in the log
(*Cannot display lost: parts[0].mesh.bbox[0][0] is not a finite number*).
Tests: `test_non_finite_opacity_is_ignored`,
`test_one_invalid_option_keeps_the_others`,
`test_a_non_finite_number_never_loses_the_whole_run`.

*Evidence: script.* **Where:** `worker/cq_worker/scene.py`
(`DisplayOptions.parse`), `worker/cq_worker/protocol.py` (`Channel.send`,
`allow_nan=False`).

`show_object(part, options={"alpha": math.nan})` passes through
`DisplayOptions.parse`, because `min(max(nan, 0), 1)` is `nan`. The
`run_finished` reply then cannot be encoded. The whole run, every valid
object included, is replaced by the error *ValueError: Out of range float
values are not JSON compliant*, which does not name the object or the
option. The same holds for a NaN alpha given in `rgba`.

**Fix:** `DisplayOptions.parse` rejects non-finite numbers with the existing
"Ignoring invalid display option" warning. A node that cannot be encoded is
dropped and named in the log instead of failing the reply.

### BUG-41 · The local history depends on how a path is written

✅ **Resolved.** *Evidence: test (found while fixing BUG-33:
`renaming_or_deleting_files_keeps_the_tabs_in_step` failed on Windows on the
unchanged tree).* **Where:** `src/recovery.rs` (`document_folder`).

A document's history folder was named after a hash of its path's raw
bytes. On Windows `C:/x/part.py` and `C:\x\part.py` are the same file but
got two folders, so versions saved under one spelling were missing from
**File > Local History…** under the other. A path that went through a
rename (`project::relocated` joins with `\`) and a path typed with `/` on
the command line could thus lose sight of each other's history.

**Resolution:** the path is rebuilt from its components before it is
hashed, so `/` and `\` (and `.` components) give one folder. Paths written
with `\`, which the system's file dialogs return, keep the folders of
earlier releases. Tests: `a_file_has_one_history_however_its_path_is_written`,
`folder_names_are_stable_across_releases`.

---

## 2. Required improvements

These follow from what CQ-editor is. A code-CAD IDE must not lose files, it
must hand over correct CAD data, and it must work on supported platforms.
Each item says **why it is required**, **what changes** and **how it helps
the code**.

| Status | Id | Improvement | Fixes | Priority |
| :---: | --- | --- | --- | --- |
| ✅ | [REQ-01](#req-01--saving-checks-the-disk-first) | Saving checks the disk first | BUG-33 | P1 |
| ✅ | [REQ-02](#req-02--dialogs-own-the-keyboard-and-their-document) | Dialogs own the keyboard and their document | BUG-34 | P1 |
| ✅ | [REQ-03](#req-03--the-editor-can-never-be-lost) | The editor can never be lost | BUG-36 | P2 |
| ✅ | [REQ-04](#req-04--exports-keep-what-the-code-built) | Exports keep what the code built | BUG-37 | P1 |
| ✅ | [REQ-05](#req-05--units-reach-the-exported-files) | Units reach the exported files | | P2 |
| ✅ | [REQ-06](#req-06--geometry-is-validated-before-it-leaves) | Geometry is validated before it leaves | | P2 |
| ✅ | [REQ-07](#req-07--show-everything-the-cadquery-api-can-specify) | Show everything the CadQuery API can specify | | P2 |
| ✅ | [REQ-08](#req-08--debugging-and-profiling-cover-the-whole-project) | Debugging and profiling cover the whole project | BUG-38 | P2 |
| ✅ | [REQ-09](#req-09--large-and-unusual-results-travel-safely) | Large and unusual results travel safely | BUG-39, BUG-40 | P2 |
| ✅ | [REQ-10](#req-10--headless-build-and-check) | Headless build and check | | P2 |
| ✅ | [REQ-11](#req-11--a-supported-python-window) | A supported Python window | | P1 |
| ✅ | [REQ-12](#req-12--signed-installers) | Signed installers | | P2 |
| ✅ | [REQ-13](#req-13--tests-at-the-seams) | Tests at the seams | all | P2 |

### REQ-01 · Saving checks the disk first

**Why it is required.** Scripts live in folders shared with other tools:
git, other editors, formatters, sync clients. An editor that writes without
looking destroys their work (BUG-33). Principle 4 promises it never happens.

**What changes.** `Document` keeps a disk fingerprint (length, modification
time, content hash) from load and save. `save` compares it with the disk.
On a difference, save does not write. It shows the conflict dialog the
watcher already uses, and archives the disk version before any overwrite.
The same check runs when the window gets focus back, even with automatic
reload off. That is how other editors notice changes. A tab whose file was
deleted or moved says so in its title.

**How it helps the code.** One conflict path (`ExternalChange`) for the
watcher, save and focus, instead of a protection that exists only while
autoreload is on. `Document::save` gets a precondition the tests can state.

**Progress.** Done. Save compares the disk with the saved baseline. Regaining
focus checks every open tab even with automatic reload off: clean files are
reloaded, conflicts use the same dialog as the watcher, and a missing file
is labeled in its tab and title while its buffer is preserved. The baseline
also records length, modification time and a hash of the original bytes.

### REQ-02 · Dialogs own the keyboard and their document

**Why it is required.** A confirmation is only worth something if nothing
else happens while it is asked. Today keys act behind every modal (BUG-34).

**What changes.** `Dialogs::modal_open()` gates `collect_shortcuts`. Each
dialog that concerns a document stores its generation and acts on that
document: external change, unsaved changes, local history. The dialog
shows the name of that document, not of the shown tab.

**How it helps the code.** Dialog outcomes stop depending on the shown tab.
That removes a class of "the user switched tabs meanwhile" bugs that tabs
(FEAT-D1) made possible.

**Progress.** Done. Modal dialogs gate global shortcuts. External-change,
unsaved-changes and local-history actions carry document generations; history
restores the tab that opened it, and refuses a version of another path.

### REQ-03 · The editor can never be lost

**Why it is required.** In a code-CAD IDE the code is the model, and the
editor is where it is written. A layout without it is not a valid state of
the program (BUG-36).

**What changes.** The saved layout is validated to contain the Editor and
the 3D View. Everything that shows a file or a line re-adds the Editor when
it is missing. Closing the Editor panel can stay possible, to work with an
external editor and autoreload. The view's empty state then says
*The editor is hidden — View > Editor*.

**How it helps the code.** One `ensure_panel(Tab)` replaces the ad-hoc
`show_editor_panel` and `show_project_panel`, and covers the Traceback and
Console panels the same way when an action needs them.

**Progress.** Done. `ensure_panel` restores the Editor, Project, Console,
Traceback or another panel when an action needs it. Restored layouts always
have an Editor and a 3D View. Opening a document shows the Editor, console
actions show the Console, and a new run error shows Traceback.

### REQ-04 · Exports keep what the code built

**Why it is required.** For a CAD tool, the exported file is the product.
Today, exporting several shown objects to STEP fuses them into one
anonymous compound. Checked by script: objects shown as `base` (steelblue)
and `pin` (red) come back from the STEP file as `1` and `2` without colors.
A sub-assembly loses its placement (BUG-37). Downstream tools (CAM, PDM,
other CAD) need the parts, names, colors and hierarchy.

**What changes.**

* STEP is written as an assembly made of what is exported: one child per
  shown object or assembly node, with its name, color and world placement.
  `merge_assemblies` and STEP's `fused` mode are offered as options, not
  imposed.
* An **Export dialog** replaces the bare file picker: format, STEP mode and
  schema (AP214 or AP242), STL ASCII or binary and tolerances, and what to
  export (selection, visible objects or everything).
* The formats CadQuery 2.7 already writes, without new dependencies:
  **3MF** (units and colors, the printing format), **glTF/GLB** (web,
  rendering), **AMF**, **VRML**, **VTK.js**, OCAF **XBF** (lossless,
  reopens with names and colors), **SVG** (a hidden-line projection) and
  **DXF** (a section or planar faces). This closes the former FEAT-D3.

**How it helps the code.** `exporting.py` gets one path built on
`cq.Assembly` instead of two (fused compound, or a single assembly). The
registry's world placements, already used by STL and BREP, become the
single source of placement for every format.

**Progress.** Done. STEP exports multiple shown objects as named, colored
assembly children with world placements. The Export dialog chooses scope,
format, STEP mode and schema, STL encoding and mesh tolerances. The worker
also exports 3MF (with per-part names and display colors), glTF/GLB, AMF,
VRML, VTK.js, XBF, SVG and DXF. A VTK.js exporter that returns an empty
archive is reported as a failure instead of a false success.

### REQ-05 · Units reach the exported files

**Why it is required.** *Preferences > 3D View > Units* labels the grid and
the scale bar (for example in inches). Every STEP file is still written in
millimeters, because neither CadQuery nor the editor sets
`write.step.unit`. A part modeled and checked in inches is imported 25.4
times too small. The documentation says Units "only names them". In a CAD
program that is a trap, not a feature.

**What changes.** Exports write the chosen unit where the format has one:
STEP (`write.step.unit`), 3MF (`unit`) and glTF (a scale to meters). STL
and DXF have no unit, so the export dialog states it. With *Units: none*,
the dialog asks.

**How it helps the code.** Units become a setting the worker receives with
every export (`SceneSettings`), not a label known only to the view.

**Progress.** Done. The Export dialog requires a model unit, defaulting to
the 3D View preference. STEP sets the OCCT output unit and restores its
previous setting afterward; 3MF records the unit in the model XML; glTF/GLB
scale coordinates to meters. STL and DXF show that their formats carry no
unit metadata. Tests read the resulting STEP, 3MF and glTF files back.

### REQ-06 · Geometry is validated before it leaves

**Why it is required.** OpenCascade operations can produce invalid solids
(self-intersections, open shells, bad tolerances) that still display.
Exported to STL or STEP, they break slicers and CAM silently. The user only
learns about it far away from the code that caused it.

**What changes.** Exports run `BRepCheck_Analyzer` (`Shape.isValid()`) on
what they write. They warn with the object's name, and offer to export
anyway or to try `ShapeFix` (`Shape.fix()`). The Objects panel gets an
on-demand **Check geometry**, which also reports open shells, non-manifold
edges and solids that are not closed. A badge marks invalid objects.

**How it helps the code.** The worker gains a `check` request that
measurement, export and CI ([REQ-10](#req-10--headless-build-and-check))
share.

**Progress.** Done. The worker checks each exported shape for OCCT validity,
open shells and non-manifold edges before opening an output file. The user
can cancel, export anyway, or try ShapeFix; an unsuccessful repair is
refused. **Check Geometry** in Objects and Tools uses the same request and
marks invalid nodes until their scene is replaced. The protocol is version 8.

### REQ-07 · Show everything the CadQuery API can specify

**Why it is required.** Principle 1: the view shows what the code says.
CadQuery 2.7 lets scripts name and color individual faces
(`Assembly.addSubshape(face, name=, color=, layer=)`) and attach materials
(`cq.Material`, with a density and a visual material). Checked by script:
a red face named "top face" is drawn in the part's gray, and its name
exists nowhere in the scene. Yet a STEP export of the same assembly keeps
both. The view and the file disagree.

**What changes.** Scene parts carry per-face colors, as a color per
triangle group, which the face ids already provide, and per-face names.
Picking reports *top face* instead of *Face 6*, and selectors and *Copy as
code* can use the name. Materials give the part its visual color and their
density to measurements (see [SUG-10](#3-suggested-improvements)).

**How it helps the code.** `MeshTopology` gains attributes per face rather
than one color per part. The same structure later carries PMI and
inspection highlights.

**Progress.** Done. Assembly face names, layers and colors travel with each
part instance, independent of the shared mesh. The viewer shades named faces
with their own color, and picking and measurements show the name and layer.
Copy as code uses a unique assembly face name when available. Assembly
materials carry their physical density to exact mass measurements, and their
visual base color to the part when specified. The protocol is version 9.

### REQ-08 · Debugging and profiling cover the whole project

**Why it is required.** Projects with several files are first-class
(FEAT-D1), but the debugger traces only the editor buffer. Breakpoints in
an imported module can be set in its tab's gutter and are silently
ignored. The user guide admits it ("breakpoints in modules are ignored").
Closing a paused script's tab does not end the session (BUG-38).

**What changes.** The debugger traces files of the project folder too, with
the breakpoints of their open tabs (`Request::Debug` carries breakpoints
per path). Pausing in a module shows its tab at the line. The profiler
reports module lines in their tabs' gutters. A paused session ends when its
script closes.

**How it helps the code.** Breakpoints become per document instead of "the
run target's", which removes the special case that BUG-38 slipped through.

**Progress.** Done. Run and resume requests carry open-tab breakpoints by
path. The worker traces project modules, reports the paused file, and the
editor opens that module at its paused line. Profiling records file paths
and source snapshots so module tabs receive line timings. Closing the run
script or paused module ends debugging. The protocol is version 12.

### REQ-09 · Large and unusual results travel safely

**Why it is required.** Large assemblies are where a CAD viewer is judged.
Today a reply above 512 MiB is fatal and misreported (BUG-39). One bad
number loses a whole run (BUG-40).

**What changes.** Mesh replies are sent per part, as `run_part` messages
followed by `run_finished`, so no single line grows with the model. The GUI
decodes parts as they arrive and can draw them progressively. The size
limit then applies per part and can be explained. Protocol encoding errors
name the object they belong to. Worker results are checked for finite
numbers at the source.

**How it helps the code.** Bounded memory on both sides of the pipe, and an
error model in which "the transport failed" and "CadQuery is missing" are
different things.

### REQ-10 · Headless build and check

**Why it is required.** The main promise of code-CAD over GUI CAD is
reproducibility: the same script gives the same part, on any machine, in
CI. That needs a way to build without a window. This is the former
FEAT-D2.

**What changes.**
`cq-editor build part.py --export out.step --set width=40` runs the worker
without the GUI, with the same settings and `show_object` semantics, and
writes any format of REQ-04. `cq-editor check part.py` exits non-zero on an
error or invalid geometry ([REQ-06](#req-06--geometry-is-validated-before-it-leaves))
and prints the traceback as text or JSON. Parameter names follow `cq-cli`
and `cqgi`, so scripts can move between tools.

**How it helps the code.** It separates the run logic from egui (the
`Execution` state machine and the worker client already are), which also
makes [REQ-13](#req-13--tests-at-the-seams) easier.

### REQ-11 · A supported Python window

**Why it is required.** Python 3.10 reached its end of life in October 2026
(PEP 619). At the time of the review, the worker declared
`requires-python = ">=3.10"`, a CI lane ran 3.10, and the development
environment of this repository was 3.10.
Libraries following the scientific Python support schedule (SPEC 0) have
already dropped it.

**What changes.** A written support policy: the Python versions still
receiving security fixes, and the last two CadQuery minor releases with
their OCP. `requires-python` moves to `>=3.11`, and the CadQuery 2.7 lane
moves to 3.11. The development setup described in the docs moves too. The
installer already bundles Python 3.12.

**How it helps the code.** Newer syntax (`except*`, `tomllib`,
`contextlib.chdir`) and
fewer compatibility shims.

### REQ-12 · Signed installers

**Why it is required.** At the time of the review, the release workflow built
the Windows `.exe` and macOS `.pkg` without signing them. Windows SmartScreen
and macOS Gatekeeper
warn about, or block, unsigned installers. A tool that runs code on the
user's machine has to be verifiable. Releases already publish
`SHA256SUMS.txt` and each installer's resolved lockfile; a checksum proves a
download intact, not who made it.

**What changes.** Authenticode signing on Windows, and Developer ID signing
and notarization on macOS, in `release.yml`, with the certificates kept as
repository secrets.

**How it helps the code.** Releases get a known origin that the operating
systems check by themselves, and the update check
([FEAT-E5](#completed)) leads to downloads that install without warnings.

### REQ-13 · Tests at the seams

**Why it is required.** The bugs above sit at seams that needed direct
coverage: keyboard focus under dialogs, the dock layout, the file
system shared with other programs, the pipe between the processes, tabs
combined with the debugger, and exported files read back by another
program. These flows can be tested without a native window.

**Implemented.** Each resolved bug above names its regression test. The
headless egui harness draws a modal and a restored dock, then checks keyboard
actions and visible panels. Rust and Python read the same golden protocol
messages for binary parts, geometry references, non-finite values and an
oversized control reply. CI runs mypy over every worker module, Ruff and the
worker suite with branch coverage; an in-process CLI test keeps terminal
dispatch inside the coverage measurement.

**What changes.**

* A regression test per bug of section 1, starting from the probe
  described in each.
* Headless UI tests of dialogs, the dock and keyboard flows, for example
  with `egui_kittest` (former ENG-03).
* Golden protocol messages read by both the Rust and the Python suites
  (former ENG-02), including oversized and non-finite cases.
* `ruff`, a type checker and coverage for the worker (former ENG-01).

**How it helps the code.** Each seam gets a harness, so the next change
there is checked by a test instead of found by a review.

---

## 3. Suggested improvements

Worth doing, not required. Labels: 🟢 mostly for beginners · 🔵 mostly for
advanced users · ⚪ both.

### Learning and writing code

| Status | Id | Suggestion | Why it fits |
| :---: | --- | --- | --- |
| ✅ | SUG-01 🟢 | **Friendly kernel errors** (former FEAT-A3): identify the CAD operation and script line behind common kernel and selector errors, then offer possible causes | The traceback remains intact; hints are qualified rather than presented as proven geometric facts |
| ✅ | SUG-02 🟢 | **Selector preview** (former FEAT-A4): with the caret on a literal call such as `part.faces(">Z")`, matching entities light up in the view | Uses the last run's displayed variable and exact topology; moving the caret never evaluates dynamic Python expressions |
| | SUG-03 🟢 | **Start page** (former FEAT-A1) with templates, recent files and projects, and an examples gallery with thumbnails | Every template is code; thumbnails come from the screenshot code (FEAT-C5) |
| | SUG-04 🟢 | **Snippets** (former FEAT-A5) for frequent patterns, including the free-function API (`cadquery.func`) and `cq.Sketch` | Inserts ordinary code |
| | SUG-05 🟢 | **Guided lessons** (former FEAT-A2): steps that each explain one concept and check the result, such as the expected volume | Learning by editing and running real code |
| | SUG-06 ⚪ | **Translations** (former FEAT-A6), starting with Portuguese and Spanish | Lowers the entry barrier; the API stays English |
| ✅ | SUG-07 🔵 | **Outline and go to definition**: script symbols in a panel, F12 for definitions and Ctrl+Shift+F12 for project references through Jedi | Static navigation in multi-file projects; outline works without Jedi |
| | SUG-08 🟢 | **Warn when `show_object` reuses a name**: in a loop such as `for i in …: part = …; show_object(part)`, each object replaces the previous one, as documented and as in the original editor. A log line could suggest `name=f"part{i}"` | Explains a frequent surprise without breaking compatibility |

### Code and model

| Status | Id | Suggestion | Why it fits |
| :---: | --- | --- | --- |
| | SUG-09 ⚪ | **Typed parameters**: booleans as checkboxes, strings and choices as drop-downs, and `describe_parameter()` descriptions, compatible with CadQuery's `cqgi` | `describe_parameter` raises `NameError` today; `cq-cli` and model servers use it |
| | SUG-10 ⚪ | **Mass properties** in Measurements: volume, mass from the material density, center of mass and inertia (`BRepGProp`) | Engineering answers from the code's own data |
| | SUG-11 🔵 | **Clearance and interference** between parts of an assembly (`BRepExtrema`, `BRepAlgoAPI_Common`), listed and highlighted | The most common assembly question |
| | SUG-12 ⚪ | **Inspector completeness**: show the root workplane's objects (`Workplane(obj=…)`) and the steps of a `Sketch` | Today the chain starts at the first operation |

### View, projects and running

| Status | Id | Suggestion | Why it fits |
| :---: | --- | --- | --- |
| | SUG-13 ⚪ | Section plane aligned to a picked face, and more than one plane | Extends FEAT-C1 |
| | SUG-14 ⚪ | Dimensions kept in the view: measurements pinned as annotations, included in screenshots | Extends FEAT-B5 and FEAT-C5 |
| | SUG-15 ⚪ | **Import by drag and drop** (former FEAT-D4): a STEP, STL or DXF file dropped on the editor inserts the matching `cq.importers…` or `cq.Assembly.importStep` line | Writes code |
| | SUG-16 🔵 | **Git awareness** (former FEAT-D5): changed-lines markers and a diff against the last commit | The conflict dialog's comparison view is a start |
| | SUG-17 🔵 | **Run selection and cells** (former FEAT-D6): run the selected lines, or a `# %%` cell, in the console | Fast experiments on the last run's variables |
| | SUG-18 ⚪ | Local history follows renames (today the versions stay in the folder of the old path) | Completes FEAT-D1's rename handling |
| | SUG-19 🔵 | Project panel for large folders: build rows only when a listing changes, not every frame, and honor `.gitignore` | Keeps the panel cheap in big repositories |

### Extensibility and engineering

| Status | Id | Suggestion |
| :---: | --- | --- |
| | SUG-20 🔵 | **Plugin API** (former FEAT-F2): Python plugins loaded by the worker that register exporters, `show_object` handlers or commands |
| | SUG-21 🔵 | **User themes** (former FEAT-F3): custom syntax and viewer colors |
| | SUG-22 | Benchmarks of tessellation, decoding and picking tracked over time (former ENG-04) |
| | SUG-23 | Accessibility review (former ENG-05): keyboard-only use, AccessKit screen readers, contrast of both themes |

---

## 4. Direction

Code-CAD has one decisive advantage over GUI CAD: a design is a program. It
can be versioned, parameterized, tested, generated and reused. CQ-editor
already turns the view into a way to *write* code (selectors, parameters,
measurements as code). The directions below follow that line. Each one
lists what it builds on, in this code and in the libraries, and a first
step small enough to start with.

### NORTH-01 · Direct manipulation that writes code

Drag a face or a dimension handle in the view, and the editor changes the
parameter that drives it, then renders. The code stays the only source of
truth (principle 1), and the view becomes an input device for it.

* **Builds on:** parameters (FEAT-B3), provenance of each object's line
  (FEAT-B4), selectors (FEAT-B1), exact picking.
* **How:** for a picked face, re-run with each parameter nudged by a small
  amount and see which parameters move that face and how. That gives the
  handle's direction and scale without any symbolic analysis.
* **First step:** a *Which parameters move this?* command on a picked face.

### NORTH-02 · Incremental execution

Re-run only what an edit changed. A `Workplane` chain is a graph of
immutable steps, and the worker already hashes geometry by content. Caching
operation results by their inputs (the source line, the parameters, the
input shapes' ids) would let a late edit skip early booleans. Live preview
could then follow typing, even on heavy models.

* **Builds on:** `GeometryCache` and content ids, provenance recording of
  `Workplane.newObject`, the profiler's timing per line and operation.
* **First step:** profile-guided memoization of the slowest operations
  (FEAT-E2 already knows which ones they are), behind a preference.

### NORTH-03 · Geometric diff and history

Compare two versions of a model: two runs, two local-history versions or
two git commits. Show added and removed material
(`BRepAlgoAPI_Cut` in both directions), changed faces, and how volume, mass
and bounding box changed.

* **Builds on:** local history, the conflict dialog's comparison view,
  measurements, SUG-16.
* **First step:** *Compare with the last run* in the Objects panel.

### NORTH-04 · Test-driven CAD

Design rules as code: `expect(part).volume_between(…)`,
`.clearance(a, b) >= 0.2`, `.min_wall(1.2)`, `.no_overhang_beyond(45)`.
They are shown as a *Checks* panel after every run and run headless in CI
([REQ-10](#req-10--headless-build-and-check)). A failing check points to the
code line and highlights the geometry.

* **Library support:** `BRepExtrema` (distances, wall thickness by
  sampling), `BRepGProp` (mass properties), `BRepCheck` (validity), face
  normals for overhang and draft analysis.
* **First step:** a small `cq_editor.checks` module injected like
  `show_object`, with three checks and a panel.

### NORTH-05 · Drawings and model-based definition from code

Technical drawings made by code: views made with `HLRBRep` (hidden-line
projection, already used by CadQuery's SVG exporter), dimensions from
measurements, sheets exported to SVG, DXF and PDF. Then the same dimensions
and tolerances written into STEP AP242 as PMI (`XCAFDimTolObjects` is in
OCP), so the 3D file carries the manufacturing information.

* **First step:** a *Drawing* tab with front, top and side views of the
  selection, exported to SVG.

### NORTH-06 · Assemblies as mechanisms

Show the constraints of `Assembly.constrain` (which faces and points, the
kind) and the solver's residuals. Animate joints by driving a parameter
over time, detect interference along the motion (SUG-11), and produce a
bill of materials with mass from materials.

* **Library support:** CadQuery's assembly solver (casadi), `cq.Material`,
  `BRepExtrema`.
* **First step:** list constraints under each assembly in the Objects
  panel and highlight their entities on hover.

### NORTH-07 · Design-space exploration and optimization

Sweep parameters over a grid and show the variants side by side, with
their measured properties. Then optimize: minimize mass under a clearance
or stiffness constraint, with nlopt, already installed as a CadQuery
dependency. Variants are saved as named parameter sets of the script.

* **Builds on:** parameters, headless runs (REQ-10), checks (NORTH-04).
* **First step:** *Sweep parameter…* producing a table and thumbnails.

### NORTH-08 · A sketch workbench that writes `cq.Sketch`

Draw 2D profiles with constraints in the view. The workbench writes them as
`cq.Sketch` code with `.constrain()` calls and solves them with CadQuery's
sketch solver. Editing the code updates the drawing.

* **First step:** show a `Sketch`'s constraints and their status (solved,
  over- or under-constrained) when it is selected.

### NORTH-09 · An AI design partner that works through code

An assistant that turns intent into CadQuery code, reads tracebacks and
measurements, proposes selectors and fixes, and checks its own proposals by
running them in the worker. The loop is code → model → checks → code, so
the result is always ordinary, reviewable code (principle 1). It can use the
same tools the GUI uses: run, measure, selectors and checks (NORTH-04). It
is optional, with explicit consent before any code or model leaves the
machine.

* **First step:** *Explain this error* and *Write a selector for this face*,
  each answered as a code edit the user accepts or rejects.

### NORTH-10 · The OCP ecosystem

Show any object that wraps a `TopoDS_Shape` (`.wrapped`), so build123d,
`cq_warehouse` parts and OCP-based libraries work with `show_object`
(former FEAT-F1). Offer part libraries through the environment manager
(FEAT-E3), and a plugin API (SUG-20) for exporters and commands.

* **First step:** accept `.wrapped` in `to_compound` and test with build123d.

---

## 5. Milestones

| Milestone | Goal | Contents |
| --- | --- | --- |
| **0.8.x: safe files** | Nothing is lost or mislabeled | BUG-33…BUG-40, REQ-01, REQ-02, REQ-03, REQ-11, regression tests of REQ-13 |
| **0.9: correct output** | What leaves the editor is right | REQ-04, REQ-05, REQ-06, REQ-07, REQ-09, REQ-12 |
| **1.0: projects and automation** | Comfortable for large designs and CI | REQ-08, REQ-10, the rest of REQ-13, SUG-01, SUG-02, SUG-09, NORTH-10 |
| **Later** | Leading code-CAD | The other suggestions, NORTH-01…NORTH-09 |

Bugs come before features. The order inside a milestone is flexible.

---

## Completed

What the previous roadmap marked as resolved; its open items moved into
the sections above. The ids are kept because code comments and the
changelog refer to them.

| Ids | What |
| --- | --- |
| BUG-01…BUG-32 | Bugs of the rewrite and of the October 2026 reviews (object keys across restarts, breakpoints following edits, worker events while minimized, recovery after *Don't save*, the debugger's `DebugStop`, bounded output, mesh ids per mesher…); see the [changelog](changelog.md) |
| PERF-01…PERF-04 | Validated face mapping for picking, glTF scratch reuse, two-phase BVH picking, exact measurements from live geometry |
| FEAT-B1…B5 | Selection to code, `selected()` in the console, Parameters panel, *Go to Source*, measurements and *Copy as code* |
| FEAT-C1…C8 | Section view, exploded view and isolation, X-ray and hidden-line modes, named views, high-resolution screenshots and turntables, large-model mode, ground grid and scale bar, stereo modes |
| FEAT-D1 | Tabs and the Project panel |
| FEAT-E1…E6 | Gentle stop, per-line profiling, environment manager, self-contained installers, update check, rich console output |
| ENG-06 | `src/app/` split by concern around the `Execution` state machine |
