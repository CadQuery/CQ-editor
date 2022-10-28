# hook-casadi.py
from PyInstaller.utils.hooks import collect_dynamic_libs

binaries = collect_dynamic_libs('casadi')

# Something about legacy import codepaths in casadi.casadi causes PyInstaller's analysis to pick up
# casadi._casadi as a top-level _casadi module, which is wrong.
hiddenimports = ['casadi._casadi']
excludedimports = ['_casadi']
