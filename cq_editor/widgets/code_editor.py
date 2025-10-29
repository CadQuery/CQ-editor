# Much of this code was adapted from https://github.com/leixingyu/codeEditor which is under
# an MIT license
import os
from PyQt5 import QtCore, QtGui, QtWidgets
from PyQt5.QtGui import QPalette, QColor


DARK_BLUE = QtGui.QColor(118, 150, 185)


class SearchWidget(QtWidgets.QWidget):
    def __init__(self, editor):
        super(SearchWidget, self).__init__(editor)
        self.editor = editor
        self.current_match = 0
        self.total_matches = 0

        self.setup_ui()

        # This widget should initially be hidden
        self.hide()

    def setup_ui(self):
        # Horizontal layout
        layout = QtWidgets.QHBoxLayout()
        layout.setContentsMargins(5, 5, 5, 5)

        # Search input box
        self.search_input = QtWidgets.QLineEdit()
        self.search_input.setPlaceholderText("Search...")
        self.search_input.setMinimumWidth(100)
        self.search_input.textChanged.connect(self.on_search_text_changed)
        self.search_input.returnPressed.connect(self.find_next)

        # Previous button
        self.prev_button = QtWidgets.QPushButton("Prev")
        self.prev_button.clicked.connect(self.find_previous)
        self.prev_button.setEnabled(False)

        # Next button
        self.next_button = QtWidgets.QPushButton("Next")
        self.next_button.clicked.connect(self.find_next)
        self.next_button.setEnabled(False)

        # Match count label
        self.match_label = QtWidgets.QLabel("0 matches")

        # Close button
        self.close_button = QtWidgets.QPushButton("×")
        self.close_button.setMaximumSize(20, 20)
        self.close_button.clicked.connect(self.hide_search)

        # Add widgets to layout
        layout.addWidget(self.search_input)
        layout.addWidget(self.prev_button)
        layout.addWidget(self.next_button)
        layout.addWidget(self.match_label)
        layout.addWidget(self.close_button)
        self.setLayout(layout)

    def on_search_text_changed(self, text):
        """
        Called as the user types text into the search field.
        """
        if not text:
            self.clear_highlights()
            self.update_match_count(0, 0)
            self.prev_button.setEnabled(False)
            self.next_button.setEnabled(False)
            return

        self.find_all_matches(text)

    def find_all_matches(self, search_text):
        """
        Finds all the matches within the search text.
        """
        if not search_text:
            return

        # Clear any previous highlights
        self.clear_highlights()

        # Find all matches
        document = self.editor.document()
        cursor = QtGui.QTextCursor(document)
        self.matches = []

        # Find all occurrences
        while True:
            # Look for a match
            cursor = document.find(search_text, cursor)
            if cursor.isNull():
                break
            self.matches.append(cursor)

        self.total_matches = len(self.matches)

        # If there are matches make them visible to the user
        if self.total_matches > 0:
            self.current_match = 0
            self.highlight_matches()
            self.highlight_current_match()
            self.prev_button.setEnabled(True)
            self.next_button.setEnabled(True)
        else:
            self.prev_button.setEnabled(False)
            self.next_button.setEnabled(False)

        self.update_match_count(
            self.current_match + 1 if self.total_matches > 0 else 0, self.total_matches
        )

    def highlight_matches(self):
        """
        Highlights all matches to make them visible.
        """
        extra_selections = []

        for cursor in self.matches:
            selection = QtWidgets.QTextEdit.ExtraSelection()
            selection.format.setBackground(QtGui.QColor(255, 255, 0, 100))
            selection.cursor = cursor
            extra_selections.append(selection)

        self.editor.setExtraSelections(extra_selections)

    def highlight_current_match(self):
        """
        Makes the current match stand out from the others.
        """

        # If there are no matches to highlight, then skip this step
        if not self.matches or self.current_match >= len(self.matches):
            return

        # Highlight current match more than others and scroll to it
        extra_selections = []

        for i, cursor in enumerate(self.matches):
            selection = QtWidgets.QTextEdit.ExtraSelection()
            # The current match should stand out
            if i == self.current_match:
                selection.format.setBackground(QtGui.QColor(255, 165, 0))
            else:
                selection.format.setBackground(QtGui.QColor(255, 255, 0, 100))
            selection.cursor = cursor
            extra_selections.append(selection)

        self.editor.setExtraSelections(extra_selections)

        # Scroll to the current match
        self.editor.setTextCursor(self.matches[self.current_match])
        self.editor.ensureCursorVisible()

    def find_next(self):
        """
        Finds the next match.
        """

        # If there are no matches, skip this step
        if not self.matches:
            return

        self.current_match = (self.current_match + 1) % len(self.matches)
        self.highlight_current_match()
        self.update_match_count(self.current_match + 1, self.total_matches)

    def find_previous(self):
        """
        Finds the previous match.
        """

        # If there are no matches, skip this step
        if not self.matches:
            return

        self.current_match = (self.current_match - 1) % len(self.matches)
        self.highlight_current_match()
        self.update_match_count(self.current_match + 1, self.total_matches)

    def update_match_count(self, current, total):
        """
        Updates the match count for the user.
        """
        if total == 0:
            self.match_label.setText("0 matches")
        else:
            self.match_label.setText(f"{current} of {total}")

    def clear_highlights(self):
        """
        Clears all of the find highlights.
        """
        self.editor.setExtraSelections([])
        self.matches = []

    def show_search(self):
        """
        Makes the search dialog visible.
        """
        self.show()

        # Make sure the user can start typing search text right away
        self.search_input.setFocus()
        self.search_input.selectAll()

        self.position_widget()

        # If there is already text in the search box, trigger the search
        if self.search_input.text():
            self.find_all_matches(self.search_input.text())

    def hide_search(self):
        """
        Hides the search dialog again.
        """
        self.hide()
        self.clear_highlights()
        self.editor.setFocus()

    def position_widget(self):
        """
        Makes sure that the search widget gets placed in the right location
        in the window.
        """

        # Top-right corner of the editor
        editor_rect = self.editor.geometry()
        widget_width = 400
        widget_height = 40
        x = editor_rect.width() - widget_width - 20
        y = 10

        # Set the size of the widget and bring it to the front
        self.setGeometry(x, y, widget_width, widget_height)
        self.raise_()


