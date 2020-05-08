from sys import platform
from path import Path
from os import system
from shutil import make_archive
from cq_editor import __version__ as version

out_p = Path('dist/CQ-editor')
out_p.rmtree_p()

build_p = Path('build')
build_p.rmtree_p()

system("pyinstaller pyinstaller.spec")

if platform == 'linux':
    with out_p:
        p = Path('.').glob('libpython*')[0]
        p.symlink(p.split(".so")[0]+".so")
        
    make_archive(f'CQ-editor-{version}-linux64','bztar', out_p / '..', 'CQ-editor')
    
elif platform == 'win32':
    
    make_archive(f'CQ-editor-{version}-win64','zip', out_p / '..', 'CQ-editor')
