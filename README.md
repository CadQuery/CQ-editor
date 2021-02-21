# CadQuery editor

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

## Installation (Binary Builds)

Stable release builds which do not require Anaconda are attached to the [latest release](https://github.com/CadQuery/CQ-editor/releases). Download the zip file for your operating system, extract it, and run the CQ-editor script for your OS (CQ-editor.cmd for Windows, CQ-editor.sh for Linux and MacOS). On Windows you should be able to simply double-click on CQ-editor.cmd. On Linux and MacOS you may need to make the script executable with `chmod +x CQ-editor.sh` and run the script from the command line. The script contains an environment variable export that may be required to get CQ-editor to launch correctly on MacOS Big Sur, so it is better to use the script than to launch CQ-editor directly.

Development builds are also available, but you must be logged in to GitHub to get access. Click on the newest build with a green checkmark [here](https://github.com/jmwright/CQ-editor/actions?query=workflow%3Abuild), wait for the _Artifacts_ section at the bottom of the page to load, and then click on the appropriate download for your operating system. Extract the archive file and run the shell (Linux/MacOS) or batch (Windows) script in the root CQ-editor directory. The CQ-editor window should launch.

## Installation (Anaconda)

Use conda to install:
```
conda install -c cadquery -c conda-forge cq-editor=master
```
and then simply type `cq-editor` to run it. This installs the latest version built directly from the HEAD of this repository.

Alternatively clone this git repository and set up the following conda environment:
```
conda env create -f cqgui_env.yml -n cqgui
conda activate cqgui
python run.py
```

On some linux distributions (e.g. `Ubuntu 18.04`) it might be necessary to install additonal packages:
```
sudo apt install libglu1-mesa libgl1-mesa-dri mesa-common-dev libglu1-mesa-dev
```
On Fedora 29 the packages can be installed as follows:
```
dnf install -y mesa-libGLU mesa-libGL mesa-libGLU-devel
```

## Usage

### Showing Objects

By default, CQ-editor will display a 3D representation of all `Workplane` objects in a script with a default color and alpha (transparency). To have more control over what is shown, and what the color and alpha settings are, the `show_object` method can be used. `show_object` tells CQ-editor to explicity display an object, and accepts the `options` parameter. The `options` parameter is a dictionary of rendering options named `alpha` and `color`. `alpha` is scaled between 0.0 and 1.0, with 0.0 being completely opaque and 1.0 being completely transparent. The color is set using R (red), G (green) and B (blue) values, and each one is scaled from 0 to 255. Either option or both can be omitted.

```python
show_object(result, options={"alpha":0.5, "color": (64, 164, 223)})
```

Note that `show_object` works for `Shape` and `TopoDS_Shape` objects too. In order to display objects from the embedded Python console use `show`.

### Rotate, Pan and Zoom

* _Left Mouse Button_ + _Drag_ = Rotate
* _Middle Mouse Button_ + _Drag_ = Pan
* _Right Mouse Button_ + _Drag_ = Zoom
* _Mouse Wheel_ = Zoom

### Using an External Code Editor

1. Open the Preferences dialog by clicking `Edit->Preferences`.
2. Make sure that `Code Editor` is selected in the left pane.
3. Check `autoreload` in the right pane.
4.  If CQ-editor is not catching the saves from your external editor, increasing `Autoreload delay` in the right pane may help. This is a fairly common issue when using vim or emacs.

### Displaying All Wires for Debugging

**NOTE:** This is for debugging purposes and if not removed, could interfere with the creation of your model.

Using `consolidateWires()` is a quick way to combine all wires so that they will display together in CQ-editor's viewer. In the following code, it is used to make sure that both rects are displayed. This technique can make it easier to debug in-progress 2D sketches.

```python
import cadquery as cq
res = cq.Workplane().rect(1,1).rect(3,3).consolidateWires()
show_object(res)
```

### Highlighting a Specific Face

Highlighting a specific face in a different color can be useful when debugging, or when trying to learn CadQuery selectors. The following code creates a separate, highlighted object to show the selected face in red.

```python
import cadquery as cq

result = cq.Workplane().box(10, 10, 10)

highlight = result.faces('>Z')

show_object(result)
show_object(highlight,'highlight',options=dict(alpha=0.1,color=(1.,0,0)))
```

### Naming an Object

By default, objects have a randomly generated ID in the object inspector. It can be useful to name objects so that it is easier to identify them. The `name` parameter of `show_object()` can be used to do this.

```python
import cadquery as cq

result = cq.Workplane().box(10, 10, 10)

highlight = result.faces('>Z')

show_object(result, name='box')
```