class LineNumberArea(QtWidgets.QWidget):
    def __init__(self, editor):
        super(LineNumberArea, self).__init__(editor)
        self._code_editor = editor

    def sizeHint(self):
        return QtCore.QSize(self._code_editor.line_number_area_width(), 0)

    def mousePressEvent(self, event):
        """
        Handles mouse clicks to add/remove breakpoints.
        """
        if event.button() == QtCore.Qt.LeftButton:
            # Calculate which line was clicked
            line_number = self.get_line_number_from_position(event.pos())
            if line_number is not None:
                self._code_editor.toggle_breakpoint(line_number)

    def get_line_number_from_position(self, pos):
        """
        Convert mouse position to line number.
        """

        # Get the first visible block
        block = self._code_editor.firstVisibleBlock()
        block_number = block.blockNumber()
        offset = self._code_editor.contentOffset()
        top = self._code_editor.blockBoundingGeometry(block).translated(offset).top()

        # Find which block the click position corresponds to
        while block.isValid():
            bottom = top + self._code_editor.blockBoundingRect(block).height()

            if top <= pos.y() <= bottom:
                return (
                    block_number + 1
                )  # Line numbers start at 0 internally and 1 for the user

            block = block.next()
            top = bottom
            block_number += 1

        return None

    def paintEvent(self, event):
        self._code_editor.lineNumberAreaPaintEvent(event)


