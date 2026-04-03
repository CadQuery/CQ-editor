import os
import sys

# On Linux, force Qt to use EGL (or no GLX integration) rather than the
# bundled libqxcb-glx-integration.so, which is built against the CI
# environment's XCB/GLX stack and may be incompatible with the user's system.
if sys.platform.startswith("linux"):
    os.environ.setdefault("QT_XCB_GL_INTEGRATION", "none")
