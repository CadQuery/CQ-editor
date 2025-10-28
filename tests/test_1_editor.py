import pytest
from PyQt5.QtCore import QSize, Qt
from cq_editor.__main__ import MainWindow

base_editor_text = """import cadquery as cq
result = cq.Workplane().box(10, 10, 10)
"""


@pytest.fixture
def main(qtbot, mocker):
    win = MainWindow()
    win.show()

    qtbot.addWidget(win)

    editor = win.components["editor"]

    return qtbot, win


def test_size_hint(main):
    """
    Tests the ability to get the size hit from the code editor widget.
    """
    qtbot, win = main

    editor = win.components["editor"]
    size_hint = editor.sizeHint()

    assert size_hint == QSize(256, 192)


def test_clear_selection(main):
    """
    Tests the ability to clear selected text.
    """
    qtbot, win = main

    editor = win.components["editor"]

    # Set a block of text and make sure it is visible
    editor.set_text(base_editor_text)
    editor.document().setModified(False)
    qtbot.wait(100)
    assert editor.get_text_with_eol() == base_editor_text

    # Remove all the text and make sure it was removed
    editor.selectAll()
    cursor = editor.textCursor()
    cursor.removeSelectedText()
    editor.setTextCursor(cursor)
    editor.document().setModified(False)
    qtbot.wait(100)
    assert editor.get_text_with_eol() == ""


def test_get_selection_range(main):
    qtbot, win = main

    editor = win.components["editor"]

    # Set a block of text and make sure it is visible
    editor.set_text(base_editor_text)
    editor.document().setModified(False)
    qtbot.wait(100)
    assert editor.get_text_with_eol() == base_editor_text

    # Select all the text and get the selection range
    editor.selectAll()
    selection_range = editor.get_selection_range()


def test_insert_remove_line_start(main):
    """
    Tests the ability to remove and insert characters from/to the beginning of a line.
    """
    qtbot, win = main

    editor = win.components["editor"]

    # Set a block of text and make sure it is visible
    editor.set_text(base_editor_text)
    editor.insert_line_start("# ", 0)
    editor.document().setModified(False)
    qtbot.wait(100)
    assert editor.get_text_with_eol() == "# " + base_editor_text

    # Remove the comment character from the line
    editor.remove_line_start("# ", 0)
    editor.document().setModified(False)
    qtbot.wait(100)
    assert editor.get_text_with_eol() == base_editor_text


def test_indent_unindent(main):
    """
    Check to make sure that indent and un-indent work properly.
    """
    qtbot, win = main

    editor = win.components["editor"]

    # Set the base text
    editor.set_text(base_editor_text)
    qtbot.wait(100)

    # Indent the text and check
    editor.selectAll()
    qtbot.keyClick(editor, Qt.Key_Tab)
    editor.document().setModified(False)
    qtbot.wait(250)
    assert editor.get_text_with_eol() != base_editor_text

    # Unindent the code with a direct method call and check
    editor.selectAll()
    start_line, end_line = editor.get_selection_range()
    # +1 here to compesate for how black wants the multi-line string
    editor.undo_indent(list(range(start_line, end_line + 1)))
    editor.document().setModified(False)
    qtbot.wait(2500)
    assert editor.get_text_with_eol() == base_editor_text

    # Indent the code again with a direct method call and check
    editor.selectAll()
    start_line, end_line = editor.get_selection_range()
    editor.do_indent(list(range(start_line, end_line)))
    editor.document().setModified(False)
    qtbot.wait(250)
    assert editor.get_text_with_eol() != base_editor_text

    # Unindent the code again with a keystroke
    editor.selectAll()
    qtbot.keyClick(editor, Qt.Key_Backtab)
    editor.document().setModified(False)
    qtbot.wait(250)
    assert editor.get_text_with_eol() == base_editor_text
