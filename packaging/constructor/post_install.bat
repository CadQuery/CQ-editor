@echo off
setlocal

if not exist "%PREFIX%\Scripts\menuinst.exe" exit /b 1

rem Refresh the Start menu entry and the optional Python file association.
"%PREFIX%\Scripts\menuinst.exe" --prefix "%PREFIX%" --remove cq-editor_menu.json >nul 2>&1
"%PREFIX%\Scripts\menuinst.exe" --prefix "%PREFIX%" --install cq-editor_menu.json
exit /b %ERRORLEVEL%
