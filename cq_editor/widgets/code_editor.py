from PyQt5 import QtCore, QtGui, QtWidgets


DARK_BLUE = QtGui.QColor(118, 150, 185)


class LineNumberArea(QtWidgets.QWidget):
    def __init__(self, editor):
        super(LineNumberArea, self).__init__(editor)
        self._code_editor = editor

    def sizeHint(self):
        return QtCore.QSize(self._code_editor.line_number_area_width(), 0)

    def paintEvent(self, event):
        self._code_editor.lineNumberAreaPaintEvent(event)


class CodeTextEdit(QtWidgets.QPlainTextEdit):
    is_first = False
    pressed_keys = list()

    indented = QtCore.pyqtSignal(object)
    unindented = QtCore.pyqtSignal(object)
    commented = QtCore.pyqtSignal(object)
    uncommented = QtCore.pyqtSignal(object)

    def __init__(self):
        super(CodeTextEdit, self).__init__()

        self.indented.connect(self.do_indent)
        self.unindented.connect(self.undo_indent)
        self.commented.connect(self.do_comment)
        self.uncommented.connect(self.undo_comment)

    def clear_selection(self):
        """
        Clear text selection on cursor
        """
        pos = self.textCursor().selectionEnd()
        self.textCursor().movePosition(pos)

    def get_selection_range(self):
        """
        Get text selection line range from cursor
        Note: currently only support continuous selection

        :return: (int, int). start line number and end line number
        """
        cursor = self.textCursor()
        if not cursor.hasSelection():
            return 0, 0

        start_pos = cursor.selectionStart()
        end_pos = cursor.selectionEnd()

        cursor.setPosition(start_pos)
        start_line = cursor.blockNumber()
        cursor.setPosition(end_pos)
        end_line = cursor.blockNumber()

        return start_line, end_line

    def remove_line_start(self, string, line_number):
        """
        Remove certain string occurrence on line start

        :param string: str. string pattern to remove
        :param line_number: int. line number
        """
        cursor = QtGui.QTextCursor(
            self.document().findBlockByLineNumber(line_number))
        cursor.select(QtGui.QTextCursor.LineUnderCursor)
        text = cursor.selectedText()
        if text.startswith(string):
            cursor.removeSelectedText()
            cursor.insertText(text.split(string, 1)[-1])

    def insert_line_start(self, string, line_number):
        """
        Insert certain string pattern on line start

        :param string: str. string pattern to insert
        :param line_number: int. line number
        """
        cursor = QtGui.QTextCursor(
            self.document().findBlockByLineNumber(line_number))
        self.setTextCursor(cursor)
        self.textCursor().insertText(string)

    def keyPressEvent(self, event):
        """
        Extend the key press event to create key shortcuts
        """
        self.is_first = True
        self.pressed_keys.append(event.key())
        start_line, end_line = self.get_selection_range()

        # indent event
        if event.key() == QtCore.Qt.Key_Tab and \
                (end_line - start_line):
            lines = range(start_line, end_line+1)
            self.indented.emit(lines)
            return

        # un-indent event
        elif event.key() == QtCore.Qt.Key_Backtab:
            lines = range(start_line, end_line+1)
            self.unindented.emit(lines)
            return

        super(CodeTextEdit, self).keyPressEvent(event)

    def keyReleaseEvent(self, event):
        """
        Extend the key release event to catch key combos
        """
        if self.is_first:
            self.process_multi_keys(self.pressed_keys)

        self.is_first = False
        self.pressed_keys.pop()
        super(CodeTextEdit, self).keyReleaseEvent(event)

    def process_multi_keys(self, keys):
        """
        Placeholder for processing multiple key combo events

        :param keys: [QtCore.Qt.Key]. key combos
        """
        # toggle comments indent event
        if keys == [QtCore.Qt.Key_Control, QtCore.Qt.Key_Slash]:
            pass

    def do_indent(self, lines):
        """
        Indent lines

        :param lines: [int]. line numbers
        """
        for line in lines:
            self.insert_line_start('\t', line)

    def undo_indent(self, lines):
        """
        Un-indent lines

        :param lines: [int]. line numbers
        """
        for line in lines:
            self.remove_line_start('\t', line)

    def do_comment(self, lines):
        """
        Comment out lines

        :param lines: [int]. line numbers
        """
        for line in lines:
            pass

    def undo_comment(self, lines):
        """
        Un-comment lines

        :param lines: [int]. line numbers
        """
        for line in lines:
            pass


