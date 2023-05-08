# CadQuery editor

This is a fork of [jmwright's fork](https://github.com/jmwright/CQ-editor) of [CadQuery/CQ-editor](https://github.com/CadQuery/CQ-editor). This fork includes changes that enable dark mode for CQ-editor (see screenshot below). Under the GitHub Actions menu this fork also contains static builds of CQ-editor for Linux/MacOS/Windows that include the [cq_gears](https://github.com/meadiode/cq_gears), [cq_cache](https://github.com/CadQuery/cadquery-plugins/tree/main/plugins/cq_cache), [cq_more](https://github.com/JustinSDK/cqMore), [cq_warehouse](https://github.com/gumyr/cq_warehouse), and [build123d](https://github.com/gumyr/build123d) libraries. Note you need to change color preferences to enable dark mode for all panes (see Edit -> Preferences).

This fork also contains additional changes to the `show_object` function in CQ-editor that make it easier to display and export build123d objects and object lists.

Running into issues? Please click here to join the [***CadQuery, CQ-Editor, and build123d Discord***](https://discord.com/invite/Bj9AQPsCfx)

![image](https://user-images.githubusercontent.com/16868537/191054760-a2cac297-3488-48d4-b9f6-52747dffcce3.png)

[![Build status](https://ci.appveyor.com/api/projects/status/g98rs7la393mgy91/branch/master?svg=true)](https://ci.appveyor.com/project/adam-urbanczyk/cq-editor/branch/master)
[![codecov](https://codecov.io/gh/CadQuery/CQ-editor/branch/master/graph/badge.svg)](https://codecov.io/gh/CadQuery/CQ-editor)
[![Build Status](https://dev.azure.com/cadquery/CQ-editor/_apis/build/status/CadQuery.CQ-editor?branchName=master)](https://dev.azure.com/cadquery/CQ-editor/_build/latest?definitionId=3&branchName=master)
[![DOI](https://zenodo.org/badge/136604983.svg)](https://zenodo.org/badge/latestdoi/136604983)

CadQuery GUI editor based on PyQT supports Linux, Windows and Mac.

<img src="https://github.com/CadQuery/CQ-editor/raw/master/screenshots/screenshot2.png" alt="Screenshot" width="70%" >
<img src="https://github.com/CadQuery/CQ-editor/raw/master/screenshots/screenshot3.png" alt="Screenshot" width="70%" >
<img src="https://github.com/CadQuery/CQ-editor/raw/master/screenshots/screenshot4.png" alt="Screenshot" width="70%" >

## Notable features

* OCCT based
* Graphical debugger for CadQuery scripts
  * Step through script and watch how your model changes
* CadQuery object stack inspector
  * Visual inspection of current workplane and selected items
  * Insight into evolution of the model
* Export to various formats
  * STL
  * STEP

## Installation - Pre-Built Packages (Recommended)

### Release Packages

TBD

### Development Packages

Development builds are also available, but can be unstable and should be used at your own risk. Click on the newest build with a green checkmark [here](https://github.com/jdegenstein/jmwright-CQ-editor/actions), wait for the _Artifacts_ section at the bottom of the page to load, and then click on the appropriate download for your operating system. Extract the archive file and run the shell (Linux/MacOS) or cmd (Windows) script in the root CQ-editor directory. The CQ-editor window should launch.

## Installation (pip)

Additional packages for Linux (known as needed on Ubuntu 22.04):
```
sudo apt install qtbase5-dev qt5-qmake
```
All platforms (Windows/Mac/Linux):
```
pip install git+https://github.com/jdegenstein/jmwright-CQ-Editor
pip install --pre "cadquery>=2.2"
pip install git+https://github.com/gumyr/build123d
```

## Usage

### Showing Objects

By default, CQ-editor will display a 3D representation of all `Workplane` objects in a script with a default color and alpha (transparency). To have more control over what is shown, and what the color and alpha settings are, the `show_object` method can be used. `show_object` tells CQ-editor to explicity display an object, and accepts the `options` parameter. The `options` parameter is a dictionary of rendering options named `alpha` and `color`. `alpha` is scaled between 0.0 and 1.0, with 0.0 being completely opaque and 1.0 being completely transparent. The color is set using R (red), G (green) and B (blue) values, and each one is scaled from 0 to 255. Either option or both can be omitted. The `name` parameter can assign a custom name which will appear in the objects pane of CQ-editor.

```python
show_object(result, name="somename", options={"alpha":0.5, "color": (64, 164, 223)})
# or using rand_color:
show_object(result, name="somename", options=rand_color(alpha=.5))
```

Note that `show_object` works for `Shape` and `TopoDS_Shape` objects too. In order to display objects from the embedded Python console use `show`.

### Rotate, Pan and Zoom the 3D View

The following mouse controls can be used to alter the view of the 3D object, and should be familiar to CAD users, even if the mouse buttons used may differ.

* _Left Mouse Button_ + _Drag_ = Rotate
* _Middle Mouse Button_ + _Drag_ = Pan
* _Right Mouse Button_ + _Drag_ = Zoom
* _Mouse Wheel_ = Zoom

### Debugging Objects

There are multiple menu options to help in debugging a CadQuery script. They are included in the `Run` menu, with corresponding buttons in the toolbar. Below is a listing of what each menu item does.

* `Debug` (Ctrl + F5) - Instead of running the script completely through as with the `Render` item, it begins executing the script but stops at the first non-empty line, waiting for the user to continue execution manually.
* `Step` (Ctrl + F10) - Will move execution of the script to the next  non-empty line.
* `Step in` (Ctrl + F11) - Will follow the flow of execution to the inside of a user-created function defined within the script.
* `Continue` (Ctrl + F12) - Completes execution of the script, starting from the current line that is being debugged.

It is also possible to do visual debugging of objects. This is possible by using the `debug()` function to display an object instead of `show_object()`. An alternative method for the following code snippet is shown below for highlighting a specific face, but it demonstrates one use of `debug()`.
```python
import cadquery as cq

result = cq.Workplane().box(10, 10, 10)

highlight = result.faces('>Z')

show_object(result, name='box')
debug(highlight)
```
Objects displayed with `debug()` are colored in red and have their alpha set so they are semi-transparent. This can be useful for checking for interference, clearance, or whether the expected face is being selected, as in the code above.

### Console Logging

Python's standard `print()` function will not output to the CQ-editor GUI, and `log()` should be used instead. `log()` will output the provided text to the _Log viewer_ panel, providing another way to debug CadQuery scripts. If you started CQ-editor from the command line, the `print()` function will output text back to it.

### Using an External Code Editor

Some users prefer to use an external code editor instead of the built-in Spyder-based editor that comes stock with CQ-editor. The steps below should allow CQ-editor to work alongside most text editors.

1. Open the Preferences dialog by clicking `Edit->Preferences`.
2. Make sure that `Code Editor` is selected in the left pane.
3. Check `Autoreload` in the right pane.
4.  If CQ-editor is not catching the saves from your external editor, increasing `Autoreload delay` in the right pane may help. This issue has been reported when using vim or emacs.

### Exporting an Object

Any object can be exported to either STEP or STL format. The steps for doing so are listed below.

1. Highlight the object to be exported in the _Objects_ panel.
2. Click either `Export as STL` or `Export as STEP` from the `Tools` menu, depending on which file format you want to export. Both of these options will be disabled if an object is not selected in the _Objects_ panel.

Clicking either _Export_ item will present a file dialog that allows the file name and location of the export file to be set.

### Displaying All Wires for Debugging

**NOTE:** This is intended for debugging purposes, and if not removed, could interfere with the execution of your model in some cases.

Using `consolidateWires()` is a quick way to combine all wires so that they will display together in CQ-editor's viewer. In the following code, it is used to make sure that both rects are displayed. This technique can make it easier to debug in-progress 2D sketches.

```python
import cadquery as cq
res = cq.Workplane().rect(1,1).rect(3,3).consolidateWires()
show_object(res)
```

### Highlighting a Specific Face

Highlighting a specific face in a different color can be useful when debugging, or when trying to learn CadQuery selectors. The following code creates a separate, highlighted object to show the selected face in red. This is an alternative to using a `debug()` object, and in most cases `debug()` will provide the same result with less code. However, this method will allow the color and alpha of the highlight object to be customized.

```python
import cadquery as cq

result = cq.Workplane().box(10, 10, 10)

highlight = result.faces('>Z')

show_object(result)
show_object(highlight,'highlight',options=dict(alpha=0.1,color=(1.,0,0)))
```

### Naming an Object

By default, objects have a randomly generated ID in the object inspector. However, it can be useful to name objects so that it is easier to identify them. The `name` parameter of `show_object()` can be used to do this.

```python
import cadquery as cq

result = cq.Workplane().box(10, 10, 10)

show_object(result, name='box')
```
