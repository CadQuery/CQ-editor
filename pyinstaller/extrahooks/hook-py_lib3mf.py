# hook-py_lib3mf.py
from PyInstaller.utils.hooks import collect_dynamic_libs

binaries = collect_dynamic_libs('py_lib3mf')