class EdgeLine(QtWidgets.QWidget):
    edge_line = None
    columns = 80

    def __init__(self):
        super(QtWidgets.QWidget, self).__init__()

    def set_enabled(self, enabled_state):
        self.setEnabled = enabled_state

    def set_columns(self, number_of_columns):
        self.columns = number_of_columns


class CodeEditor(CodeTextEdit):
    def __init__(self, parent=None):
        super(CodeEditor, self).__init__()
        self.line_number_area = LineNumberArea(self)

        self.font = QtGui.QFont()
        self.font.setFamily("Courier New")
        self.font.setStyleHint(QtGui.QFont.Monospace)
        self.font.setPointSize(10)
        self.setFont(self.font)

        self.tab_size = 4
        self.setTabStopWidth(self.tab_size * self.fontMetrics().width(' '))

        self.blockCountChanged.connect(self.update_line_number_area_width)
        self.updateRequest.connect(self.update_line_number_area)
        self.cursorPositionChanged.connect(self.highlight_current_line)

        self.update_line_number_area_width(0)
        self.highlight_current_line()

        self.menu = QtWidgets.QMenu()

        self.edge_line = EdgeLine()

        self._filename = ""

    def setup_editor(
        self,
        line_numbers=True,
        markers=True,
        edge_line=100,
        tab_mode=False,
        show_blanks=True,
        font=QtGui.QFontDatabase.systemFont(QtGui.QFontDatabase.FixedFont),
        language="Python",
        filename="",):
        pass

    def set_color_scheme(self, color_scheme):
        # print(color_scheme)
        pass

    def set_font(self, new_font):
        self.font = new_font

    def toggle_wrap_mode(self, wrap_mode):
        # print(wrap_mode)
        pass

    def go_to_line(self, line_number):
        # print(line_number)
        pass

    def set_text(self, new_text):
        self.setPlainText(new_text)

    def set_text_from_file(self, file_name):
        self._filename = file_name
        # Load the text into the text field
        pass

    def line_number_area_width(self):
        digits = 1
        max_num = max(1, self.blockCount())
        while max_num >= 10:
            max_num *= 0.1
            digits += 1

        space = 30 + self.fontMetrics().width('9') * digits
        return space

    def resizeEvent(self, e):
        super(CodeEditor, self).resizeEvent(e)
        cr = self.contentsRect()
        width = self.line_number_area_width()
        rect = QtCore.QRect(cr.left(), cr.top(), width, cr.height())
        self.line_number_area.setGeometry(rect)

    def lineNumberAreaPaintEvent(self, event):
        painter = QtGui.QPainter(self.line_number_area)
        try:
            # painter.fillRect(event.rect(), QtCore.Qt.lightGray)
            block = self.firstVisibleBlock()
            block_number = block.blockNumber()
            offset = self.contentOffset()
            top = self.blockBoundingGeometry(block).translated(offset).top()
            bottom = top + self.blockBoundingRect(block).height()

            while block.isValid() and top <= event.rect().bottom():
                if block.isVisible() and bottom >= event.rect().top():
                    number = str(block_number + 1)
                    painter.setPen(DARK_BLUE)
                    width = self.line_number_area.width() - 10
                    height = self.fontMetrics().height()
                    painter.drawText(0, int(top), width, height, QtCore.Qt.AlignCenter, number)

                block = block.next()
                top = bottom
                bottom = top + self.blockBoundingRect(block).height()
                block_number += 1
        finally:
            painter.end()

    def update_line_number_area_width(self, newBlockCount):
        self.setViewportMargins(self.line_number_area_width(), 0, 0, 0)

    def update_line_number_area(self, rect, dy):
        if dy:
            self.line_number_area.scroll(0, dy)
        else:
            width = self.line_number_area.width()
            self.line_number_area.update(0, rect.y(), width, rect.height())

        if rect.contains(self.viewport().rect()):
            self.update_line_number_area_width(0)

    def highlight_current_line(self):
        extra_selections = list()
        if not self.isReadOnly():
            selection = QtWidgets.QTextEdit.ExtraSelection()
            line_color = DARK_BLUE.lighter(160)
            selection.format.setBackground(line_color)
            selection.format.setProperty(QtGui.QTextFormat.FullWidthSelection, True)

            selection.cursor = self.textCursor()
            selection.cursor.clearSelection()
            extra_selections.append(selection)
        self.setExtraSelections(extra_selections)
