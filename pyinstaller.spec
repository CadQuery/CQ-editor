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

# Exclude problemmatic Linux system libraries
if sys.platform == 'linux':
    exclude_libs = ('libGL', 'libEGL', 'libGLX', 'libGLdispatch', 'libGLES', 'libxcb-glx', 'libbsd')
    a.binaries = TOC(
        [x for x in a.binaries if not x[0].startswith(exclude_libs)]
    )

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

