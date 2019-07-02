# -*- mode: python -*-

import sys
from path import Path

block_cipher = None

spyder_fonts =Path(sys.prefix) / 'lib/python3.6/site-packages/spyder/fonts'

a = Analysis(['run.py'],
             pathex=['/home/adam/cq/CQ-editor'],
             binaries=[],
             datas=[(spyder_fonts / 'spyder.ttf','spyder/fonts'),
                    (spyder_fonts / 'spyder-charmap.json' ,'spyder/fonts')],
             hiddenimports=['ipykernel.datapub'],
             hookspath=[],
             runtime_hooks=[],
             excludes=[],
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
          name='run',
          debug=False,
          bootloader_ignore_signals=False,
          strip=False,
          upx=True,
          console=True )
coll = COLLECT(exe,
               a.binaries,
               a.zipfiles,
               a.datas,
               strip=False,
               upx=True,
               name='run')
