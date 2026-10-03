@echo off
setlocal
set MSBUILD=%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\MSBuild.exe
if not exist "%MSBUILD%" set MSBUILD=%WINDIR%\Microsoft.NET\Framework\v4.0.30319\MSBuild.exe
if not exist "%MSBUILD%" (
  echo MSBuild.exe was not found.
  exit /b 1
)
"%MSBUILD%" "%~dp0WinGlow.Legacy.csproj" /t:Rebuild /p:Configuration=Release /m
