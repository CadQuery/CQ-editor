# Console, variables and logs

## Console

The **Console** panel is an interactive Python prompt running in the same
Python process as your scripts.

* After each run it contains the script's **variables**, so you can query
  the model without editing the script:

  ```python
  >>> bracket.val().Volume()
  10944.0…
  >>> bracket.faces(">Z").val().Center()
  ```

* It offers the script API: `show_object` (or its alias `show`), `debug`,
  `log`, `rand_color` and `cq`. Objects shown from the console are **added**
  to the view, next to the script's objects:

  ```python
  >>> show_object(cq.Workplane().sphere(5), name="ball", color="tomato")
  ```

* `selected()` returns what is picked in the 3D view, for measurements and
  experiments: the picked faces, edges and vertices as `cq.Face`,
  `cq.Edge` and `cq.Vertex`, in the order they were picked and placed as
  shown (an assembly part in the assembly's coordinates). When no entity is
  picked, it returns the shapes of the selected objects (a shown workplane
  holding one solid gives that `cq.Solid`), and an empty list when nothing
  is selected:

  ```python
  >>> top, hole = selected()        # a face and an edge, Ctrl+clicked
  >>> top.Area(), hole.radius()
  >>> top.distance(hole)
  >>> selected()[0].Volume()        # with an object selected instead
  ```

  The selection is read when `selected()` runs. Picks from before a Python
  restart, or of a model that changed since, raise a `LookupError` that
  says so.

* `print` output and errors appear in the console itself.

* **Rich results**: a value with a picture shows it, as in Jupyter. A
  matplotlib figure, or any object with `_repr_png_` or `_repr_jpeg_`
  (PIL images, for example), is drawn in the console, scaled to fit, above
  its text. `display(a, b, …)` shows each value the same way, also from
  inside a loop or a function:

  ```python
  >>> import matplotlib.pyplot as plt
  >>> fig, ax = plt.subplots()
  >>> ax.plot(sizes, volumes)
  >>> fig                           # the plot appears in the console
  >>> display(fig, "done")
  ```

  Only PNG and JPEG images are shown; HTML, Markdown, SVG and LaTeX show
  their plain text. An image over 4 MB or 4096 pixels, or beyond the 20th
  of one command, is replaced by a note. Each command may produce up to 100
  rich results. The console keeps its newest 50 images within a shared memory
  budget, replacing older images with a note when necessary.

| Key | Action |
| --- | --- |
| Enter | Run the input |
| Shift+Enter | New line (for multi-line statements such as `for` loops) |
| ↑ / ↓ | Browse the input history (single-line input) |
| Tab | Complete the name before the cursor (needs `jedi`) |

An incomplete statement (an open `for:` block, for example) waits for more
input, as in the standard Python prompt.

The buttons next to the input clear the console (**Run > Clear Console**
does the same) and restart it with a fresh namespace. While a command runs,
a Stop button appears there too: like **Run > Stop**, it interrupts the
command (`KeyboardInterrupt`) and keeps the namespace, unless the command does
not stop within 3 seconds and Python has to be restarted. **Run > Restart
Python** always resets the console, since it starts a new Python process.

## Variables

The **Variables** panel lists name, type and value of every public variable
after a run, or of the local variables of the paused frame while debugging.

* Type in the filter box to search by name or type.
* The **Value** column takes the width that **Name** and **Type** leave; drag
  the column borders to resize them.
* **Click a row** to show its whole value below the table, where it wraps and
  can be selected or copied; × closes it. Right-click a row to copy its value
  or name.
* Values are cut at 2000 characters.
* The names CQ-editor injects (`show_object`, `debug`, …) are hidden.

## Traceback

When a run fails, the **Traceback** panel shows:

* the exception type and message, in red;
* one row per frame (**File**, **Line**, **Function**, **Code**), starting
  at your script. Frames inside CQ-editor are hidden, frames inside CadQuery
  are kept;
* a copy button for the full Python traceback.

Click a row that belongs to your script to jump to that line, in the tab of
the script that ran. A row in another Python file that exists, such as a
module of your project, opens that file at the line. When the last run
succeeded, the panel shows *No errors*.

## Log

The **Log** panel collects everything that is not a direct result:

| Source | Examples |
| --- | --- |
| `print` | `print()` calls in scripts (stdout) |
| `stderr` | Text written to `sys.stderr` and native library messages (OpenCascade) |
| `script` | `log(value)` calls |
| Python loggers | `logging.info(...)`, warnings such as *Ignoring invalid display option* |
| CQ-editor | Worker start-up, recovery events, file reload messages |

* Toggle **Errors**, **Warnings**, **Info** and **Debug** to filter by level
  (Debug is hidden by default), or type in the filter box, which matches the
  text and the source and ignores case.
* The copy button copies the lines currently shown. **Clear** (or **Run >
  Clear Log**) empties the panel.
* Every line is also written to the log file `logs/cq-editor.log` in the
  [application data
  folder](../getting-started/installation.md#where-cq-editor-keeps-its-files),
  which survives a crash of CQ-editor and is not affected by **Clear**.
* The log keeps the newest 10,000 lines.
