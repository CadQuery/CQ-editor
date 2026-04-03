import sys
import faulthandler

# PyInstaller with console=False sets sys.stdout/stderr to None on Windows
if sys.stdout is None:
    sys.stdout = open("nul", "w")
if sys.stderr is None:
    sys.stderr = open("nul", "w")

faulthandler.enable()

from cq_editor.cqe_run import main

if __name__ == "__main__":
    main()
