import os, sys, asyncio

from cq_editor.qt_platform import prefer_non_wayland_platform

if "CASROOT" in os.environ:
    del os.environ["CASROOT"]

if sys.platform == "win32":
    asyncio.set_event_loop_policy(asyncio.WindowsSelectorEventLoopPolicy())

prefer_non_wayland_platform()

from cq_editor.__main__ import main

if __name__ == "__main__":
    main()
