import json
import os
import tempfile

import requests
from PyQt5.QtGui import QDesktopServices
from PyQt5.QtCore import QRunnable, QThreadPool, pyqtSignal, QObject
from PyQt5.QtWidgets import (
    QComboBox,
    QDialog,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QTextBrowser,
    QVBoxLayout,
)
from ..cq_utils import export

PRESETS = [
    {
        "name": "PCBWay",
        "url": "https://www.pcbway.com/common/CadQueryUpFile",
        "file_type": "step",
        "result_key": "redirect",
    },
]


class UploadSignals(QObject):
    finished = pyqtSignal(str)  # Emits formatted JSON or error message
    error = pyqtSignal(str)


class _UploadRunnable(QRunnable):
    def __init__(self, url, file_path, file_type):
        super().__init__()
        self.url = url
        self.file_path = file_path
        self.file_type = file_type
        self.signals = UploadSignals()

    def run(self):
        try:
            with open(self.file_path, "rb") as f:
                resp = requests.post(
                    self.url,
                    files={"file": (f"model.{self.file_type}", f)},
                )
            resp.raise_for_status()
            self.signals.finished.emit(json.dumps(resp.json(), indent=2))
        except Exception as e:
            self.signals.error.emit(str(e))


class UploadDialog(QDialog):
    def __init__(self, parent, object_tree, editor, selected_shapes=None):
        super().__init__(parent, windowTitle="Upload Model")
        self._object_tree = object_tree
        self._editor = editor
        self._active_preset = None
        self._selected_shapes = selected_shapes or []

        # URL row
        url_layout = QHBoxLayout()
        url_layout.addWidget(QLabel("URL:"))
        self._url_input = QLineEdit()
        url_layout.addWidget(self._url_input)

        # Preset buttons
        preset_layout = QHBoxLayout()
        preset_layout.addWidget(QLabel("Presets:"))
        for preset in PRESETS:
            btn = QPushButton(preset["name"])
            btn.clicked.connect(lambda checked, p=preset: self._apply_preset(p))
            preset_layout.addWidget(btn)
        preset_layout.addStretch()

        # File type selector
        type_layout = QHBoxLayout()
        type_layout.addWidget(QLabel("File type:"))
        self._type_combo = QComboBox()
        self._type_combo.addItems(["STEP", "STL", "CadQuery"])
        type_layout.addWidget(self._type_combo)
        type_layout.addStretch()

        # Upload button
        self._upload_btn = QPushButton("Upload")
        self._upload_btn.clicked.connect(self._do_upload)

        # Response area
        self._response = QTextBrowser(placeholderText="Response will appear here...")
        self._response.setOpenLinks(False)
        self._response.anchorClicked.connect(lambda url: QDesktopServices.openUrl(url))

        layout = QVBoxLayout()
        layout.addLayout(url_layout)
        layout.addLayout(preset_layout)
        layout.addLayout(type_layout)
        layout.addWidget(self._upload_btn)
        layout.addWidget(self._response)
        self.setLayout(layout)
        self.resize(500, 400)

    def _apply_preset(self, preset):
        self._url_input.setText(preset["url"])
        self._type_combo.setCurrentText(preset["file_type"].upper())
        self._active_preset = preset

    def _do_upload(self):
        url = self._url_input.text().strip()
        if not url:
            self._response.setPlainText("Error: no URL entered.")
            return

        file_type = self._type_combo.currentText()
        self._upload_btn.setEnabled(False)
        self._response.setPlainText("Uploading...")

        if file_type == "CadQuery":
            tmp = tempfile.NamedTemporaryFile(suffix=".py", delete=False, mode="w")
            tmp.write(self._editor.toPlainText())
            tmp.close()
            self._run_upload(url, tmp.name, "py")
            return
        else:
            # Prefer selected item, fall back to add top-level CQ objects
            shapes = (
                self._selected_shapes
                if self._selected_shapes
                else [
                    self._object_tree.CQ.child(i).shape
                    for i in range(self._object_tree.CQ.childCount())
                ]
            )

        # If the user did not select an object, let them know
        if not shapes:
            self._response.setPlainText("ERROR: Please select a model to upload.")
            self._upload_btn.setEnabled(True)
            return

        # Assume that only one object is selected and upload it
        cur_type = file_type.lower()
        tmp = tempfile.NamedTemporaryFile(suffix=f".{cur_type}", delete=False)
        tmp.close()
        export(shapes, cur_type, tmp.name)
        self._run_upload(url, tmp.name, cur_type)

    def _run_upload(self, url, file_path, file_type):
        runnable = _UploadRunnable(url, file_path, file_type)
        runnable.signals.finished.connect(lambda msg: self._on_done(file_path, msg))
        runnable.signals.error.connect(
            lambda msg: self._on_done(file_path, f"Error: {msg}")
        )
        QThreadPool.globalInstance().start(runnable)

    def _on_done(self, file_path, message):
        os.unlink(file_path)
        try:
            data = json.loads(message)
            result_key = (
                self._active_preset.get("result_key") if self._active_preset else None
            )
            if result_key and result_key in data:
                link = data[result_key]
                name = self._active_preset.get("name", "unknown service")
                self._response.setHtml(
                    f"<p>Click the link below to see your model on {name}'s website.</p>"
                    f'<a href="{link}">{link}</a>'
                )
            else:
                self._response.setPlainText(message)
        except json.JSONDecodeError:
            self._response.setPlainText(message)
        self._upload_btn.setEnabled(True)
