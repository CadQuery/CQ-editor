import sys
import os
import faulthandler

# OCP's Xw_Window requires an X11 window ID, so we force XCB so winId() is valid under Wayland/XWayland.
if sys.platform == "linux" and "QT_QPA_PLATFORM" not in os.environ:
    os.environ["QT_QPA_PLATFORM"] = "xcb"

# PyInstaller with console=False sets sys.stdout/stderr to None on Windows
if sys.stdout is None:
    sys.stdout = open("nul", "w")
if sys.stderr is None:
    sys.stderr = open("nul", "w")

faulthandler.enable()

from cq_editor.cqe_run import main

if __name__ == "__main__":
    main()