class EdgeLine(QtWidgets.QWidget):
    edge_line = None
    columns = 80

    def __init__(self):
        super(QtWidgets.QWidget, self).__init__()

    def set_enabled(self, enabled_state):
        self.setEnabled = enabled_state

    def set_columns(self, number_of_columns):
        self.columns = number_of_columns


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

        # Get the selection range of lines
        start_line, end_line = self.get_selection_range()

        # If a single line is selected, make sure it is the only line in the range
        if start_line == 0 and end_line == 0:
            # Insert a tab at the current cursor location
            cursor = self.textCursor()
            cursor.insertText("    ")

            # Make sure that no lines are changed
            lines = []
        # Multiple lines have been selected
        else:
            lines = range(start_line, end_line + 1)

        # Walk through the selected lines and tab them (with 4 spaces)
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

        self.search_widget = SearchWidget(self)

        self._filename = ""

    def keyPressEvent(self, event):
        # Handle Ctrl+F for search
        if (
            event.modifiers() == QtCore.Qt.ControlModifier
            and event.key() == QtCore.Qt.Key_F
        ):
            self.search_widget.show_search()
            return

        # Handle F3 for find next (when search widget is visible)
        if event.key() == QtCore.Qt.Key_F3 and self.search_widget.isVisible():
            if event.modifiers() == QtCore.Qt.AltModifier:
                self.search_widget.find_previous()  # Alt+F3 for previous
            else:
                self.search_widget.find_next()  # F3 for next
            return

        # Handle Escape to close search
        if event.key() == QtCore.Qt.Key_Escape and self.search_widget.isVisible():
            self.search_widget.hide_search()
            return

        # Call parent for other keys
        super(CodeEditor, self).keyPressEvent(event)

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
        self.setFont(new_font)

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
                    line_number = block_number + 1

                    # Draw the breakpoint dot, if there is a breakpoint on this line
                    if self.line_has_breakpoint(line_number):
                        painter.setBrush(
                            QtGui.QBrush(QtGui.QColor(255, 0, 0))
                        )  # Red circle
                        painter.setPen(QtGui.QPen(QtGui.QColor(150, 0, 0)))
                        circle_size = 10
                        circle_x = 5
                        circle_y = (
                            int(top)
                            + (self.fontMetrics().height() - circle_size - 2) // 2
                        )
                        painter.drawEllipse(
                            circle_x, circle_y, circle_size, circle_size
                        )

                    # Draw the line number
                    number = str(line_number)
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

    def toggle_breakpoint(self, line_number):
        """
        Toggle breakpoint on/off for a given line number.
        """
        if line_number in self.debugger.breakpoints:
            self.debugger.breakpoints.remove(line_number)
        else:
            self.debugger.breakpoints.append(line_number)

        # Repaint the line number area
        self.line_number_area.update()

    def line_has_breakpoint(self, line_number):
        """
        Checks if a line has a breakpoint.
        """
        return line_number in self.debugger.breakpoints

    def paintEvent(self, event):
        """
        Overrides the default paint event so that we can draw the line length indicator.
        """

        # Call the parent's paintEvent first to render the text
        super(CodeEditor, self).paintEvent(event)

        painter = QtGui.QPainter(self.viewport())
        try:
            # Calculate the x position for the line
            font_metrics = self.fontMetrics()
            char_width = font_metrics.width("M")  # Use 'M' for average character width
            x_position = self.edge_line.columns * char_width + self.contentOffset().x()

            # Only draw if the line is within the visible area
            if 0 <= x_position <= self.viewport().width():
                # Set the pen color (light gray is common)
                painter.setPen(
                    QtGui.QPen(QtGui.QColor(200, 200, 200), 1, QtCore.Qt.SolidLine)
                )

                # Draw the vertical line from top to bottom of the viewport
                painter.drawLine(
                    int(x_position), 0, int(x_position), self.viewport().height()
                )
        finally:
            painter.end()
