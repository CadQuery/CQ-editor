"""
AI Chat Assistant widget for CQ-editor.

Provides a dockable panel with:
  - Chat history display
  - Prompt input box
  - Async LLM calls (OpenAI-compatible API) via QThread
  - Context injection: sends the current editor code along with the prompt
  - One-click "Insert & Run" to push LLM-generated code into the editor

All imports of openai are lazy/guarded so CQ-editor starts fine without it.
Install the optional dependency with:  pip install openai
"""

from __future__ import annotations

from PyQt5.QtCore import QObject, QThread, pyqtSignal, pyqtSlot, Qt
from PyQt5.QtWidgets import (
    QWidget,
    QVBoxLayout,
    QHBoxLayout,
    QTextEdit,
    QLineEdit,
    QPushButton,
    QLabel,
    QSizePolicy,
    QMessageBox,
    QScrollBar,
)
from PyQt5.QtGui import QColor, QTextCharFormat, QFont

from pyqtgraph.parametertree import Parameter


# ---------------------------------------------------------------------------
# Background worker
# ---------------------------------------------------------------------------

class _LLMWorker(QObject):
    """Runs the blocking LLM API call in a QThread."""

    finished = pyqtSignal(str)      # emits the assistant reply
    error    = pyqtSignal(str)      # emits a human-readable error message

    def __init__(self, api_key: str, base_url: str, model: str,
                 messages: list[dict], parent=None):
        super().__init__(parent)
        self._api_key  = api_key
        self._base_url = base_url
        self._model    = model
        self._messages = messages

    @pyqtSlot()
    def run(self):
        try:
            from openai import OpenAI  # lazy import
        except ImportError:
            self.error.emit(
                "The 'openai' package is not installed.\n"
                "Run:  pip install openai"
            )
            return

        try:
            client = OpenAI(
                api_key=self._api_key,
                base_url=self._base_url or None,
            )
            response = client.chat.completions.create(
                model=self._model,
                messages=self._messages,
            )
            reply = response.choices[0].message.content or ""
            self.finished.emit(reply)
        except Exception as exc:  # noqa: BLE001
            self.error.emit(str(exc))


# ---------------------------------------------------------------------------
# Main widget
# ---------------------------------------------------------------------------

SYSTEM_PROMPT = (
    "You are an expert CadQuery (Python) programmer.\n"
    "When the user asks you to create or modify a 3-D model, respond with "
    "a complete, valid CadQuery Python script and NOTHING else — no markdown "
    "fences, no explanations, just runnable code.\n"
    "The user's current script is provided as context under the heading "
    "'CURRENT SCRIPT'. Modify it as requested, or start fresh if they ask "
    "for something new."
)


