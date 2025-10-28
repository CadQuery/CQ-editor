import os
from PyQt5 import QtCore, QtGui, QtWidgets
from PyQt5.QtGui import QPalette, QColor


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

    def __init__(self):
        super(CodeTextEdit, self).__init__()

        self.indented.connect(self.do_indent)
        self.unindented.connect(self.undo_indent)

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
        cursor = QtGui.QTextCursor(self.document().findBlockByLineNumber(line_number))
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
        cursor = QtGui.QTextCursor(self.document().findBlockByLineNumber(line_number))
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
        if event.key() == QtCore.Qt.Key_Tab:
            lines = range(start_line, end_line + 1)
            self.indented.emit(lines)
            return
        elif event.key() == QtCore.Qt.Key_Tab and (end_line - start_line):
            lines = range(start_line, end_line + 1)
            self.indented.emit(lines)
            return
        # un-indent event
        elif event.key() == QtCore.Qt.Key_Backtab:
            lines = range(start_line, end_line + 1)
            self.unindented.emit(lines)
            return

        super(CodeTextEdit, self).keyPressEvent(event)

    def do_indent(self, lines):
        """
        Indent lines

        :param lines: [int]. line numbers
        """
        for line in lines:
            self.insert_line_start("    ", line)

    def undo_indent(self, lines):
        """
        Un-indent lines

        :param lines: [int]. line numbers
        """
        for line in lines:
            self.remove_line_start("    ", line)

        # Set the cursor to the beginning of the last line
        cursor = self.textCursor()
        cursor.setPosition(cursor.selectionEnd())  # Move to end of selection
        cursor.movePosition(QtGui.QTextCursor.StartOfLine)  # Jump to start of line
        self.setTextCursor(cursor)


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
        self.setTabStopWidth(self.tab_size * self.fontMetrics().width(" "))

        self.blockCountChanged.connect(self.update_line_number_area_width)
        self.updateRequest.connect(self.update_line_number_area)
        # self.cursorPositionChanged.connect(self.highlight_current_line)

        self.update_line_number_area_width(0)
        # self.highlight_current_line()

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
        filename="",
    ):
        print("setup_editor called")

    def set_color_scheme(self, color_scheme):
        """
        Sets the color theme of the editor widget.
        :param str color_scheme: Name of the color theme to be set
        """

        if color_scheme == "Light":
            self.setStyleSheet("")
            self.setPalette(QtWidgets.QApplication.style().standardPalette())
        else:
            # Now use a palette to switch to dark colors:
            white_color = QColor(255, 255, 255)
            black_color = QColor(0, 0, 0)
            red_color = QColor(255, 0, 0)
            palette = QPalette()
            palette.setColor(QPalette.Window, QColor(53, 53, 53))
            palette.setColor(QPalette.WindowText, white_color)
            palette.setColor(QPalette.Base, QColor(25, 25, 25))
            palette.setColor(QPalette.AlternateBase, QColor(53, 53, 53))
            palette.setColor(QPalette.ToolTipBase, black_color)
            palette.setColor(QPalette.ToolTipText, white_color)
            palette.setColor(QPalette.Text, white_color)
            palette.setColor(QPalette.Button, QColor(53, 53, 53))
            palette.setColor(QPalette.ButtonText, white_color)
            palette.setColor(QPalette.BrightText, red_color)
            palette.setColor(QPalette.Link, QColor(42, 130, 218))
            palette.setColor(QPalette.Highlight, QColor(42, 130, 218))
            palette.setColor(QPalette.HighlightedText, black_color)
            self.setPalette(palette)

    def set_font(self, new_font):
        self.font = new_font

    def toggle_wrap_mode(self, wrap_mode):
        self.setLineWrapMode(wrap_mode)

    def set_cursor_position(self, position):
        """
        Allows the caller to set the position of the cursor within
        the editor text.
        """

        cursor = self.textCursor()
        cursor.setPosition(position)
        self.setTextCursor(cursor)

    def go_to_line(self, line_number):
        """
        Set the text cursor at a specific line number.
        """

        cursor = self.textCursor()

        # Line numbers start at 0
        block = self.document().findBlockByNumber(line_number - 1)

        cursor.setPosition(block.position())
        self.setTextCursor(cursor)

    def toggle_comment_single_line(self, cursor, left_pos):
        """
        Adds the comment character (#) and a space at the beginning of a line,
        or removes them, if needed.
        """

        # Move right by pos characters to the position before text starts
        cursor.movePosition(
            QtGui.QTextCursor.Right, QtGui.QTextCursor.MoveAnchor, left_pos
        )
        cursor.movePosition(QtGui.QTextCursor.Right, QtGui.QTextCursor.KeepAnchor, 1)

        # Toggle the comment character on/off
        if cursor.selectedText() != "#":
            cursor.movePosition(QtGui.QTextCursor.Left, QtGui.QTextCursor.MoveAnchor, 1)
            cursor.insertText("# ")
        else:
            # Remove the comment character
            if cursor.selectedText() == "#":
                cursor.removeSelectedText()

            cursor.movePosition(
                QtGui.QTextCursor.Right, QtGui.QTextCursor.KeepAnchor, 1
            )

            # Also remove an extra space if there is one
            if cursor.selectedText() == " ":
                cursor.removeSelectedText()

    def toggle_comment(self):
        """
        High level method to comment or uncomment a single line,
        or block of lines.
        """

        # See if there is a selection range
        sel_range = self.get_selection_range()
        if sel_range[0] == 0 and sel_range[1] == 0:
            # Get the text of the line
            cursor = self.textCursor()
            cursor.movePosition(QtGui.QTextCursor.StartOfLine)
            line_text = cursor.block().text()

            # Skip blank lines
            if line_text == "":
                return

            # Find the first non-whitespace character position
            pos = 0
            while pos < len(line_text) and line_text[pos].isspace():
                pos += 1

            # Move right by pos characters to the position before text starts
            cursor.movePosition(
                QtGui.QTextCursor.Right, QtGui.QTextCursor.MoveAnchor, pos
            )
            cursor.movePosition(
                QtGui.QTextCursor.Right, QtGui.QTextCursor.KeepAnchor, 1
            )

            # If we have a single line comment, remove it
            if cursor.selectedText() == "#":
                cursor.removeSelectedText()

                # Remove any whitespace after the comment character
                cursor.movePosition(
                    QtGui.QTextCursor.Right, QtGui.QTextCursor.KeepAnchor, 1
                )
                if cursor.selectedText() == " ":
                    cursor.removeSelectedText()
            else:
                # Insert the comment characters
                cursor.movePosition(
                    QtGui.QTextCursor.Left, QtGui.QTextCursor.MoveAnchor, 1
                )
                cursor.insertText("# ")
        else:
            # Make the selected line numbers 1-based
            sel_start = sel_range[0]
            sel_end = sel_range[1]
            cursor = self.textCursor()

            # Select the text block
            block = self.document().findBlockByNumber(sel_start)
            cursor.setPosition(block.position())
            last_block = self.document().findBlockByNumber(sel_end)
            end_pos = last_block.position() + last_block.length() - 1
            cursor.setPosition(end_pos, QtGui.QTextCursor.KeepAnchor)

            # Find the left-most position to put the comment at
            leftmost_pos = 99999
            comment_line_found = False
            non_comment_line_found = False
            blank_lines = []
            # Step through all of the selected lines and toggle their comments
            for i in range(sel_start, sel_end + 1):
                # Set the cursor to the current line number
                block = self.document().findBlockByNumber(i)
                cursor.setPosition(block.position())
                cursor.movePosition(cursor.EndOfBlock, cursor.KeepAnchor)
                line_text = cursor.selectedText()

                # Make sure the line is not blank
                if line_text == "":
                    blank_lines.append(i)
                    continue

                if line_text.strip()[0] == "#":
                    comment_line_found = True
                else:
                    non_comment_line_found = True

                # Find the first non-whitespace character position
                pos = 0
                while pos < len(line_text) and line_text[pos].isspace():
                    pos += 1

                # Save the left-most position
                if pos < leftmost_pos:
                    leftmost_pos = pos

            # Step through all of the selected lines and toggle their comments
            for i in range(sel_start, sel_end + 1):
                # If this is a blank line, do not process it
                if i in blank_lines:
                    continue

                # Set the cursor to the current line number
                block = self.document().findBlockByNumber(i)
                cursor.setPosition(block.position())

                # See if we need to comment the whole block
                if comment_line_found and non_comment_line_found:
                    # Insert the comment characters
                    cursor.insertText("# ")
                else:
                    # Move right by pos characters to the position before text starts
                    cursor.movePosition(
                        QtGui.QTextCursor.Right,
                        QtGui.QTextCursor.MoveAnchor,
                        leftmost_pos,
                    )
                    cursor.movePosition(
                        QtGui.QTextCursor.Right, QtGui.QTextCursor.KeepAnchor, 1
                    )

                    # If the line starts with a hash, uncomment it. Otherwise comment it
                    if cursor.selectedText() == "#":
                        cursor.removeSelectedText()

                        # Remove any whitespace after the comment character
                        cursor.movePosition(
                            QtGui.QTextCursor.Right, QtGui.QTextCursor.KeepAnchor, 1
                        )
                        if cursor.selectedText() == " ":
                            cursor.removeSelectedText()
                    else:
                        # Insert the comment characters
                        cursor.movePosition(
                            QtGui.QTextCursor.Left, QtGui.QTextCursor.MoveAnchor, 1
                        )
                        cursor.insertText("# ")

    def set_text(self, new_text):
        """
        Sets the text content of the editor.
        :param str new_text: Text to be set in the editor.
        """
        # Set the text in the document
        self.setPlainText(new_text)

        # Set the cursor at the end of the text
        cursor = self.textCursor()
        cursor.movePosition(cursor.End)
        self.setTextCursor(cursor)

        # Set the document to be modified
        self.document().setModified(True)

    def set_text_from_file(self, file_name):
        """
        Allows the editor text to be set from a file.
        :param str file_name: Full path of the file to be loaded into the editor.
        """

        self._filename = file_name

        # Load the text into the text field
        with open(file_name, "r", encoding="utf-8") as file:
            file_content = file.read()

            self.setPlainText(file_content)

    def get_text_with_eol(self):
        """
        Returns a string representing the full text in the editor.
        """
        return self.toPlainText()

    def line_number_area_width(self):
        digits = 1
        max_num = max(1, self.blockCount())
        while max_num >= 10:
            max_num *= 0.1
            digits += 1

        space = 30 + self.fontMetrics().width("9") * digits
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
                    painter.drawText(
                        0, int(top), width, height, QtCore.Qt.AlignRight, number
                    )

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
