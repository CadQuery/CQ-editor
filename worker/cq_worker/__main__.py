"""Entry point: ``python -m cq_worker``."""

import logging
import os
import signal
import sys
import traceback

from .protocol import Channel, redirect_std_streams


def supported_python(version: tuple[int, int]) -> bool:
    return (3, 11) <= version < (3, 15)


def main() -> int:
    # Stop interrupts scripts like Ctrl+C would (``_thread.interrupt_main``),
    # which does nothing if SIGINT was inherited as ignored.
    signal.signal(signal.SIGINT, signal.default_int_handler)
    channel = Channel.from_std_streams()
    redirect_std_streams(channel)

    if not supported_python(sys.version_info[:2]):
        channel.send(
            {
                "type": "fatal",
                "message": (
                    f"Python {sys.version_info.major}.{sys.version_info.minor} is unsupported; "
                    "use Python 3.11–3.14 with matching CadQuery and OCP packages"
                ),
                "detail": "The worker package requires Python >=3.11,<3.15.",
                "executable": sys.executable,
            }
        )
        return 1

    # Importing CadQuery takes a few seconds; the GUI shows a "starting"
    # state until the ready message arrives.
    try:
        # A stale CASROOT from another OpenCascade install breaks OCP.
        os.environ.pop("CASROOT", None)

        from .server import ChannelLogHandler, Server
    except Exception as error:
        channel.send(
            {
                "type": "fatal",
                "message": f"Cannot load CadQuery: {type(error).__name__}: {error}",
                "detail": traceback.format_exc(),
                "executable": sys.executable,
            }
        )
        return 1

    logging.basicConfig(level=logging.INFO, handlers=[ChannelLogHandler(channel)])
    Server(channel, session=os.environ.get("CQ_EDITOR_SESSION")).serve_forever()
    return 0


if __name__ == "__main__":
    sys.exit(main())
