import pytest
from PyQt5.QtCore import Qt, QThread
from PyQt5.QtWidgets import QMessageBox
from pyqtgraph.parametertree import Parameter

from cq_editor.widgets.ai_chat import AIChatWidget, _keyring_load, _keyring_save
from cq_editor.__main__ import MainWindow

def test_ai_chat_init(qtbot):
    """Test that AIChatWidget initializes with correct default preferences."""
    widget = AIChatWidget()
    qtbot.addWidget(widget)

    assert widget.preferences is not None
    assert isinstance(widget.preferences, Parameter)
    assert widget._pref("Enabled") is False
    assert widget._pref("Provider / Base URL") == "https://integrate.api.nvidia.com/v1"
    assert widget._pref("Model") == "meta/llama-3.3-70b-instruct"
    assert widget._pref("API Key") == ""

def test_keyring_sanitation():
    """Test that keyring load/save successfully deletes and handles corrupted OrderedDict maps."""
    # Test saving a corrupted key containing OrderedDict
    assert _keyring_save("OrderedDict([('tip', 'Your API key.')])") is True
    # Loading should automatically detect the corruption, delete it, and return None
    assert _keyring_load() is None

    # Test saving a clean string key
    assert _keyring_save("my-clean-test-key") is True
    assert _keyring_load() == "my-clean-test-key"

    # Clean up
    assert _keyring_save("") is True
    assert _keyring_load() is None

def test_privacy_consent_dialog(qtbot, mocker):
    """Test the one-time privacy consent notice triggers correctly and saves selection."""
    widget = AIChatWidget()
    qtbot.addWidget(widget)

    # Initially consent is False
    assert widget._pref("_privacy_consent") is False

    # Mock the QMessageBox to simulate user clicking Yes
    mocker.patch.object(QMessageBox, "information", return_value=QMessageBox.Yes)
    assert widget._ensure_privacy_consent() is True
    assert widget._pref("_privacy_consent") is True

    # Subsequent calls should return True immediately without prompting
    mocker.patch.object(QMessageBox, "information", side_effect=Exception("Should not prompt"))
    assert widget._ensure_privacy_consent() is True

def test_auto_fix_error_flow(qtbot, mocker):
    """Test that auto-fix error triggers the correct chat flow and status updating."""
    # Mock MainWindow and dependencies
    win = MainWindow()
    qtbot.addWidget(win)

    chat = win.components.get("ai_chat")
    if chat is None:
        pytest.skip("AI Assistant is not loaded in MainWindow components")

    # Mock the worker execution to prevent thread start
    mock_run = mocker.patch.object(chat, "_run_worker")

    # Trigger auto-fix
    test_error = "Standard Failure: edges mismatch on line 12"
    chat.auto_fix_error(test_error)

    # Should set auto_insert_flag and initiate LLM worker
    assert chat._auto_insert_flag is True
    assert mock_run.call_count == 1

    # Clean up
    chat._auto_insert_flag = False
