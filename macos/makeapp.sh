#!/bin/sh
cd ..
pyinstaller --onedir --windowed --clean -y pyinstaller.spec
cd macos
