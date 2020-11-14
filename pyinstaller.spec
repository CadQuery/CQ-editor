# -*- mode: python -*-

import sys, site, os
from path import Path

block_cipher = None

spyder_data = Path(site.getsitepackages()[-1]) / 'spyder'
parso_grammar = (Path(site.getsitepackages()[-1]) / 'parso/python').glob('grammar*')

if sys.platform == 'linux':
    occt_dir = os.path.join(Path(sys.prefix), 'share', 'opencascade')
    ocp_path = (os.path.join(HOMEPATH, 'OCP.cpython-38-x86_64-linux-gnu.so'), '.')
elif sys.platform == 'darwin':
    occt_dir = os.path.join(Path(sys.prefix), 'share', 'opencascade')
    ocp_path = (os.path.join(HOMEPATH, 'OCP.cpython-38-darwin.so'), '.')
elif sys.platform == 'win32':
    occt_dir = os.path.join(Path(sys.prefix), 'Library', 'share', 'opencascade')
    ocp_path = (os.path.join(HOMEPATH, 'OCP.cp38-win_amd64.pyd'), '.')

a = Analysis(['run.py'],
             pathex=['.'],
             binaries=[ocp_path],
             datas=[(spyder_data, 'spyder'),
                    (occt_dir, 'opencascade')] +
                    [(p, 'parso/python') for p in parso_grammar],
             hiddenimports=['ipykernel.datapub'],
             hookspath=[],
             runtime_hooks=['pyinstaller/pyi_rth_occ.py',
                            'pyinstaller/pyi_rth_fontconfig.py'],
             excludes=['_tkinter',],
             win_no_prefer_redirects=False,
             win_private_assemblies=False,
             cipher=block_cipher,
             noarchive=False)

# There is an issue that keeps the OpenSSL libraries from being copied to the output directory
if sys.platform == 'win32':
    from PyInstaller.compat import is_win
    from PyInstaller.depend.bindepend import getfullnameof
    rel_data_path = ['PyQt5', 'Qt', 'bin']
    a.datas += [
        (getfullnameof('libssl-1_1-x64.dll'), os.path.join(*rel_data_path), 'DATA'),
        (getfullnameof('libcrypto-1_1-x64.dll'), os.path.join(*rel_data_path), 'DATA'),
    ]

#    ssl_dll_path = ('.', os.path.join(HOMEPATH, 'Library', 'bin', 'libssl-1_1-x64.dll'), 'LIBRARY')
#    ssl_pdb_path = ('.', os.path.join(HOMEPATH, 'Library', 'bin', 'libssl-1_1-x64.pdb'), 'EXTENSION')
#    crypto_dll_path = ('.', os.path.join(HOMEPATH, 'Library', 'bin', 'libcrypto-1_1-x64.dll'), 'LIBRARY')
#    crypto_pdb_path = ('.', os.path.join(HOMEPATH, 'Library', 'bin', 'libcrypto-1_1-x64.pdb'), 'EXTENSION')
#    a.binaries += [ssl_dll_path]
#    a.binaries += [ssl_pdb_path]
#    a.binaries += [crypto_dll_path]
#    a.binaries += [crypto_pdb_path]

pyz = PYZ(a.pure, a.zipped_data,
             cipher=block_cipher)
exe = EXE(pyz,
          a.scripts,
          [],
          exclude_binaries=True,
          name='CQ-editor',
          debug=False,
          bootloader_ignore_signals=False,
          strip=False,
          upx=True,
          console=True,
          icon='icons/cadquery_logo_dark.ico')

exclude = ('libGL','libEGL','libbsd')
a.binaries = TOC([x for x in a.binaries if not x[0].startswith(exclude)])

coll = COLLECT(exe,
               a.binaries,
               a.zipfiles,
               a.datas,
               strip=False,
               upx=True,
               name='CQ-editor')
