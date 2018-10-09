import os
if 'CASROOT' in os.environ:
    del os.environ['CASROOT']

from src.main import main

main()