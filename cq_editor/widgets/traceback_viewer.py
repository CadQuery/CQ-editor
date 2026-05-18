from traceback import extract_tb, format_exception_only
from itertools import dropwhile

from PyQt5.QtWidgets import QWidget, QTreeWidget, QTreeWidgetItem, QAction, QLabel, QHBoxLayout, QPushButton
from PyQt5.QtCore import Qt, pyqtSlot, pyqtSignal
from PyQt5.QtGui import QFontMetrics

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
    sigAutoFixError = pyqtSignal(str)

    def __init__(self, parent):

        super(TracebackPane, self).__init__(parent)
        self.last_exc_info = None
        self.last_code = None

        self.tree = TracebackTree(self)
        self.current_exception = QLabel(self)
        self.current_exception.setStyleSheet("QLabel {color : red; }")

        # Create a horizontal row for the exception message and the Auto-Fix button
        top_row_widget = QWidget(self)
        top_row_layout = QHBoxLayout(top_row_widget)
        top_row_layout.setContentsMargins(0, 0, 0, 0)
        top_row_layout.setSpacing(4)
        top_row_widget.setLayout(top_row_layout)

        top_row_layout.addWidget(self.current_exception, stretch=1)

        self.autofix_btn = QPushButton("✨ Auto-Fix with AI", top_row_widget)
        self.autofix_btn.setToolTip("Automatically send this error and code to the AI Assistant to fix it")
        self.autofix_btn.setStyleSheet(
            "QPushButton { background-color: #7B1FA2; color: white; font-weight: bold; border-radius: 3px; padding: 4px 8px; }"
            "QPushButton:hover { background-color: #8E24AA; }"
            "QPushButton:disabled { background-color: #BDBDBD; color: #757575; }"
        )
        self.autofix_btn.setEnabled(False)
        self.autofix_btn.clicked.connect(self.trigger_autofix)
        top_row_layout.addWidget(self.autofix_btn)

        layout(self, (top_row_widget, self.tree), self)

        self.tree.currentItemChanged.connect(self.handleSelection)

    def trigger_autofix(self):
        if not self.last_exc_info:
            return
        
        t, exc, tb = self.last_exc_info
        exc_name = t.__name__
        exc_msg = str(exc)
        
        tb_list = extract_tb(tb)
        filtered_tb = list(dropwhile(lambda el: "string>" not in el.filename, tb_list))
        
        error_context = ""
        if filtered_tb:
            last_frame = filtered_tb[-1]
            error_context = f"at line {last_frame.lineno}: `{last_frame.line}`"
        else:
            error_context = "in the script"
            
        prompt = (
            f"I encountered a '{exc_name}' error {error_context}.\n"
            f"Error details: {exc_msg}\n"
            f"Please identify the issue in the code and provide the complete, corrected CadQuery script."
        )
        self.sigAutoFixError.emit(prompt)

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
        self.last_exc_info = exc_info
        self.last_code = code

        if exc_info:
            t, exc, tb = exc_info
            self.autofix_btn.setEnabled(True)

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
            self.current_exception.setText("")
            self.current_exception.setToolTip("")
            self.autofix_btn.setEnabled(False)

    @pyqtSlot(QTreeWidgetItem, QTreeWidgetItem)
    def handleSelection(self, item, *args):

        if item:
            f, line = item.data(0, 0), int(item.data(1, 0))

            if "<string>" in f:
                self.sigHighlightLine.emit(line)
