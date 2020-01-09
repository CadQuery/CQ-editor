# -*- mode: python -*-

import sys, site, os
from path import Path

block_cipher = None
spyder_data = Path(site.getsitepackages()[-1]) / "spyder"
parso_grammar = Path(site.getsitepackages()[-1]) / "parso/python/grammar36.txt"

if sys.platform == "linux" or sys.platform == "darwin":
    oce_dir = Path(sys.prefix) / "share" / "oce-0.18"
else:
    oce_dir = Path(sys.prefix) / "Library" / "share" / "oce"


a = Analysis(
    ["run.py"],
    pathex=[],
    binaries=[],
    datas=[(spyder_data, "spyder"), (parso_grammar, "parso/python"), (oce_dir, "oce")],
    hiddenimports=["ipykernel.datapub"],
    hookspath=[],
    runtime_hooks=["pyinstaller/pyi_rth_occ.py", "pyinstaller/pyi_rth_fontconfig.py"],
    excludes=["_tkinter",],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)
exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="CQ-Editor",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=True,
    icon="icons/cadquery_logo_dark.ico",
)

exclude = ("libGL", "libEGL", "libbsd")
a.binaries = TOC([x for x in a.binaries if not x[0].startswith(exclude)])

coll = COLLECT(
    exe, a.binaries, a.zipfiles, a.datas, strip=False, upx=True, name="CQ-Editor"
)

app = BUNDLE(
    coll,
    name="CQ-Editor.app",
    icon="icons/cadquery_logo_dark.icns",
    bundle_identifier="org.cadquery.cqeditor",
    info_plist={
        "CFBundleName": "CQ-Editor",
        "CFBundleShortVersionString": "0.1.0",
        "NSHighResolutionCapable": True,
        "NSPrincipalClass": "NSApplication",
        "NSAppleScriptEnabled": False,
        "LSBackgroundOnly": False,
    },
)

