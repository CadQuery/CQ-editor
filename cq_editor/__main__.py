import os
import sys
import argparse

from PyQt5.QtWidgets import QApplication

NAME = "CQ-editor"

# Reorder QT_QPA_PLATFORM so Qt reaches a Wayland plugin only after everything
# else has failed to load, defaulting to xcb. OCCT does not yet support Wayland.
if sys.platform == "linux":
    # Desktops such as COSMIC export QT_QPA_PLATFORM="wayland;xcb" session-wide,
    # we want to respect this config as far as possible, without using wayland.
    plugins = (os.environ.get("QT_QPA_PLATFORM") or "xcb").split(";")
    os.environ["QT_QPA_PLATFORM"] = ";".join(
        sorted(plugins, key=lambda plugin: plugin.startswith("wayland"))
    )

# need to initialize QApp here, otherwise svg icons do not work on windows
app = QApplication(sys.argv, applicationName=NAME)

from .main_window import MainWindow


def main():

    parser = argparse.ArgumentParser(description=NAME)
    parser.add_argument("filename", nargs="?", default=None)

    args = parser.parse_args(app.arguments()[1:])

    # sys.exit(app.exec_())

    try:
        win = MainWindow(filename=args.filename if args.filename else None)
        win.show()
        app.exec_()
    except Exception as e:
        import traceback

        traceback.print_exc()


if __name__ == "__main__":

    main()
