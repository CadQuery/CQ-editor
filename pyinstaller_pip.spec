# -*- mode: python -*-

import sys, site, os
from path import Path
from PyInstaller.utils.hooks import collect_all, collect_submodules

block_cipher = None

spyder_data = Path(site.getsitepackages()[-1]) / 'spyder'
parso_grammar = (Path(site.getsitepackages()[-1]) / 'parso/python').glob('grammar*')
cqw_path = Path(site.getsitepackages()[-1]) / 'cq_warehouse'
cq_path = Path(site.getsitepackages()[-1]) / 'cadquery'

if sys.platform == 'linux':
    occt_dir = os.path.join(Path(sys.prefix), 'share', 'opencascade')
    ocp_path = [(os.path.join(HOMEPATH, 'OCP.cpython-39-x86_64-linux-gnu.so'), '.')]
elif sys.platform == 'darwin':
    occt_dir = os.path.join(Path(sys.prefix), 'share', 'opencascade')
    ocp_path = [(os.path.join(HOMEPATH, 'OCP.cpython-39-darwin.so'), '.')]
elif sys.platform == 'win32':
    occt_dir = os.path.join(Path(sys.prefix), 'Library', 'share', 'opencascade')
    ocp_path = [(os.path.join(HOMEPATH, 'OCP.cp39-win_amd64.pyd'), '.')]

datas1, binaries1, hiddenimports1 = collect_all('debugpy')
hiddenimports2 = collect_submodules('xmlrpc')

a = Analysis(['run.py'],
             pathex=['.'],
             binaries=ocp_path + binaries1,
             datas=[(spyder_data, 'spyder'),
                    (cqw_path, 'cq_warehouse'),
                    (cq_path, 'cadquery')] +
                    [(p, 'parso/python') for p in parso_grammar] + datas1,
             hiddenimports=['ipykernel.datapub', 'debugpy', 'vtkmodules', 'vtkmodules.all',
                            'pyqtgraph.graphicsItems.ViewBox.axisCtrlTemplate_pyqt5',
                            'pyqtgraph.graphicsItems.PlotItem.plotConfigTemplate_pyqt5',
                            'pyqtgraph.imageview.ImageViewTemplate_pyqt5', 'xmlrpc',
                            'zmq.backend', 'cq_warehouse', 'cq_warehouse.bearing', 'cq_warehouse.chain',
                            'cq_warehouse.drafting', 'cq_warehouse.extensions', 'cq_warehouse.fastener',
                            'cq_warehouse.sprocket', 'cq_warehouse.thread', 'cq_gears', 'cq_cache',
                            'build123d', 'cqmore'] + hiddenimports1 + hiddenimports2,
             hookspath=['pyinstaller/extrahooks/'],
             runtime_hooks=['pyinstaller/pyi_rth_occ.py',
                            'pyinstaller/pyi_rth_fontconfig.py'],
             excludes=['_tkinter'],
             win_no_prefer_redirects=False,
             win_private_assemblies=False,
             cipher=block_cipher,
             noarchive=False)

# There is an issue that keeps the OpenSSL libraries from being copied to the output directory.
# This should work if nothing else, but does not with GitHub Actions
if sys.platform == 'win32':
    from PyInstaller.depend.bindepend import getfullnameof
    rel_data_path = ['PyQt5', 'Qt', 'bin']
    a.datas += [
        (getfullnameof('libssl-1_1-x64.dll'), os.path.join(*rel_data_path), 'DATA'),
        (getfullnameof('libcrypto-1_1-x64.dll'), os.path.join(*rel_data_path), 'DATA'),
    ]


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

exclude = ()
#exclude = ('libGL','libEGL','libbsd')
a.binaries = TOC([x for x in a.binaries if not x[0].startswith(exclude)])

coll = COLLECT(exe,
               a.binaries,
               a.zipfiles,
               a.datas,
               strip=False,
               upx=True,
               name='CQ-editor')
