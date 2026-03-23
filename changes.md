# Release 0.5.0

* Implemented a dark theme and tweaked it to work better with icons (#490)
* Fixed bug causing the cursor to jump when autoreload was enabled (#496)
* Added a max line length indicator (#495)
* Mentioned work-around for Linux-Wayland users in the readme (#497)
* Fixed bug where dragging over an object while rotating would select the object (#498)
* Changed alpha setting in `show_object` to be consistent with other CadQuery alpha settings (#499)
* Added ability to clear the Log View and Console (#500)
* Started preserving undo history across saves (#501)
* Updated run.sh script to find the real path to the executable (#505)

# Release 0.4.0

* Updated version pins in order to fix some issues, including segfaults on Python 3.12
* Changed to forcing UTF-8 when saving (#480)
* Added `Toggle Comment` Edit menu item with a `Ctrl+/` hotkey (#481)
* Fixed the case where long exceptions would force the window to expand (#481)
* Add stdout redirect so that print statements could be used and passed to log viewer (#481)
* Improvements in stdout redirect method (#483 and #485)
* Fixed preferences drop downs not populating (#484)
* Fixed double-render calls on saves in some cases (#486)
* Added a lint check to CI and linted codebase (#487)
