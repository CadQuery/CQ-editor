# CadQuery editor

[![Build status](https://ci.appveyor.com/api/projects/status/g98rs7la393mgy91/branch/master?svg=true)](https://ci.appveyor.com/project/adam-urbanczyk/cq-editor/branch/master)
[![codecov](https://codecov.io/gh/CadQuery/CQ-editor/branch/master/graph/badge.svg)](https://codecov.io/gh/CadQuery/CQ-editor)


CadQuery GUI editor based on PyQT.

![Screenshot](https://github.com/CadQuery/CQ-editor/raw/master/screenshots/screenshot2.png)

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

CQ-editor works on Linux, Windows and Mac. To try it out clone this git repository and set up the following conda environment:
```
conda env create -f cqgui_env.yml -n cqgui
conda activate cqgui
pip install 
python run.py
```
