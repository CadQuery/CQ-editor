import sys

from PyQt5.QtWidgets import QMessageBox
from PyQt5 import sip

from cq_editor.__main__ import MainWindow
from cq_editor.main_window import PRINT_REDIRECTOR


def test_print_redirector_released_with_window(qtbot, mocker):
    """
    PRINT_REDIRECTOR is a module-level singleton that outlives any MainWindow.
    Once a window's LogViewer is destroyed, a write must be dropped rather than
    delivered to the now-deleted C++ object (which raises "wrapped C/C++ object
    of type LogViewer has been deleted").
    """

    mocker.patch.object(QMessageBox, "question", return_value=QMessageBox.Yes)
    mocker.patch.object(QMessageBox, "warning", return_value=QMessageBox.Discard)

    win = MainWindow()
    qtbot.addWidget(win)

    # destroy this window's LogViewer, as tearing the window down would
    sip.delete(win.components["log"])

    # PyQt routes an exception raised inside a slot to sys.excepthook rather
    # than propagating it, so capture it there while emitting
    errors = []
    original_hook = sys.excepthook
    sys.excepthook = lambda exc_type, *rest: errors.append(exc_type)
    try:
        PRINT_REDIRECTOR.sigStdoutWrite.emit("stray output after close")
    finally:
        sys.excepthook = original_hook

    assert errors == []
