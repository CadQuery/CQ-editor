import signal
import sys
import argparse

from PyQt5.QtWidgets import QApplication

NAME = 'CQ-editor'

#need to initialize QApp here, otherewise svg icons do not work on windows
app = QApplication(sys.argv,
                   applicationName=NAME)

from .main_window import MainWindow

def main():

    parser = argparse.ArgumentParser(description=NAME)
    parser.add_argument('filename',nargs='?',default=None)

    args = parser.parse_args(app.arguments()[1:])

    signal.signal(signal.SIGINT, signal.SIG_DFL)  # handle Ctrl+C

    win = MainWindow(filename=args.filename if args.filename else None)

    win.show()
    sys.exit(app.exec_())


if __name__ == "__main__":

    main()
