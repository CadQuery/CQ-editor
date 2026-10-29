"""Rich console output: bundles gathered, checked and bounded in the worker."""

import base64
import builtins
import threading

import pytest

from cq_worker import display
from cq_worker.display import Displayer, message, mime_bundle

#: A 1×1 PNG.
PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
)
JPEG = b"\xff\xd8\xff\xe0" + b"\x00" * 16


class Rich:
    def __init__(self, **reprs):
        for name, value in reprs.items():
            setattr(self, f"_repr_{name}_", value)

    def __repr__(self):
        return "Rich()"


def test_bundles_are_gathered_as_jupyter_does():
    bundle = mime_bundle(
        Rich(mimebundle=lambda include, exclude: ({"text/html": "<b>x</b>"}, {}))
    )
    assert bundle == {"text/html": "<b>x</b>", "text/plain": "Rich()"}
    assert mime_bundle(Rich(png=lambda: PNG))["image/png"] == PNG
    assert mime_bundle(Rich(png=lambda: (PNG, {"width": 1})))["image/png"] == PNG
    # A broken representation hides nothing else.
    broken = mime_bundle(Rich(png=lambda: 1 / 0))
    assert broken == {"text/plain": "Rich()"}
    empty_tuple = mime_bundle(Rich(png=lambda: ()))
    assert empty_tuple == {"text/plain": "Rich()"}

    invalid_plain = mime_bundle(
        Rich(mimebundle=lambda include, exclude: {"text/plain": b"not text"})
    )
    assert invalid_plain["text/plain"] == "Rich()"

    class Bad:
        def __repr__(self):
            raise RuntimeError("no")

    assert mime_bundle(Bad())["text/plain"].startswith("<")

    class BrokenAttribute:
        def __getattr__(self, _name):
            raise RuntimeError("broken descriptor")

    assert "BrokenAttribute" in mime_bundle(BrokenAttribute())["text/plain"]
    # Classes have the methods unbound: they are not called.
    assert mime_bundle(Rich) == {"text/plain": repr(Rich)}


def test_only_checked_images_and_plain_text_leave_the_worker():
    fields = message({"text/plain": "x", "image/png": PNG}, images_left=1)
    assert fields["image"] == {
        "format": "png",
        "data": base64.b64encode(PNG).decode(),
    }
    assert set(fields) == {"text", "image"}, "no HTML, SVG or other markup"
    # Bundles from _repr_mimebundle_ hold base64 text.
    as_text = message({"image/jpeg": base64.b64encode(JPEG).decode()}, images_left=1)
    assert as_text["image"]["format"] == "jpeg"
    # Not what it claims to be.
    assert message({"image/png": b"<svg/>"}, images_left=1)["image"] is None
    assert message({"image/png": "not base64!"}, images_left=1)["image"] is None


def test_results_are_bounded(monkeypatch):
    monkeypatch.setattr(display, "MAX_IMAGE_BYTES", len(PNG) + 10)
    big = PNG + b"\x00" * 64
    fields = message({"text/plain": "big", "image/png": big}, images_left=1)
    assert fields["image"] is None and "over the limit" in fields["text"]
    encoded = message(
        {"text/plain": "encoded", "image/png": base64.b64encode(big).decode()},
        images_left=1,
    )
    assert encoded["image"] is None and "over the limit" in encoded["text"]
    fields = message({"text/plain": "one more", "image/png": PNG}, images_left=0)
    assert fields["image"] is None and "too many images" in fields["text"]
    long = message({"text/plain": "x" * (display.MAX_TEXT_CHARS + 10)}, images_left=1)
    assert long["text"].endswith("characters]")
    assert len(long["text"]) < display.MAX_TEXT_CHARS + 100


def test_the_displayer_counts_images_per_command_and_keeps_underscore(monkeypatch):
    monkeypatch.setattr(display, "MAX_IMAGES_PER_REQUEST", 2)
    sent = []
    displayer = Displayer(sent.append)
    displayer.start(7)
    displayer.display(*[Rich(png=lambda: PNG)] * 3)
    assert [m["image"] is not None for m in sent] == [True, True, False]
    assert {m["id"] for m in sent} == {7}
    displayer.start(8)
    displayer.display(Rich(png=lambda: PNG))
    assert sent[-1]["image"] is not None, "a new command, a new allowance"

    displayer.displayhook(None)
    assert len(sent) == 4, "None is not shown"
    displayer.displayhook(41)
    assert sent[-1] == {
        "id": 8,
        "type": "display",
        "text": "41",
        "image": None,
    }
    assert builtins._ == 41


def test_the_displayer_bounds_text_only_results_per_command(monkeypatch):
    monkeypatch.setattr(display, "MAX_RESULTS_PER_REQUEST", 2)
    represented = 0

    class Counted:
        def __repr__(self):
            nonlocal represented
            represented += 1
            return "value"

    sent = []
    displayer = Displayer(sent.append)
    displayer.start(10)
    displayer.display(Counted(), Counted(), Counted())
    displayer.display(Counted())

    assert represented == 2, "discarded values do not run user representation code"
    assert len(sent) == 3
    assert "truncated after 2 results" in sent[-1]["text"]

    displayer.start(11)
    displayer.display(Counted())
    assert represented == 3 and sent[-1]["text"] == "value"


def test_displayer_rejects_late_and_background_results():
    sent = []
    displayer = Displayer(sent.append)

    displayer.display("before")
    displayer.start(17)
    background = threading.Thread(target=displayer.display, args=("background",))
    background.start()
    background.join()
    assert sent == [], "another thread cannot impersonate the active command"

    displayer.display("current")
    assert sent == [{"id": 17, "type": "display", "text": "'current'", "image": None}]
    displayer.finish()
    displayer.display("late")
    assert len(sent) == 1, "a retained display call cannot leak into a later request"


def test_matplotlib_figures_are_shown_as_images():
    pyplot = pytest.importorskip("matplotlib.pyplot")
    figure, axes = pyplot.subplots(figsize=(2, 1))
    axes.plot([0, 1], [1, 0])
    image = mime_bundle(figure)["image/png"]
    assert image.startswith(b"\x89PNG")
    pyplot.close(figure)


def test_the_console_sends_results_and_images_after_what_was_printed(conversation):
    png = base64.b64encode(PNG).decode()
    source = (
        "print('first', end='')\n"
        "class Picture:\n"
        "    def _repr_png_(self):\n"
        f"        import base64; return base64.b64decode('{png}')\n"
        "\n"
        "display(Picture(), 'a text')"
    )
    talk = conversation(
        {"cmd": "console", "id": 1, "source": "6 * 7"},
        {"cmd": "console", "id": 2, "source": source},
    )
    talk.serve()

    (result,) = [m for m in talk.of_type("display") if m["id"] == 1]
    assert result["text"] == "42" and result["image"] is None
    kinds = [m["type"] for m in talk.messages if m.get("id") == 2]
    assert kinds == ["output", "display", "display", "console_result"]
    picture, text = [m for m in talk.of_type("display") if m["id"] == 2]
    assert picture["image"]["format"] == "png"
    assert text == {"type": "display", "id": 2, "text": "'a text'", "image": None}
