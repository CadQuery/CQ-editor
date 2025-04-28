# CadQuery editor

[![Build status](https://ci.appveyor.com/api/projects/status/g98rs7la393mgy91/branch/master?svg=true)](https://ci.appveyor.com/project/adam-urbanczyk/cq-editor/branch/master)
[![codecov](https://codecov.io/gh/CadQuery/CQ-editor/branch/master/graph/badge.svg)](https://codecov.io/gh/CadQuery/CQ-editor)
[![Build Status](https://dev.azure.com/cadquery/CQ-editor/_apis/build/status/CadQuery.CQ-editor?branchName=master)](https://dev.azure.com/cadquery/CQ-editor/_build/latest?definitionId=3&branchName=master)
[![DOI](https://zenodo.org/badge/136604983.svg)](https://zenodo.org/badge/latestdoi/136604983)

CadQuery GUI editor based on PyQT that supports Linux, Windows and Mac.

<img src="https://github.com/CadQuery/CQ-editor/raw/master/screenshots/screenshot4.png" alt="Screenshot" width="70%" >

Additional screenshots are available [in the wiki](https://github.com/CadQuery/CQ-editor/wiki#screenshots).

## Notable features

* Automatic code reloading - you can use your favourite editor
* OCCT based
* Graphical debugger for CadQuery scripts
  * Step through script and watch how your model changes
* CadQuery object stack inspector
  * Visual inspection of current workplane and selected items
  * Insight into evolution of the model
* Export to various formats
  * STL
  * STEP

## Documentation

Documentation is available in the wiki. Topics covered are the following.

* [Installation](https://github.com/CadQuery/CQ-editor/wiki/Installation)
* [Usage](https://github.com/CadQuery/CQ-editor/wiki/Usage)
* [Configuration](https://github.com/CadQuery/CQ-editor/wiki/Configuration)

## Getting Help

For general discussion and questions about CQ-editor, please create a [GitHub Discussion](https://github.com/CadQuery/CQ-editor/discussions).

## Reporting a Bug

If you believe that you have found a bug in CQ-editor, please ensure the following.

* You are not running a CQ-editor fork, as these are not always synchronized with the latest updates in this project.
* You have searched the [issue tracker](https://github.com/CadQuery/CQ-editor/issues) to make sure that the bug is not already known.

If you have already checked those things, please file a [new issue](https://github.com/CadQuery/CQ-editor/issues/new) with the following information.

* Operating System (type, version, etc) - If running Linux, please include the distribution and the version.
* How CQ-editor was installed.
* Python version of your environment (unless you are running a pre-built package).
* Steps to reproduce the bug.
