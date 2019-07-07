# -*- mode: python -*-

import sys, site
from path import Path

block_cipher = None

spyder_fonts = Path(site.getsitepackages()[-1]) / 'spyder/fonts'
parso_grammar = Path(site.getsitepackages()[-1]) / 'parso/python/grammar36.txt'
oce_dir = Path(sys.prefix) / 'Library' / 'share' / 'oce'

with open('dummy','wb'):
    pass

a = Analysis(['run.py'],
             pathex=['/home/adam/cq/CQ-editor'],
             binaries=[],
             datas=[('dummy', 'spyder/utils'),
                    (spyder_fonts ,'spyder/fonts'),
                    (parso_grammar, 'parso/python'),
                    (oce_dir , 'oce')],
             hiddenimports=['ipykernel.datapub'],
             hookspath=[],
             runtime_hooks=['pyinstaller/pyi_rth_occ.py'],
             excludes=['_tkinter',],
             win_no_prefer_redirects=False,
             win_private_assemblies=False,
             cipher=block_cipher,
             noarchive=False)

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

coll = COLLECT(exe,
               a.binaries,
               a.zipfiles,
               a.datas,
               strip=False,
               upx=True,
               name='CQ-editor')
