import os
import spyder.utils.encoding
from modulefinder import ModuleFinder

from spyder.plugins.editor.widgets.codeeditor import CodeEditor
from PyQt5.QtCore import pyqtSignal, QFileSystemWatcher, QTimer
from PyQt5.QtWidgets import QAction, QFileDialog, QApplication
from PyQt5.QtGui import QFontDatabase, QTextCursor
from path import Path

import sys

from pyqtgraph.parametertree import Parameter

from ..mixins import ComponentMixin
from ..utils import get_save_filename, get_open_filename, confirm

from ..icons import icon


class Editor(CodeEditor, ComponentMixin):

    name = "Code Editor"

    # This signal is emitted whenever the currently-open file changes and
    # autoreload is enabled.
    triggerRerender = pyqtSignal(bool)
    sigFilenameChanged = pyqtSignal(str)

    preferences = Parameter.create(
        name="Preferences",
        children=[
            {"name": "Font size", "type": "int", "value": 12},
            {"name": "Autoreload", "type": "bool", "value": False},
            {"name": "Autoreload delay", "type": "int", "value": 50},
            {
                "name": "Autoreload: watch imported modules",
                "type": "bool",
                "value": False,
            },
            {"name": "Line wrap", "type": "bool", "value": False},
            {
                "name": "Color scheme",
                "type": "list",
                "values": ["Spyder", "Monokai", "Zenburn"],
                "value": "Spyder",
            },
            {"name": "Maximum line length", "type": "int", "value": 88},
        ],
    )

    EXTENSIONS = "py"

    # Tracks whether or not the document was saved from the Spyder editor vs an external editor
    was_modified_by_self = False

    def __init__(self, parent=None):

        self._watched_file = None

        super(Editor, self).__init__(parent)
        ComponentMixin.__init__(self)

        self.setup_editor(
            linenumbers=True,
            markers=True,
            edge_line=self.preferences["Maximum line length"],
            tab_mode=False,
            show_blanks=True,
            font=QFontDatabase.systemFont(QFontDatabase.FixedFont),
            language="Python",
            filename="",
        )

        self._actions = {
            "File": [
                QAction(
                    icon("new"), "New", self, shortcut="ctrl+N", triggered=self.new
                ),
                QAction(
                    icon("open"), "Open", self, shortcut="ctrl+O", triggered=self.open
                ),
                QAction(
                    icon("save"), "Save", self, shortcut="ctrl+S", triggered=self.save
                ),
                QAction(
                    icon("save_as"),
                    "Save as",
                    self,
                    shortcut="ctrl+shift+S",
                    triggered=self.save_as,
                ),
                QAction(
                    icon("autoreload"),
                    "Automatic reload and preview",
                    self,
                    triggered=self.autoreload,
                    checkable=True,
                    checked=False,
                    objectName="autoreload",
                ),
            ]
        }

        for a in self._actions.values():
            self.addActions(a)

        self._fixContextMenu()

        # autoreload support
        self._file_watcher = QFileSystemWatcher(self)
        # we wait for 50ms after a file change for the file to be written completely
        self._file_watch_timer = QTimer(self)
        self._file_watch_timer.setInterval(self.preferences["Autoreload delay"])
        self._file_watch_timer.setSingleShot(True)
        self._file_watcher.fileChanged.connect(
            lambda val: self._file_watch_timer.start()
        )
        self._file_watch_timer.timeout.connect(self._file_changed)

        self.updatePreferences()

    def _fixContextMenu(self):

        menu = self.menu

        menu.removeAction(self.run_cell_action)
        menu.removeAction(self.run_cell_and_advance_action)
        menu.removeAction(self.run_selection_action)
        menu.removeAction(self.re_run_last_cell_action)

    def updatePreferences(self, *args):

        self.set_color_scheme(self.preferences["Color scheme"])

        font = self.font()
        font.setPointSize(self.preferences["Font size"])
        self.set_font(font)

        self.findChild(QAction, "autoreload").setChecked(self.preferences["Autoreload"])

        self._file_watch_timer.setInterval(self.preferences["Autoreload delay"])

        self.toggle_wrap_mode(self.preferences["Line wrap"])

        # Update the edge line (maximum line length)
        self.edge_line.set_enabled(True)
        self.edge_line.set_columns(self.preferences["Maximum line length"])

        self._clear_watched_paths()
        self._watch_paths()

    def confirm_discard(self):

        if self.modified:
            rv = confirm(
                self,
                "Please confirm",
                "Current document is not saved - do you want to continue?",
            )
        else:
            rv = True

        return rv

    def new(self):

        if not self.confirm_discard():
            return

        self.set_text("")
        self.filename = ""
        self.reset_modified()

    def open(self):

        if not self.confirm_discard():
            return

        curr_dir = Path(self.filename).absolute().dirname()
        fname = get_open_filename(self.EXTENSIONS, curr_dir)
        if fname != "":
            self.load_from_file(fname)

    def load_from_file(self, fname):

        self.set_text_from_file(fname)
        self.filename = fname
        self.reset_modified()

    def save(self):
        """
        Saves the current document to the current filename if it exists, otherwise it triggers a
        save-as dialog.
        """

        if self._filename != "":
            with open(self._filename, "w", encoding="utf-8") as f:
                f.write(self.toPlainText())

            # Let the editor and the rest of the app know that the file is no longer dirty
            self.reset_modified()

            self.was_modified_by_self = True

        else:
            self.save_as()

    def save_as(self):

        fname = get_save_filename(self.EXTENSIONS)
        if fname != "":
            with open(fname, "w", encoding="utf-8") as f:
                f.write(self.toPlainText())
                self.filename = fname

            self.reset_modified()

    def toggle_comment(self):
        """
        Allows us to mark the document as modified when the user toggles a comment.
        """
        super(Editor, self).toggle_comment()
        self.document().setModified(True)

    def _update_filewatcher(self):
        if self._watched_file and (
            self._watched_file != self.filename or not self.preferences["Autoreload"]
        ):
            self._clear_watched_paths()
            self._watched_file = None
        if (
            self.preferences["Autoreload"]
            and self.filename
            and self.filename != self._watched_file
        ):
            self._watched_file = self._filename
            self._watch_paths()

    @property
    def filename(self):
        return self._filename

    @filename.setter
    def filename(self, fname):
        self._filename = fname
        self._update_filewatcher()
        self.sigFilenameChanged.emit(fname)

    def _clear_watched_paths(self):
        paths = self._file_watcher.files()
        if paths:
            self._file_watcher.removePaths(paths)

    def _watch_paths(self):
        if Path(self._filename).exists():
            self._file_watcher.addPath(self._filename)
            if self.preferences["Autoreload: watch imported modules"]:
                module_paths = self.get_imported_module_paths(self._filename)
                if module_paths:
                    self._file_watcher.addPaths(module_paths)

    # callback triggered by QFileSystemWatcher
    def _file_changed(self):
        # neovim writes a file by removing it first so must re-add each time
        self._watch_paths()

        # Save the current cursor position and selection
        cursor = self.textCursor()
        cursor_position = cursor.position()
        anchor_position = cursor.anchor()

        # Save the current scroll position
        vertical_scroll_pos = self.verticalScrollBar().value()
        horizontal_scroll_pos = self.horizontalScrollBar().value()

        # Block signals to avoid reset issues
        self.blockSignals(True)

        # Read the contents of the file into a string
        with open(self._filename, "r", encoding="utf-8") as f:
            file_contents = f.read()

            # Insert new text while preserving history
            cursor = self.textCursor()
            cursor.select(QTextCursor.Document)
            cursor.insertText(file_contents)

            # The editor will not always update after a text insertion, so we force it
            QApplication.processEvents()

        # Stop blocking signals
        self.blockSignals(False)

        self.document().setModified(True)

        # Undo has to be backed up one step to compensate for the text insertion
        if self.was_modified_by_self:
            self.document().undo()
            self.was_modified_by_self = False

        # Restore the cursor position and selection
        cursor.setPosition(anchor_position)
        cursor.setPosition(cursor_position, QTextCursor.KeepAnchor)
        self.setTextCursor(cursor)

        # Restore the scroll position
        self.verticalScrollBar().setValue(vertical_scroll_pos)
        self.horizontalScrollBar().setValue(horizontal_scroll_pos)

        # Reset the dirty state and trigger a 3D render
        self.reset_modified()
        self.triggerRerender.emit(True)

    # Turn autoreload on/off.
    def autoreload(self, enabled):
        self.preferences["Autoreload"] = enabled
        self._update_filewatcher()

    def reset_modified(self):

        self.document().setModified(False)

    @property
    def modified(self):

        return self.document().isModified()

    def saveComponentState(self, store):

        if self.filename != "":
            store.setValue(self.name + "/state", self.filename)

    def restoreComponentState(self, store):

        filename = store.value(self.name + "/state")

        if filename and self.filename == "":
            try:
                self.load_from_file(filename)
            except IOError:
                self._logger.warning(f"could not open {filename}")

    def get_imported_module_paths(self, module_path):

        finder = ModuleFinder([os.path.dirname(module_path)])
        imported_modules = []

        try:
            finder.run_script(module_path)
        except SyntaxError as err:
            self._logger.warning(f"Syntax error in {module_path}: {err}")
        except Exception as err:
            # The module finder has trouble when CadQuery is imported in the top level script and in
            # imported modules. The warning about it can be ignored.
            if "cadquery" not in finder.badmodules or (
                "cadquery" in finder.badmodules and len(finder.badmodules) > 1
            ):
                self._logger.warning(
                    f"Cannot determine imported modules in {module_path}: {type(err).__name__} {err}"
                )
        else:
            for module_name, module in finder.modules.items():
                if module_name != "__main__":
                    path = getattr(module, "__file__", None)
                    if path is not None and os.path.isfile(path):
                        imported_modules.append(path)

        return imported_modules


if __name__ == "__main__":

    from PyQt5.QtWidgets import QApplication

    app = QApplication(sys.argv)
    editor = Editor()
    editor.show()

    sys.exit(app.exec_())
