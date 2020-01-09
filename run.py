import os, sys
import faulthandler

faulthandler.enable()

# macOS implements a security sandbox policy to executable
# programs when they are launched from the launchd context, e.g. 
# when double clicked from the Finder or launched from the shell
# with the 'open' OS command.  Therefore, the macOS build requires
# the path to the environment variable 'CSF_ShadersDirectory'
# to be specified as an absolute path rather than a relative one.
# 
# The following code performs a runtime substitution of the
# 'CSF_ShadersDirectory' environment variable if it is discovered.
# It introspects this program's absolute path and appends the
# relative path discovered from the environment.

if sys.platform == 'darwin':
    basedir = os.path.dirname(os.path.abspath(sys.argv[0]))
    if 'CSF_ShadersDirectory' in os.environ:
        new_path = basedir + os.sep + os.environ['CSF_ShadersDirectory']
        os.environ['CSF_ShadersDirectory'] = new_path
else:
    if 'CASROOT' in os.environ:
        del os.environ['CASROOT']

from cq_editor.__main__ import main

main()
