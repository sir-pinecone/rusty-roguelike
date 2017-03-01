@echo off

REM
REM  To run this at startup, use this as your shortcut target:
REM  %windir%\system32\cmd.exe /k w:\rusty-roguelike\misc\shell.bat
REM

call "X:\Programs\Visual Studio 15\VC\vcvarsall.bat" x64
set path=w:\rusty-roguelike\misc;%path%
set _NO_DEBUG_HEAP=1

REM Start the editor
call "C:\Program Files\Git\git-bash.exe"
