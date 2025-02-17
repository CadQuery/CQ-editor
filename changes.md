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
