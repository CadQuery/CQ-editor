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
* CadQuery object stack inspector
  * Visual inspection of current workplane and selected items
  * Insight into evolution of the model
* Export to various formats
  * STL
  * STEP

## Installation

The easiest way to try it out is using provided self-contained archives:

* [0.1 RC1 Linux](https://github.com/CadQuery/CQ-editor/releases/download/0.1RC1/CQ-editor-0.1RC1-linux64.tar.bz2)
* [0.1 RC1 Windows](https://github.com/CadQuery/CQ-editor/releases/download/0.1RC1/CQ-editor-0.1RC1-win64.zip)

Alternatively one can use conda:
```
conda install -c cadquery -c conda-forge cq-editor
```
and then simply type `cq-editor` to run it.

## Installation of CQ-editor master

There are at least two reasons for running master.

### Testing latest version of CQ-editor

If you are interested in testing the CQ-editor master branch
this would be an approriate choice:
```
conda install -c cadquery -c conda-forge cq-editor=master
```
Then type `cq-editor` to run it:

### Contributing to CQ-editor

This is the option to use if you're considering
contributing pull requests:

First fork this [repo](https://github.com/CadQuery/CQ-editor),
then execute the following commands replacing EnvName,
AccountName, PythonSitePackagePrefix and PYTHONPATH
with appropriate values:
```
export EnvName=cqgui-master-fork
export AccountName=YOUR_ACCOUNT_NAME
export PythonSitePackagePrefix=/opt/anaconda/envs/$EnvName
export PYTHONPATH=$PythonSitePackagePrefix/lib/python3.7/site-package

git clone https://github.com/$AccountName/CQ-editor
cd CQ-editor
conda env create -f cqgui_env.yml -n $EnvName
conda activate $EnvName
python run.py
```
You'll probably want to add PYTHONPATH to your `.bashrc` or equivalent.

On some linux distributions (e.g. `Ubuntu 18.04`) it might be necessary to install additonal packages:
```
sudo apt install libglu1-mesa libgl1-mesa-dri mesa-common-dev libglu1-mesa-dev
```
On Fedora 29 the packages can be installed as follows:
```
dnf install -y mesa-libGLU mesa-libGL mesa-libGLU-devel
```

