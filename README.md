# CadQuery editor

[![Build status](https://ci.appveyor.com/api/projects/status/g98rs7la393mgy91/branch/master?svg=true)](https://ci.appveyor.com/project/adam-urbanczyk/cq-editor/branch/master)
[![codecov](https://codecov.io/gh/CadQuery/CQ-editor/branch/master/graph/badge.svg)](https://codecov.io/gh/CadQuery/CQ-editor)
[![Build Status](https://dev.azure.com/cadquery/CQ-editor/_apis/build/status/CadQuery.CQ-editor?branchName=master)](https://dev.azure.com/cadquery/CQ-editor/_build/latest?definitionId=3&branchName=master)

CadQuery GUI editor based on PyQT supports Linux, Windows and Mac.

<img src="https://github.com/CadQuery/CQ-editor/raw/master/screenshots/screenshot2.png" alt="Screenshot" width="70%" >
<img src="https://github.com/CadQuery/CQ-editor/raw/master/screenshots/screenshot3.png" alt="Screenshot" width="70%" >
<img src="https://github.com/CadQuery/CQ-editor/raw/master/screenshots/screenshot4.png" alt="Screenshot" width="70%" >

## Notable features

* PythonOCC based
* Graphical debugger for CadQuery scripts
  * Step through script and watch how your model changes
* CadQery object stack inspector
  * Visual inspection of current workplane and selected items
  * Insight into evolution of the model
* Export to various formats
  * STL
  * STEP

## Installation

To try it out clone this git repository and set up the following conda environment:
```
conda env create -f cqgui_env.yml -n cqgui
conda activate cqgui
python run.py
```

On some linux distributions (e.g. `Ubuntu 18.04`) it might be necessary to install additonal packages:
```
sudo apt install libglu1-mesa libgl1-mesa-dri mesa-common-dev libglu1-mesa-dev
```