class AIChatWidget(QWidget):
    """Dockable AI Chat panel for CQ-editor."""

    name = "AI Assistant"

    # Emitted when the LLM returns code that should be loaded into the editor.
    # Connected to Editor.set_text() + Debugger.render() in main_window.py
    insert_code = pyqtSignal(str)

    preferences = Parameter.create(
        name="AI Assistant",
        children=[
            {
                "name": "Enabled",
                "type": "bool",
                "value": False,
                "tip": "Enable the AI Chat Assistant panel",
            },
            {
                "name": "Provider / Base URL",
                "type": "str",
                "value": "https://api.openai.com/v1",
                "tip": (
                    "OpenAI-compatible base URL. "
                    "Use https://api.openai.com/v1 for OpenAI, "
                    "https://openrouter.ai/api/v1 for OpenRouter, etc."
                ),
            },
            {
                "name": "Model",
                "type": "str",
                "value": "gpt-4o",
                "tip": "Model identifier, e.g. gpt-4o, claude-sonnet-4-5, o3",
            },
            {
                "name": "API Key",
                "type": "str",
                "value": "",
                "tip": "Your API key. Stored in local preferences (not sent anywhere else).",
            },
            {
                "name": "Auto-run after insert",
                "type": "bool",
                "value": True,
                "tip": "Automatically execute the script after inserting LLM code",
            },
        ],
    )

    def __init__(self, parent=None, editor=None, debugger=None):
        super().__init__(parent)
        self._editor   = editor    # cq_editor.widgets.editor.Editor instance
        self._debugger = debugger  # cq_editor.widgets.debugger.Debugger instance
        self._history: list[dict] = []   # OpenAI message history
        self._thread: QThread | None = None
        self._worker: _LLMWorker | None = None
        self._setup_ui()

    # ------------------------------------------------------------------
    # UI construction
    # ------------------------------------------------------------------

    def _setup_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(4, 4, 4, 4)
        layout.setSpacing(4)

        # ---- header
        header = QLabel("<b>🤖 AI CAD Assistant</b>")
        header.setAlignment(Qt.AlignCenter)
        layout.addWidget(header)

        # ---- chat history
        self.chat_display = QTextEdit()
        self.chat_display.setReadOnly(True)
        self.chat_display.setFont(QFont("Monospace", 9))
        self.chat_display.setMinimumHeight(200)
        layout.addWidget(self.chat_display, stretch=1)

        # ---- input row
        input_row = QHBoxLayout()
        self.prompt_input = QLineEdit()
        self.prompt_input.setPlaceholderText(
            "Describe what you want to model or change…"
        )
        self.prompt_input.returnPressed.connect(self._send)
        input_row.addWidget(self.prompt_input, stretch=1)

        self.send_btn = QPushButton("Send")
        self.send_btn.setFixedWidth(60)
        self.send_btn.clicked.connect(self._send)
        input_row.addWidget(self.send_btn)
        layout.addLayout(input_row)

        # ---- action buttons
        btn_row = QHBoxLayout()

        self.insert_btn = QPushButton("Insert & Run")
        self.insert_btn.setToolTip(
            "Copy the last LLM code block into the editor and run it"
        )
        self.insert_btn.setEnabled(False)
        self.insert_btn.clicked.connect(self._insert_last_code)
        btn_row.addWidget(self.insert_btn)

        self.clear_btn = QPushButton("Clear Chat")
        self.clear_btn.clicked.connect(self._clear)
        btn_row.addWidget(self.clear_btn)

        layout.addLayout(btn_row)

        # ---- status
        self.status_label = QLabel("")
        self.status_label.setAlignment(Qt.AlignCenter)
        layout.addWidget(self.status_label)

        self._last_code: str = ""

    # ------------------------------------------------------------------
    # Helpers to read live preferences
    # ------------------------------------------------------------------

    def _pref(self, name: str):
        return self.preferences[name]

    def _api_key(self) -> str:
        return self._pref("API Key").strip()

    def _base_url(self) -> str:
        return self._pref("Provider / Base URL").strip()

    def _model(self) -> str:
        return self._pref("Model").strip()

    def _auto_run(self) -> bool:
        return self._pref("Auto-run after insert")

    # ------------------------------------------------------------------
    # Slots
    # ------------------------------------------------------------------

    @pyqtSlot()
    def _send(self):
        prompt = self.prompt_input.text().strip()
        if not prompt:
            return

        if not self._api_key():
            QMessageBox.warning(
                self,
                "API Key Missing",
                "Please enter your API key in\n"
                "Edit → Preferences → AI Assistant → API Key",
            )
            return

        if self._thread and self._thread.isRunning():
            self.status_label.setText("Waiting for previous response…")
            return

        self.prompt_input.clear()
        self._append_chat("user", prompt)

        # Build message list with optional context injection
        if not self._history:  # first message — inject system prompt
            self._history.append({"role": "system", "content": self._build_system()})

        self._history.append({"role": "user", "content": self._inject_context(prompt)})

        self._set_busy(True)
        self._run_worker(list(self._history))

    def _build_system(self) -> str:
        return SYSTEM_PROMPT

    def _inject_context(self, prompt: str) -> str:
        """Prepend current editor code as context to the user prompt."""
        if self._editor is None:
            return prompt
        try:
            current_code = self._editor.toPlainText().strip()
        except Exception:
            current_code = ""

        if not current_code:
            return prompt

        return (
            f"CURRENT SCRIPT:\n```python\n{current_code}\n```\n\n"
            f"USER REQUEST:\n{prompt}"
        )

    def _run_worker(self, messages: list[dict]):
        self._thread = QThread(self)
        self._worker = _LLMWorker(
            api_key=self._api_key(),
            base_url=self._base_url(),
            model=self._model(),
            messages=messages,
        )
        self._worker.moveToThread(self._thread)
        self._thread.started.connect(self._worker.run)
        self._worker.finished.connect(self._on_response)
        self._worker.error.connect(self._on_error)
        self._worker.finished.connect(self._thread.quit)
        self._worker.error.connect(self._thread.quit)
        self._thread.finished.connect(self._thread.deleteLater)
        self._thread.start()

    @pyqtSlot(str)
    def _on_response(self, reply: str):
        self._set_busy(False)
        self._history.append({"role": "assistant", "content": reply})
        self._append_chat("assistant", reply)
        # Try to extract a code block for the Insert button
        code = self._extract_code(reply)
        if code:
            self._last_code = code
            self.insert_btn.setEnabled(True)
        else:
            # The whole reply might be bare code
            self._last_code = reply.strip()
            self.insert_btn.setEnabled(bool(self._last_code))

    @pyqtSlot(str)
    def _on_error(self, message: str):
        self._set_busy(False)
        self._append_chat("error", message)

    @pyqtSlot()
    def _insert_last_code(self):
        if not self._last_code:
            return
        self.insert_code.emit(self._last_code)
        # Optionally trigger a re-render via debugger
        if self._auto_run() and self._debugger is not None:
            try:
                self._debugger.render()
            except Exception:
                pass
        self._append_chat("system", "✅ Code inserted into editor.")

    @pyqtSlot()
    def _clear(self):
        self._history.clear()
        self._last_code = ""
        self.insert_btn.setEnabled(False)
        self.chat_display.clear()
        self.status_label.setText("")

    # ------------------------------------------------------------------
    # Utilities
    # ------------------------------------------------------------------

    def _set_busy(self, busy: bool):
        self.send_btn.setEnabled(not busy)
        self.prompt_input.setEnabled(not busy)
        self.status_label.setText("Thinking…" if busy else "")

    def _append_chat(self, role: str, text: str):
        cursor = self.chat_display.textCursor()
        cursor.movePosition(cursor.End)

        fmt_label = QTextCharFormat()
        fmt_text  = QTextCharFormat()

        if role == "user":
            fmt_label.setForeground(QColor("#1565C0"))
            fmt_label.setFontWeight(QFont.Bold)
            label = "You: "
        elif role == "assistant":
            fmt_label.setForeground(QColor("#1B5E20"))
            fmt_label.setFontWeight(QFont.Bold)
            label = "AI:  "
        elif role == "error":
            fmt_label.setForeground(QColor("#B71C1C"))
            fmt_label.setFontWeight(QFont.Bold)
            label = "ERR: "
            fmt_text.setForeground(QColor("#B71C1C"))
        else:  # system
            fmt_label.setForeground(QColor("#4A148C"))
            label = "     "

        cursor.insertText(label, fmt_label)
        cursor.insertText(text.strip() + "\n\n", fmt_text)

        # Auto-scroll
        sb: QScrollBar = self.chat_display.verticalScrollBar()
        sb.setValue(sb.maximum())

    @staticmethod
    def _extract_code(text: str) -> str:
        """Pull the first fenced ```python ... ``` block, or ``` ... ``` block."""
        import re
        pattern = re.compile(
            r"```(?:python)?\n(.*?)```", re.DOTALL | re.IGNORECASE
        )
        m = pattern.search(text)
        return m.group(1).strip() if m else ""

    # ------------------------------------------------------------------
    # MainMixin compatibility stubs
    # ------------------------------------------------------------------

    def menuActions(self) -> dict:
        return {}

    def toolbarActions(self) -> list:
        return []
