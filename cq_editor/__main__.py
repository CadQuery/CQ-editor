import sys, os
import argparse

from PyQt5.QtWidgets import QApplication

NAME = 'CadQuery GUI (PyQT)'

#need to initialize QApp here, otherewise svg icons do not work on windows
if sys.platform == "win32":
    app = QApplication(sys.argv, applicationName=NAME)

from .main_window import MainWindow

def main():
    # if this is not a windows platform, initialize QApp here.
    # This also silences the warning from qt about initializing
    # pyqtgraph after QApplication
    if sys.platform != "win32":
        app = QApplication(sys.argv, applicationName=NAME)

    win = MainWindow()
    parser = argparse.ArgumentParser(description=NAME)
    parser.add_argument('filename',nargs='?',default=None)
    args = parser.parse_args(app.arguments()[1:])

    if args.filename is not None:
        win.components['editor'].load_from_file(args.filename)

    win.show()
    sys.exit(app.exec_())


if __name__ == "__main__":

    main()
