import base64
import io
import json
import sys

import numpy as np
import pytest

from cq_worker.protocol import Channel, OutputStream


def pytest_report_header(config):
    """Make every saved test or benchmark log identify its CAD runtime."""

    from cq_worker.environment import summary

    return f"worker environment: {summary()}"


def decode(text: str, dtype: str, columns: int) -> np.ndarray:
    return np.frombuffer(base64.b64decode(text), dtype=dtype).reshape(-1, columns)


class Conversation:
    """Feeds scripted requests to a server and collects its messages."""

    def __init__(self, requests: list[dict], session: str | None = None) -> None:
        payload = "".join(json.dumps(r) + "\n" for r in requests).encode()
        self._output = io.BytesIO()
        self.channel = Channel(io.BytesIO(payload), self._output)
        self.session = session

    def serve(self) -> None:
        """Run a server over the scripted requests with stdout redirected."""

        from cq_worker.server import Server

        saved = sys.stdout, sys.stderr
        sys.stdout = OutputStream(self.channel, "stdout")
        sys.stderr = OutputStream(self.channel, "stderr")
        try:
            Server(self.channel, session=self.session).serve_forever()
        finally:
            sys.stdout, sys.stderr = saved

    def output(self, request_id: int) -> str:
        return "".join(
            m["text"] for m in self.of_type("output") if m["id"] == request_id
        )

    @property
    def messages(self) -> list[dict]:
        lines = self._output.getvalue().decode().splitlines()
        return [json.loads(line) for line in lines]

    def of_type(self, kind: str) -> list[dict]:
        return [m for m in self.messages if m["type"] == kind]


@pytest.fixture
def conversation():
    def make(*requests: dict, session: str | None = None) -> Conversation:
        return Conversation(list(requests), session)

    return make
