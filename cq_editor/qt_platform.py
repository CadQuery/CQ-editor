"""Override platform env vars before Qt starts up to work around platforms
unsupported by ourselves or OCCT.

Must not import Qt: this runs before cq_editor.__main__ builds the QApplication.
"""

import os
import sys


def prefer_non_wayland_platform():
    """
    Reorder QT_QPA_PLATFORM so Qt reaches a Wayland plugin only after everything
    else has failed to load, defaulting to xcb when completely unset.

    Desktops such as COSMIC export QT_QPA_PLATFORM="wayland;xcb" session-wide,
    we want to respect this config as far as possible, without using wayland.

    On wayland occt_widget hands winId() to OCP's Xw_Window, which needs an X11
    window id and gets a Wayland surface id instead, dying with BadWindow.
    """
    if sys.platform != "linux":
        return

    plugins = (os.environ.get("QT_QPA_PLATFORM") or "xcb").split(";")
    os.environ["QT_QPA_PLATFORM"] = ";".join(
        sorted(plugins, key=lambda plugin: plugin.startswith("wayland"))
    )
