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

## Installation (Binary Builds)

Development builds are now available that should work stand-alone without Anaconda. Click on the newest build with a green checkmark [here](https://github.com/jmwright/CQ-editor/actions?query=workflow%3Abuild), wait for the _Artifacts_ section at the bottom of the page to load, and then click on the appropriate download for your operating system. Extract the archive file and run the shell (*nix) or batch (Windows) script in the root CQ-editor directory. The CQ-editor window should launch.

A stable version of these builds will be provided in the future, but are not available currently.

## Usage

### Showing Objects

By default, CQ-editor will display a 3D representation of all `Workplane` objects in a script with a default color and alpha (transparency). To have more control over what is shown, and what the color and alpha settings are, the `show_object` method can be used. `show_object` tells CQ-editor to explicity display an object, and accepts the `options` parameter. The `options` parameter is a dictionary of rendering options named `alpha` and `color`. `alpha` is scaled between 0.0 and 1.0, with 0.0 being completely opaque and 1.0 being completely transparent. The color is set using R (red), G (green) and B (blue) values, and each one is scaled from 0 to 255. Either option or both can be omitted.

```python
show_object(result, options={"alpha":0.5, "color": (64, 164, 223)})
```

Note that `show_object` works for `Shape` and `TopoDS_Shape` objects too. In order to display objects from the embedded Python console use `show`.
