import os
import sys

# Prevent Qt's XCB plugin from initializing GLX before OCCT does.
# When Qt's bundled libqxcb.so fails to find a matching GLX FBConfig,
# it can leave the display connection in a state that breaks OCCT's own
# GLX initialization. OCCT creates its own GL context via Xw_Window
# directly, so Qt does not need GLX integration.
if sys.platform.startswith("linux"):
    os.environ["QT_XCB_GL_INTEGRATION"] = "none"
