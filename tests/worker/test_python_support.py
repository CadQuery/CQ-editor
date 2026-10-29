from pathlib import Path
import tomllib

from cq_worker.__main__ import supported_python


def test_worker_python_window_matches_package_metadata():
    metadata = tomllib.loads(
        (Path(__file__).resolve().parents[2] / "worker" / "pyproject.toml").read_text(
            encoding="utf-8"
        )
    )
    assert metadata["project"]["requires-python"] == ">=3.11,<3.15"
    assert not supported_python((3, 10))
    assert all(supported_python((3, minor)) for minor in range(11, 15))
    assert not supported_python((3, 15))
