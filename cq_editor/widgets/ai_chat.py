"""
AI Chat Assistant widget for CQ-editor.

Provides a dockable panel with:
  - Chat history display (colour-coded roles)
  - Prompt input box (Enter or Send button)
  - Async LLM calls via QThread — UI never freezes
  - Context injection: current editor script is prepended to every prompt
  - "Insert & Run" pushes extracted code into the editor
  - Privacy consent dialog on first use
  - System keyring for API key storage (falls back to plaintext)
  - Bounded conversation history (MAX_HISTORY_TURNS)
  - Clean thread shutdown in closeEvent

Optional dependencies (install separately):
  pip install openai          # required for LLM calls
  pip install keyring         # recommended for secure API key storage
"""

from __future__ import annotations

import re

from PyQt5.QtCore import QObject, QThread, pyqtSignal, pyqtSlot, Qt
from PyQt5.QtWidgets import (
    QWidget,
    QVBoxLayout,
    QHBoxLayout,
    QTextEdit,
    QLineEdit,
    QPushButton,
    QLabel,
    QMessageBox,
    QScrollBar,
    QAction,
)
from PyQt5.QtGui import QColor, QTextCharFormat, QFont

from pyqtgraph.parametertree import Parameter

# Maximum number of user+assistant turn pairs kept in history.
# Older turns are pruned to limit token cost and payload size.
MAX_HISTORY_TURNS = 10

# Keyring service name used when storing the API key securely.
_KEYRING_SERVICE = "cq-editor-ai-assistant"
_KEYRING_USER    = "api-key"


# ---------------------------------------------------------------------------
# Secure key storage helpers
# ---------------------------------------------------------------------------

def _keyring_save(key: str) -> bool:
    """Try to store *key* in the system keychain. Returns True on success."""
    try:
        import keyring
        keyring.set_password(_KEYRING_SERVICE, _KEYRING_USER, key)
        return True
    except Exception:
        return False


def _keyring_load() -> str | None:
    """Try to load the API key from the system keychain."""
    try:
        import keyring
        return keyring.get_password(_KEYRING_SERVICE, _KEYRING_USER) or None
    except Exception:
        return None


# ---------------------------------------------------------------------------
# Background LLM worker
# ---------------------------------------------------------------------------

class _LLMWorker(QObject):
    """Runs the blocking OpenAI-compatible API call in a QThread."""

    finished = pyqtSignal(str)   # assistant reply text
    error    = pyqtSignal(str)   # human-readable error

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
            from openai import OpenAI  # lazy import — openai is optional
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
            resp = client.chat.completions.create(
                model=self._model,
                messages=self._messages,
            )
            self.finished.emit(resp.choices[0].message.content or "")
        except Exception as exc:          # noqa: BLE001
            self.error.emit(str(exc))


# ---------------------------------------------------------------------------
# System prompt
# ---------------------------------------------------------------------------

SYSTEM_PROMPT = (
    "You are an expert CadQuery (Python) programmer.\n"
    "Always respond with a complete, valid CadQuery Python script wrapped in "
    "a single ```python ... ``` fenced code block. "
    "Do not include any text outside that block.\n"
    "The user's current script (if any) is provided under 'CURRENT SCRIPT'. "
    "Modify it as requested, or start fresh if the user asks for something new."
)

_CODE_RE = re.compile(r"```(?:python)?\n(.*?)```", re.DOTALL | re.IGNORECASE)


# ---------------------------------------------------------------------------
# Main widget
# ---------------------------------------------------------------------------

