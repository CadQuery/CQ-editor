# Projects and multiple files

A CadQuery model often grows into several files: a script that assembles the
model and modules for its parts. CQ-editor keeps every file you open in a
**tab** of the editor, and shows a folder's files in the **Project** panel.
The code stays the source of truth: tabs and the panel only help you move
between the files and run the right one.

```
gearbox/
├── main.py          ▶ the main script: F5 renders it
├── parts/
│   ├── __init__.py
│   ├── axle.py
│   └── plate.py
└── README.md
```

## Tabs

Every file you open (**File > Open…**, **Open Recent**, the Project panel, a
traceback row) gets a tab; opening a file that is already open shows its
tab. **New** (Ctrl+N), the examples and recovered work open in tabs too. The
untitled script shown when CQ-editor starts gives way to the first file you
open, as long as you did not edit it.

The tab strip appears above the editor once two or more files are open:

* click a tab to show it; **Ctrl+Tab** / **Ctrl+Shift+Tab** (or
  **Ctrl+PageDown** / **Ctrl+PageUp**) go to the next or previous tab;
* a dot after the name marks unsaved changes, and ▶ the project's main
  script;
* **×**, the middle mouse button or **Ctrl+W** closes a tab. Closing the
  last one leaves an empty untitled script;
* right-click a tab for **Close**, **Close Others**, **Close All**, **Set
  as Main Script** and **Copy Path**;
* two files with the same name show their folder, for example
  *part.py — left* and *part.py — right*.

Each tab keeps its own state while another one is shown: the caret and the
scroll position, the undo history, breakpoints, the error line, the times of
a profiled run and the search bar.

### Saving and closing

**Save** and **Save As…** work on the shown tab; **File > Save All** saves
every tab with unsaved changes (an untitled one asks for a location).
Closing a tab with unsaved changes asks **Save**, **Don't save** or
**Cancel**. Closing the window or **Quit** asks once for all of them, with
the list of files, and **Save all** saves each in turn: if you cancel the
location of an untitled one, nothing is closed.

### The next session

CQ-editor reopens the tabs of the last session in their order and shows the
file that was shown. Files deleted meanwhile are skipped. A file given on
the command line is opened as well, and shown. The project folder is opened
again too.

## The Project panel

**File > Open Folder…** (Ctrl+Shift+O) shows a folder in the **Project**
panel, docked to the left of the editor. **Open Recent** lists recent
folders below recent files, and **File > Close Folder** (or the × of the
panel) closes it. The panel can be moved like any other (see
[the interface](interface.md#panels)) and shown again from the **View**
menu.

* Folders come first, then files, sorted by name. Hidden files and folders
  (`.git`, `.venv`), `__pycache__`, compiled files and `*.egg-info` are not
  listed.
* Click a folder to open or close it, and a script or text file (`.py`,
  `.md`, `.txt`, `.json`, `.toml`, `.yaml`, …) to open it in a tab. Other
  files are grayed out.
* The file shown in the editor is highlighted, and open files with unsaved
  changes have a dot.
* Files created, renamed or deleted by other programs show up by
  themselves as you work in the window; the ⟳ button reads the folder
  again at once.
* The open folders are remembered per project.

Right-click a file or folder, or the empty space below the tree, for:

| Command | What it does |
| --- | --- |
| **New File…** | Creates an empty file in the folder and opens it. Without an extension, the name gets `.py` |
| **New Folder…** | Creates a folder |
| **Rename…** | Renames in place. Open tabs follow the new name and keep unsaved edits; breakpoints, named views, recent files and the main script follow too. An existing name is refused, also when it differs only by case in a folder where case matters; changing just the case of a name works everywhere |
| **Delete…** | Deletes the file, or the folder and everything in it, after a confirmation that names open files with unsaved changes. Their tabs close; their last contents and a deleted file stay in the [local history](editor.md#crash-recovery-and-local-history) |
| **Set as Main Script** | Makes the script the one F5 renders (below) |
| **Copy Path** | Copies the full path |
| **Show in File Manager** | Opens the folder in the system's file manager |

Names are checked before anything is created: an existing file is never
replaced, and names that some systems refuse (`a:b`, `con.py`, a trailing
dot) are refused everywhere, so the project can move between computers.

## The main script

A project's **main script** is the one that builds the model. While a file
of the project is shown, **F5** (and Profile and Debug) runs the main
script, not the file shown, so you can edit a part module and press F5 to
see the whole model. A file outside the project, an untitled script or an
example still renders itself.

* When a folder is opened for the first time, CQ-editor picks `main.py`,
  `__main__.py` or a script named after the folder, else the only script at
  the top of the folder that calls `show_object`. When that is not clear,
  there is no main script and F5 renders the file shown.
* **Set as Main Script** in the Project panel, on a tab or in the **Run**
  menu chooses another one; **Clear Main Script** goes back to rendering
  the file shown. The choice is remembered per folder.
* The main script has ▶ in the tree and on its tab. While another file is
  shown, the status bar shows ▶ and its name, and so does the tooltip of
  the **Render** button.
* The main script opens in a tab, without being shown, the first time F5
  runs it. Its error line, the debugger's current line and profile times go
  to its tab; the Parameters panel shows its parameters.

Python imports the **saved** version of a module. When an open module next
to the script has unsaved changes, the status bar says so after the render:
save it (or **Save All**) and render again. With *Reload imported modules*
on (the default), saved edits to modules apply on the next run.

## Errors and debugging across files

* The **Traceback** panel names the script that ran. A frame in the script
  goes to its line, in its tab. A frame in another Python file, such as a
  module of the project, opens that file at the line.
* A debugging session pauses in the main script and shows its tab. Only
  the script itself is stepped through: breakpoints in modules are ignored
  (see [Debugging](debugging.md)).
* **Go to Source** on an object shows the line of the script that created
  it, in its tab.

## Automatic reload

With **File > Automatic Reload and Preview** on, CQ-editor watches the
script F5 runs (the main script in a project), its imported modules
(*Autoreload: watch imported modules*) and every file open in a tab:

* a file changed by another program is reloaded in its tab, unless the tab
  has unsaved edits that differ: then the **File changed on disk** dialog
  shows that tab and asks (see [the editor](editor.md#when-the-file-changes-while-you-have-unsaved-edits));
  several conflicts are asked about one after the other;
* a change to the script or to one of its modules renders it again.

## Crash recovery

Every tab with unsaved changes is part of the recovery checkpoint. After a
crash, **Unsaved work can be recovered** lists the files; **Restore
session** puts each back in the tab of its file, or in a new tab for an
untitled script.
