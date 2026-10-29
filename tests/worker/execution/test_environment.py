"""Script isolation, imports, and variable inspection."""

import linecache
import os
import sys

import pytest

from cq_worker.execution import (
    SCRIPT_FILENAME,
    RunSettings,
    compile_script,
    describe_variables,
    script_environment,
)


def test_compile_script_populates_linecache_for_tracebacks():
    code = compile_script("a = 1\nraise RuntimeError('broken')\n")
    assert code.co_filename == SCRIPT_FILENAME
    assert linecache.getline(SCRIPT_FILENAME, 2) == "raise RuntimeError('broken')\n"


def test_script_environment_restores_cwd_and_path_after_exception(tmp_path):
    script = tmp_path / "part.py"
    old_cwd = os.getcwd()
    old_path = list(sys.path)

    with pytest.raises(RuntimeError, match="stop"):
        with script_environment(str(script), RunSettings()):
            assert os.getcwd() == str(tmp_path)
            assert sys.path[0] == str(tmp_path)
            raise RuntimeError("stop")

    assert os.getcwd() == old_cwd
    assert sys.path == old_path


def test_script_environment_respects_disabled_path_and_cwd_changes(tmp_path):
    old_cwd = os.getcwd()
    old_path = list(sys.path)
    settings = RunSettings(
        add_script_dir_to_path=False,
        change_working_dir=False,
        reload_imported_modules=False,
    )
    with script_environment(str(tmp_path / "part.py"), settings):
        assert os.getcwd() == old_cwd
        assert sys.path == old_path


def test_script_environment_without_file_does_not_change_cwd_or_path():
    old_cwd = os.getcwd()
    old_path = list(sys.path)
    with script_environment(None, RunSettings()):
        assert os.getcwd() == old_cwd
        assert sys.path == old_path


def test_describe_variables_skips_private_and_hidden_values():
    rows = describe_variables({"_private": 1, "secret": 2, "visible": 3}, {"secret"})
    assert rows == [{"name": "visible", "type": "int", "value": "3"}]


def test_describe_variables_handles_broken_string_conversion():
    class Broken:
        def __str__(self):
            raise ValueError("cannot display")

    (row,) = describe_variables({"bad": Broken()}, set())
    assert row["name"] == "bad"
    assert row["type"] == "Broken"
    assert row["value"] == "<unprintable: ValueError>"


@pytest.mark.parametrize("message", [None, {}, {"unknown": 1}])
def test_run_settings_defaults_for_missing_fields(message):
    settings = RunSettings.from_message(message)
    assert settings == RunSettings()
