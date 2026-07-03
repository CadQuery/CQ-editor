import os
import sys

# Python 3.8+ uses a restricted DLL search path on Windows. PyInstaller adds
# _MEIPASS to the search path but not subdirectories. casadi's companion DLLs
# live in _MEIPASS/casadi/, so register that directory explicitly.
if sys.platform == "win32":
    casadi_dir = os.path.join(sys._MEIPASS, "casadi")
    if os.path.isdir(casadi_dir):
        os.add_dll_directory(casadi_dir)
