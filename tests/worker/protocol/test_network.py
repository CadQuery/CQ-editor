"""Network requests are checked without contacting external services."""

import io
import json

import pytest

from cq_worker import network


class Response(io.BytesIO):
    status = 201


def test_upload_sends_file_as_binary_multipart(monkeypatch):
    captured = {}

    def urlopen(request, timeout):
        captured.update(request=request, timeout=timeout)
        return Response(b"received")

    monkeypatch.setattr(network.urllib.request, "urlopen", urlopen)
    result = network.upload(
        "https://example.test/upload", b"\x00\xffshape", "part.step"
    )
    request = captured["request"]
    content_type = request.get_header("Content-type")
    boundary = content_type.split("boundary=", 1)[1].encode()

    assert result == {"status": 201, "body": "received"}
    assert request.full_url == "https://example.test/upload"
    assert request.get_method() == "POST"
    assert request.get_header("User-agent") == network.USER_AGENT
    assert captured["timeout"] == network.TIMEOUT
    assert b'name="file"; filename="part.step"' in request.data
    assert b"\x00\xffshape" in request.data
    assert request.data.startswith(b"--" + boundary + b"\r\n")
    assert request.data.endswith(b"--" + boundary + b"--\r\n")


def test_upload_replaces_invalid_response_bytes(monkeypatch):
    monkeypatch.setattr(
        network.urllib.request,
        "urlopen",
        lambda *_args, **_kwargs: Response(b"bad \xff"),
    )
    assert network.upload("https://example.test", b"x", "x") == {
        "status": 201,
        "body": "bad \ufffd",
    }


def test_upload_response_is_bounded_before_it_enters_the_protocol(monkeypatch):
    excess = b"x" * (network.MAX_UPLOAD_RESPONSE_BYTES + 100)
    monkeypatch.setattr(
        network.urllib.request,
        "urlopen",
        lambda *_args, **_kwargs: Response(excess),
    )

    result = network.upload("https://example.test", b"x", "x")

    assert result["body"].startswith("x" * network.MAX_UPLOAD_RESPONSE_BYTES)
    assert result["body"].endswith(
        f"[response truncated after {network.MAX_UPLOAD_RESPONSE_BYTES} bytes]"
    )


def test_newer_releases_ignores_drafts_and_older_versions(monkeypatch):
    captured = {}
    releases = [
        {"tag_name": "v2.3.0", "draft": False},
        {"tag_name": "v2.2.1", "draft": True},
        {"tag_name": "v2.2.0"},
        {"tag_name": "v1.99.0"},
    ]

    def urlopen(request, timeout):
        captured.update(request=request, timeout=timeout)
        return Response(json.dumps(releases).encode())

    monkeypatch.setattr(network.urllib.request, "urlopen", urlopen)
    assert network.newer_releases("2.2", "acme", "modeler") == ["v2.3.0"]
    assert captured["request"].full_url.endswith("/acme/modeler/releases")
    assert captured["timeout"] == network.TIMEOUT


def test_release_response_has_a_size_limit(monkeypatch):
    excess = b"[" + b" " * network.MAX_RELEASES_RESPONSE_BYTES
    monkeypatch.setattr(
        network.urllib.request,
        "urlopen",
        lambda *_args, **_kwargs: Response(excess),
    )

    with pytest.raises(ValueError, match="Release response exceeds"):
        network.newer_releases("2.2")


def test_release_response_must_be_a_list(monkeypatch):
    monkeypatch.setattr(
        network.urllib.request,
        "urlopen",
        lambda *_args, **_kwargs: Response(b'{"tag_name":"v9.0.0"}'),
    )

    with pytest.raises(ValueError, match="must be a JSON list"):
        network.newer_releases("2.2")


@pytest.mark.parametrize(
    ("version", "expected"),
    [
        ("v2.4.0", (2, 4, 0)),
        ("2.4", (2, 4, 0)),
        ("v2.4.1-rc1", (2, 4, 1)),
        ("release-10.0.12", (10, 0, 12)),
    ],
)
def test_version_key_orders_numeric_components(version, expected):
    assert network._version_key(version) == expected


def test_editor_releases_are_newer_published_ones_newest_first(monkeypatch):
    captured = {}
    releases = [
        {
            "tag_name": "v0.9.0",
            "name": "CQ-editor 0.9",
            "html_url": "https://github.com/igors93/CQ-editor/releases/tag/v0.9.0",
        },
        {"tag_name": "v0.10.0-rc1", "prerelease": True},
        {"tag_name": "v0.11.0", "draft": True},
        {"tag_name": "v0.10.1"},
        {"tag_name": "v0.8.0"},
        {"tag_name": "v0.7.5"},
    ]

    def urlopen(request, timeout):
        captured.update(request=request)
        return Response(json.dumps(releases).encode())

    monkeypatch.setattr(network.urllib.request, "urlopen", urlopen)
    newer = network.editor_releases("0.8.0")
    assert captured["request"].full_url.endswith("/igors93/CQ-editor/releases")
    assert [release["tag"] for release in newer] == ["v0.10.1", "v0.9.0"]
    assert newer[1] == {
        "tag": "v0.9.0",
        "name": "CQ-editor 0.9",
        "url": "https://github.com/igors93/CQ-editor/releases/tag/v0.9.0",
    }
    # Without a name or a page, the tag and the usual page stand in.
    assert newer[0]["name"] == "v0.10.1"
    assert newer[0]["url"].endswith("/igors93/CQ-editor/releases/tag/v0.10.1")
