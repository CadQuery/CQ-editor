from traceback import extract_tb, format_exception_only
from itertools import dropwhile

from PyQt5.QtWidgets import (
    QWidget,
    QTreeWidget,
    QTreeWidgetItem,
    QAction,
    QLabel,
    QApplication,
)
from PyQt5.QtCore import Qt, pyqtSlot, pyqtSignal
from PyQt5.QtGui import QFontMetrics, QKeySequence

from ..mixins import ComponentMixin
from ..utils import layout


class TracebackTree(QTreeWidget):

    name = "Traceback Viewer"

    def __init__(self, parent):

        super(TracebackTree, self).__init__(parent)
        self.setHeaderHidden(False)
        self.setItemsExpandable(False)
        self.setRootIsDecorated(False)
        self.setContextMenuPolicy(Qt.ActionsContextMenu)

        self.setColumnCount(3)
        self.setHeaderLabels(["File", "Line", "Code"])

        self.root = self.invisibleRootItem()


class TracebackPane(QWidget, ComponentMixin):

    sigHighlightLine = pyqtSignal(int)

    def __init__(self, parent):

        super(TracebackPane, self).__init__(parent)

        self.tree = TracebackTree(self)
        self.current_exception = QLabel(self)
        self.current_exception.setStyleSheet("QLabel {color : red; }")

        # the message is shown elided, so let it at least be selected
        self.current_exception.setTextInteractionFlags(
            Qt.TextSelectableByMouse | Qt.TextSelectableByKeyboard
        )

        # the full exception text, unescaped and not elided
        self._exception_text = ""

        self.copy_action = QAction("Copy traceback", self)
        self.copy_action.setShortcut(QKeySequence.Copy)
        self.copy_action.setShortcutContext(Qt.WidgetWithChildrenShortcut)
        self.copy_action.triggered.connect(self.copyTraceback)
        self.tree.addAction(self.copy_action)

        layout(self, (self.current_exception, self.tree), self)

        self.tree.currentItemChanged.connect(self.handleSelection)

    def truncate_text(self, text, max_length=100):
        """
        Used to prevent the label from expanding the window width off the screen.
        """
        metrics = QFontMetrics(self.current_exception.font())
        elided_text = metrics.elidedText(
            text, Qt.ElideRight, self.current_exception.width() - 75
        )

        return elided_text

    @pyqtSlot(object, str)
    def addTraceback(self, exc_info, code):

        self.tree.clear()

        if exc_info:
            t, exc, tb = exc_info

            root = self.tree.root
            code = code.splitlines()

            for el in dropwhile(
                lambda el: "string>" not in el.filename, extract_tb(tb)
            ):
                # workaround of the traceback module
                if el.line == "":
                    line = code[el.lineno - 1].strip()
                else:
                    line = el.line

                root.addChild(QTreeWidgetItem([el.filename, str(el.lineno), line]))

            exc_name = t.__name__
            exc_msg = str(exc)

            self._exception_text = "{}: {}".format(exc_name, exc_msg)

            exc_msg = exc_msg.replace("<", "&lt;").replace(">", "&gt;")  # replace <>

            truncated_msg = self.truncate_text(exc_msg)
            self.current_exception.setText(
                "<b>{}</b>: {}".format(exc_name, truncated_msg)
            )
            self.current_exception.setToolTip(exc_msg)

            # handle the special case of a SyntaxError
            if t is SyntaxError:
                root.addChild(
                    QTreeWidgetItem(
                        [
                            exc.filename,
                            str(exc.lineno),
                            exc.text.strip() if exc.text else "",
                        ]
                    )
                )
        else:
            self._exception_text = ""
            self.current_exception.setText("")
            self.current_exception.setToolTip("")

    def tracebackText(self):
        """
        The traceback as it is shown in the pane, in the usual Python format.
        """

        if not self._exception_text:
            return ""

        lines = ["Traceback (most recent call last):"]

        root = self.tree.root

        for i in range(root.childCount()):
            item = root.child(i)
            filename, lineno, code = (item.data(col, 0) for col in range(3))

            lines.append('  File "{}", line {}'.format(filename, lineno))

            if code:
                lines.append("    {}".format(code))

        lines.append(self._exception_text)

        return "\n".join(lines)

    @pyqtSlot()
    def copyTraceback(self):
        """
        Puts the traceback on the clipboard, so that it can be pasted into a
        bug report or a search box.
        """

        text = self.tracebackText()

        if text:
            QApplication.clipboard().setText(text)

    @pyqtSlot(QTreeWidgetItem, QTreeWidgetItem)
    def handleSelection(self, item, *args):

        if item:
            f, line = item.data(0, 0), int(item.data(1, 0))

            if "<string>" in f:
                self.sigHighlightLine.emit(line)
