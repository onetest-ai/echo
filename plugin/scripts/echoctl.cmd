@echo off
rem The plugin's way to the Echo CLI on Windows (the plugin ships no binary).
rem Lookup: %ECHOCTL_BIN% -> plugin\bin\echoctl.exe (dev build) -> PATH ->
rem %LOCALAPPDATA%\Programs\echo\echoctl.exe
if defined ECHOCTL_BIN if exist "%ECHOCTL_BIN%" ( "%ECHOCTL_BIN%" %* & exit /b %errorlevel% )
if exist "%~dp0..\bin\echoctl.exe" ( "%~dp0..\bin\echoctl.exe" %* & exit /b %errorlevel% )
where echoctl.exe >nul 2>nul && ( echoctl.exe %* & exit /b %errorlevel% )
if exist "%LOCALAPPDATA%\Programs\echo\echoctl.exe" ( "%LOCALAPPDATA%\Programs\echo\echoctl.exe" %* & exit /b %errorlevel% )
echo echoctl app is not installed - install it (see the Echo README), then retry 1>&2
exit /b 127
