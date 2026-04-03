# -*- mode: python -*-

import sys
from pathlib import Path
import parso
from PyInstaller.utils.hooks import collect_data_files

block_cipher = None

parso_grammar = (Path(parso.__file__).parent / 'python').glob('grammar*')

a = Analysis(
    ['run.py'],
    pathex=['.'],
    binaries=[],
    datas=[
        ('cq_editor/icons_res.py', 'cq_editor'),
        *[(str(p), 'parso/python') for p in parso_grammar],
        *collect_data_files('debugpy'),
    ],
    hiddenimports=[
        'ipykernel.datapub',
        'pygments.styles.default',
        'qtconsole.client',
        'OCP',
    ],
    hookspath=[],
    runtime_hooks=[
        'pyinstaller/pyi_rth_fontconfig.py',
    ],
    excludes=['_tkinter', 'spyder'],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

# Exclude unmangled system X11/GL libraries that would shadow the system stack
# via PyInstaller's LD_LIBRARY_PATH. Manylinux-vendored copies with a hash in
# their name (e.g. libXcursor-1a09904e.so.1.0.2) are safe to keep because
# their SONAME is also mangled and won't conflict with system libraries.
if sys.platform == 'linux':
    import os, re
    _HASH_RE = re.compile(r'-[0-9a-f]{8}\.')
    _SYSTEM_PREFIXES = (
        'libGL', 'libEGL', 'libGLX', 'libGLdispatch', 'libGLES',
        'libX11', 'libXau', 'libxcb', 'libXcursor', 'libXext',
        'libXfixes', 'libXi', 'libXrender',
        'libbsd',
    )
    def _is_conflicting(name):
        bn = os.path.basename(name)
        return bn.startswith(_SYSTEM_PREFIXES) and not _HASH_RE.search(bn)
    a.binaries = TOC([x for x in a.binaries if not _is_conflicting(x[0])])

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    name='CQ-editor',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    console=False,
    icon='icons/cadquery_logo_dark.ico',
)

