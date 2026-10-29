"""Wire examples shared with the Rust reader tests."""

import io
import json
from pathlib import Path

import pytest

from cq_worker.protocol import OUTPUT_CAPTURE_CHARS, Channel, OutputStream

FIXTURE = (
    Path(__file__).resolve().parents[2] / "fixtures" / "protocol" / "messages.json"
)


def examples():
    return json.loads(FIXTURE.read_text(encoding="utf-8"))


def test_worker_emits_golden_messages_as_strict_json_lines():
    writer = io.BytesIO()
    channel = Channel(io.BytesIO(), writer)
    cases = examples()["valid"]
    for case in cases:
        channel.send(case["message"])

    lines = writer.getvalue().splitlines()
    assert len(lines) == len(cases)
    assert [json.loads(line) for line in lines] == [case["message"] for case in cases]
    assert json.loads(lines[1])["part"]["mesh"]["positions"]
    assert list(json.loads(lines[2])["part"]["mesh"]) == ["id"]


def test_nonfinite_golden_message_cannot_cross_the_pipe():
    wire = examples()["invalid"][0]["wire"]

    def reject_constant(value):
        raise ValueError(f"Non-finite JSON number: {value}")

    with pytest.raises(ValueError, match="NaN"):
        json.loads(wire, parse_constant=reject_constant)
    writer = io.BytesIO()
    channel = Channel(io.BytesIO(), writer)
    with pytest.raises(ValueError):
        channel.send(json.loads(wire))
    assert not writer.getvalue()


def test_oversized_golden_output_is_split_and_bounded_before_the_pipe():
    spec = examples()["oversized_control"]
    writer = io.BytesIO()
    channel = Channel(io.BytesIO(), writer)
    channel.current_id = 7
    stream = OutputStream(channel, "stdout")
    stream.write("x" * spec["repeat"])
    messages = [json.loads(line) for line in writer.getvalue().splitlines()]
    assert all(message["type"] == spec["type"] for message in messages)
    assert (
        sum(message["text"].count("x") for message in messages) == OUTPUT_CAPTURE_CHARS
    )
    assert sum("output truncated" in message["text"] for message in messages) == 1
