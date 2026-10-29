@echo off
setlocal

if exist "%PREFIX%\Scripts\menuinst.exe" (
  "%PREFIX%\Scripts\menuinst.exe" --prefix "%PREFIX%" --remove cq-editor_menu.json
)
exit /b 0