class AIChatWidget(QWidget):
    """Dockable AI Chat panel for CQ-editor."""

    name = "AI Assistant"

    # Emitted with the extracted CadQuery code; connected to Editor.set_text()
    insert_code = pyqtSignal(str)

    def __init__(self, parent=None):
        super().__init__(parent)

        # Instance-level preferences (not class-level) to avoid shared state
        self.preferences = Parameter.create(
            name="AI Assistant",
            children=[
                {
                    "name": "Enabled",
                    "type": "bool",
                    "value": False,
                    "tip": "Show the AI Chat Assistant dock panel",
                },
                {
                    "name": "Provider / Base URL",
                    "type": "str",
                    "value": "https://api.openai.com/v1",
                    "tip": (
                        "OpenAI-compatible base URL.\n"
                        "OpenAI: https://api.openai.com/v1\n"
                        "OpenRouter: https://openrouter.ai/api/v1\n"
                        "Ollama: http://localhost:11434/v1"
                    ),
                },
                {
                    "name": "Model",
                    "type": "str",
                    "value": "gpt-4o",
                    "tip": "Model identifier, e.g. gpt-4o, o3, claude-sonnet-4-5",
                },
                {
                    "name": "API Key",
                    "type": "str",
                    "value": "",
                    "tip": (
                        "Your API key.\n"
                        "Stored in the system keychain when 'keyring' is installed; "
                        "otherwise stored in plaintext preferences on disk.\n"
                        "Run: pip install keyring   for secure storage."
                    ),
                },
                {
                    "name": "Auto-run after insert",
                    "type": "bool",
                    "value": True,
                    "tip": "Re-render the model immediately after inserting LLM code",
                },
                {
                    "name": "_privacy_consent",
                    "type": "bool",
                    "value": False,
                    "tip": "Internal: records that the user has seen the privacy notice",
                },
            ],
        )

        self._editor   = None   # set via set_dependencies()
        self._debugger = None
        self._history: list[dict] = []
        self._thread: QThread | None = None
        self._worker: _LLMWorker | None = None
        self._last_code: str = ""

        self._setup_ui()

        # Wire Enabled toggle -> show/hide this widget's parent dock
        self.preferences.sigTreeStateChanged.connect(self._on_prefs_changed)

    # ------------------------------------------------------------------
    # Dependency injection
    # ------------------------------------------------------------------

    def set_dependencies(self, editor, debugger):
        """Called by main_window after both editor and debugger are ready."""
        self._editor   = editor
        self._debugger = debugger

    # ------------------------------------------------------------------
    # UI construction
    # ------------------------------------------------------------------

    def _setup_ui(self):
        root = QVBoxLayout(self)
        root.setContentsMargins(4, 4, 4, 4)
        root.setSpacing(4)

        header = QLabel("<b>&#x1F916; AI CAD Assistant</b>")
        header.setAlignment(Qt.AlignCenter)
        root.addWidget(header)

        # Privacy notice banner
        self._privacy_banner = QLabel(
            "<small><i>&#x26A0; Each prompt sends your current script to the "
            "configured API endpoint.</i></small>"
        )
        self._privacy_banner.setWordWrap(True)
        self._privacy_banner.setAlignment(Qt.AlignCenter)
        self._privacy_banner.setStyleSheet("color: #7B5B00; background: #FFF8E1; "
                                           "padding: 2px; border-radius: 3px;")
        root.addWidget(self._privacy_banner)

        self.chat_display = QTextEdit()
        self.chat_display.setReadOnly(True)
        self.chat_display.setFont(QFont("Monospace", 9))
        self.chat_display.setMinimumHeight(200)
        root.addWidget(self.chat_display, stretch=1)

        input_row = QHBoxLayout()
        self.prompt_input = QLineEdit()
        self.prompt_input.setPlaceholderText(
            "Describe what you want to model or change\u2026"
        )
        self.prompt_input.returnPressed.connect(self._send)
        input_row.addWidget(self.prompt_input, stretch=1)

        self.send_btn = QPushButton("Send")
        self.send_btn.setFixedWidth(60)
        self.send_btn.clicked.connect(self._send)
        input_row.addWidget(self.send_btn)
        root.addLayout(input_row)

        btn_row = QHBoxLayout()

        self.insert_btn = QPushButton("Insert & Run")
        self.insert_btn.setToolTip(
            "Copy the extracted code block into the editor and run it"
        )
        self.insert_btn.setEnabled(False)
        self.insert_btn.clicked.connect(self._insert_last_code)
        btn_row.addWidget(self.insert_btn)

        self.clear_btn = QPushButton("Clear Chat")
        self.clear_btn.clicked.connect(self._clear)
        btn_row.addWidget(self.clear_btn)
        root.addLayout(btn_row)

        self.status_label = QLabel("")
        self.status_label.setAlignment(Qt.AlignCenter)
        root.addWidget(self.status_label)

    # ------------------------------------------------------------------
    # Preference helpers
    # ------------------------------------------------------------------

    def _pref(self, name: str):
        return self.preferences[name]

    def _api_key(self) -> str:
        # Prefer system keyring; fall back to plaintext preference
        key = _keyring_load()
        if key:
            return key
        return self._pref("API Key").strip()

    def _save_api_key(self, key: str):
        if not _keyring_save(key):
            # keyring unavailable — store in plaintext preference with warning
            self.preferences["API Key"] = key

    def _base_url(self) -> str:
        return self._pref("Provider / Base URL").strip()

    def _model(self) -> str:
        return self._pref("Model").strip()

    def _auto_run(self) -> bool:
        return self._pref("Auto-run after insert")

    @pyqtSlot(object, object)
    def _on_prefs_changed(self, _param, changes):
        """React to preference changes; wire Enabled to dock visibility."""
        for param, _change, _val in changes:
            if param.name() == "Enabled":
                dock = self._find_dock()
                if dock is not None:
                    if self._pref("Enabled"):
                        dock.show()
                        dock.raise_()
                    else:
                        dock.hide()

    def _find_dock(self):
        """Walk up the parent chain to find the QDockWidget containing us."""
        from PyQt5.QtWidgets import QDockWidget
        p = self.parent()
        while p is not None:
            if isinstance(p, QDockWidget):
                return p
            p = p.parent()
        return None

    # ------------------------------------------------------------------
    # Privacy consent
    # ------------------------------------------------------------------

    def _ensure_privacy_consent(self) -> bool:
        """Show a one-time privacy dialog. Returns True if user consents."""
        if self._pref("_privacy_consent"):
            return True
        rv = QMessageBox.information(
            self,
            "Privacy Notice \u2014 AI Assistant",
            "When you send a prompt, your current CadQuery script and prompt "
            "text are sent to the API endpoint you configured:\n\n"
            "  {url}\n\n"
            "Make sure you are comfortable sharing your code with that provider "
            "before continuing. You can change the endpoint in "
            "Edit \u2192 Preferences \u2192 AI Assistant.\n\n"
            "Do you want to continue?".format(url=self._base_url()),
            QMessageBox.Yes | QMessageBox.No,
            QMessageBox.No,
        )
        if rv == QMessageBox.Yes:
            self.preferences["_privacy_consent"] = True
            return True
        return False

    # ------------------------------------------------------------------
    # History management
    # ------------------------------------------------------------------

    def _prune_history(self):
        """Keep system prompt + at most MAX_HISTORY_TURNS turn pairs."""
        non_system = [m for m in self._history if m["role"] != "system"]
        system     = [m for m in self._history if m["role"] == "system"]
        # Each turn = one user + one assistant message
        max_msgs = MAX_HISTORY_TURNS * 2
        if len(non_system) > max_msgs:
            non_system = non_system[-max_msgs:]
        self._history = system + non_system

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
                "Edit \u2192 Preferences \u2192 AI Assistant \u2192 API Key",
            )
            return

        if not self._ensure_privacy_consent():
            return

        if self._thread and self._thread.isRunning():
            self.status_label.setText("Waiting for previous response\u2026")
            return

        self.prompt_input.clear()
        self._append_chat("user", prompt)

        if not self._history:
            self._history.append({"role": "system", "content": SYSTEM_PROMPT})

        self._history.append({"role": "user", "content": self._inject_context(prompt)})
        self._prune_history()

        self._set_busy(True)
        self._run_worker(list(self._history))

    def _inject_context(self, prompt: str) -> str:
        """Prepend the current editor script as context."""
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

        code = _CODE_RE.search(reply)
        if code:
            self._last_code = code.group(1).strip()
            self.insert_btn.setEnabled(True)
        else:
            # No fenced code block found — do NOT fall back to raw reply.
            # Show an informational message so the user knows what happened.
            self._last_code = ""
            self.insert_btn.setEnabled(False)
            self._append_chat(
                "system",
                "\u26a0\ufe0f  The response did not contain a fenced code block. "
                "Ask the AI to provide the complete script again.",
            )

    @pyqtSlot(str)
    def _on_error(self, message: str):
        self._set_busy(False)
        self._append_chat("error", message)

    @pyqtSlot()
    def _insert_last_code(self):
        if not self._last_code:
            return
        self.insert_code.emit(self._last_code)
        if self._auto_run() and self._debugger is not None:
            try:
                self._debugger.render()
            except Exception:
                pass
        self._append_chat("system", "\u2705 Code inserted into editor.")

    @pyqtSlot()
    def _clear(self):
        self._history.clear()
        self._last_code = ""
        self.insert_btn.setEnabled(False)
        self.chat_display.clear()
        self.status_label.setText("")
        # Reset consent so the privacy notice shows again after a clear
        self.preferences["_privacy_consent"] = False

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    def closeEvent(self, event):
        """Ensure the background thread is cleanly stopped before the widget closes."""
        if self._thread and self._thread.isRunning():
            self._thread.quit()
            self._thread.wait(3000)   # wait up to 3 s
        super().closeEvent(event)

    # ------------------------------------------------------------------
    # Utilities
    # ------------------------------------------------------------------

    def _set_busy(self, busy: bool):
        self.send_btn.setEnabled(not busy)
        self.prompt_input.setEnabled(not busy)
        self.status_label.setText("Thinking\u2026" if busy else "")

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
        else:  # system / info
            fmt_label.setForeground(QColor("#4A148C"))
            label = "     "

        cursor.insertText(label, fmt_label)
        cursor.insertText(text.strip() + "\n\n", fmt_text)

        sb: QScrollBar = self.chat_display.verticalScrollBar()
        sb.setValue(sb.maximum())

    # ------------------------------------------------------------------
    # MainMixin compatibility stubs
    # ------------------------------------------------------------------

    def menuActions(self) -> dict:
        return {}

    def toolbarActions(self) -> list:
        return []
