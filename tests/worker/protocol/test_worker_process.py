"""The Python worker's real stdin/stdout process boundary."""

import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time

from cq_worker.protocol import OUTPUT_CAPTURE_CHARS

PROJECT_ROOT = Path(__file__).resolve().parents[3]


def run_worker(*requests, env=None):
    environment = os.environ.copy()
    environment["PYTHONPATH"] = str(PROJECT_ROOT / "worker")
    environment.update(env or {})
    payload = "".join(json.dumps(request) + "\n" for request in requests)
    process = subprocess.run(
        [sys.executable, "-m", "cq_worker"],
        input=payload,
        text=True,
        encoding="utf-8",
        capture_output=True,
        cwd=PROJECT_ROOT,
        env=environment,
        timeout=30,
        check=False,
    )
    messages = [json.loads(line) for line in process.stdout.splitlines()]
    return process, messages


def test_worker_process_keeps_native_stdout_out_of_protocol():
    process, messages = run_worker(
        {
            "cmd": "run",
            "id": 12,
            "script": "import os\nos.write(1, b'native warning\\n')\nprint('captured')\nresult = cq.Workplane().box(1, 1, 1)\nshow_object(result)",
        },
        {"cmd": "shutdown"},
    )
    assert process.returncode == 0
    assert "native warning" in process.stderr
    assert messages[0]["type"] == "ready"
    assert any(
        message.get("type") == "output"
        and message.get("id") == 12
        and message.get("text") == "captured\n"
        for message in messages
    )
    (finished,) = [m for m in messages if m["type"] == "run_finished"]
    assert finished["ok"]
    assert [obj["name"] for obj in finished["objects"]] == ["result"]


def test_worker_process_bounds_output_without_a_newline():
    process, messages = run_worker(
        {
            "cmd": "run",
            "id": 13,
            "script": "import sys\nsys.stdout.write('x' * (2 * 1024 * 1024))",
        },
        {"cmd": "shutdown"},
    )

    output = [
        message["text"]
        for message in messages
        if message.get("type") == "output" and message.get("id") == 13
    ]
    assert process.returncode == 0
    assert sum(part.count("x") for part in output) == OUTPUT_CAPTURE_CHARS
    assert sum("output truncated" in part for part in output) == 1
    assert any(message.get("type") == "run_finished" for message in messages)


def test_worker_namespaces_scene_keys_with_the_gui_session():
    process, messages = run_worker(
        {"cmd": "run", "id": 1, "script": "show_object(cq.Workplane().box(1, 1, 1))"},
        {"cmd": "shutdown"},
        env={"CQ_EDITOR_SESSION": "7"},
    )
    assert process.returncode == 0
    (finished,) = [m for m in messages if m["type"] == "run_finished"]
    assert [obj["key"] for obj in finished["objects"]] == ["s7-n1"]


def test_worker_can_be_restarted_after_a_native_process_exit():
    crashed, messages = run_worker(
        {"cmd": "run", "id": 1, "script": "import os; os._exit(7)"}
    )
    assert crashed.returncode == 7
    assert messages[0]["type"] == "ready"

    restarted, messages = run_worker(
        {"cmd": "run", "id": 2, "script": "value = 42"},
        {"cmd": "shutdown"},
    )
    assert restarted.returncode == 0
    (finished,) = [m for m in messages if m["type"] == "run_finished"]
    assert finished["id"] == 2 and finished["ok"]


def test_a_running_worker_stops_meshing_when_cancelled():
    """The cancellation arrives while the worker is busy meshing: its reader
    thread sees it, and meshing stops at the next shape."""

    environment = os.environ.copy()
    environment["PYTHONPATH"] = str(PROJECT_ROOT / "worker")
    process = subprocess.Popen(
        [sys.executable, "-m", "cq_worker"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        encoding="utf-8",
        cwd=PROJECT_ROOT,
        env=environment,
    )
    script = (
        "for i in range(400):\n"
        "    show_object(cq.Workplane().sphere(1 + i / 100).translate((3 * i, 0, 0)))\n"
    )

    def send(request):
        process.stdin.write(json.dumps(request) + "\n")
        process.stdin.flush()

    try:
        send({"cmd": "run", "id": 1, "script": script})
        progress = []
        finished = None
        for line in process.stdout:
            message = json.loads(line)
            if message["type"] == "progress":
                progress.append(message)
                if len(progress) == 2:
                    send({"cmd": "cancel", "target": 1})
            elif message["type"] == "run_finished":
                finished = message
                break
        send({"cmd": "shutdown"})
        assert process.wait(timeout=30) == 0
    finally:
        if process.poll() is None:
            process.kill()

    assert finished is not None and finished["cancelled"] is True
    assert finished["objects"] is None
    assert progress[0]["total"] == 400
    assert progress[-1]["done"] < 400, "stopped before the end"


def test_stop_interrupts_a_running_worker_which_keeps_its_session():
    """Interrupts arrive while the worker is busy, in a sleep and in a pure
    Python loop; the process, its console namespace and cache survive."""

    environment = os.environ.copy()
    environment["PYTHONPATH"] = str(PROJECT_ROOT / "worker")
    process = subprocess.Popen(
        [sys.executable, "-m", "cq_worker"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        encoding="utf-8",
        cwd=PROJECT_ROOT,
        env=environment,
    )

    def send(request):
        process.stdin.write(json.dumps(request) + "\n")
        process.stdin.flush()

    def until(kind, request_id):
        for line in process.stdout:
            message = json.loads(line)
            if message.get("type") == kind and message.get("id") == request_id:
                return message
            if message.get("type") == "output" and message.get("id") == request_id:
                outputs.append(message["text"])
        raise AssertionError(f"no {kind} for {request_id}")

    outputs = []
    timer = threading.Timer(60, process.kill)
    timer.start()
    try:
        send({"cmd": "run", "id": 1, "script": "answer = 41 + 1"})
        assert until("run_finished", 1)["ok"]
        for request_id, script in [
            (2, "import time\ntime.sleep(120)"),
            (3, "while True:\n    pass"),
        ]:
            send({"cmd": "run", "id": request_id, "script": script})
            time.sleep(1.0)
            started = time.monotonic()
            send({"cmd": "interrupt", "target": request_id})
            finished = until("run_finished", request_id)
            assert finished["interrupted"] is True, script
            assert time.monotonic() - started < 10
        send({"cmd": "console", "id": 4, "source": "print(answer)"})
        until("console_result", 4)
        assert outputs == ["42\n"]
        send({"cmd": "shutdown"})
        assert process.wait(timeout=30) == 0
    finally:
        timer.cancel()
        if process.poll() is None:
            process.kill()
