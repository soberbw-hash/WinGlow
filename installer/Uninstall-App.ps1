[CmdletBinding()]
param(
    [switch]$Quiet
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms

$appDisplayName = 'WinGlow'
$installDir = $PSScriptRoot
$installedExe = Join-Path $installDir 'WinGlow.exe'
$desktopShortcut = Join-Path ([Environment]::GetFolderPath('Desktop')) ($appDisplayName + '.lnk')
$startMenuDir = Join-Path ([Environment]::GetFolderPath('Programs')) $appDisplayName
$registryPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\WinGlowLegacy'

$running = Get-Process -Name 'WinGlow' -ErrorAction SilentlyContinue
if ($running) {
    [System.Windows.Forms.MessageBox]::Show('请先关闭正在运行的 WinGlow，然后再卸载。', $appDisplayName, [System.Windows.Forms.MessageBoxButtons]::OK, [System.Windows.Forms.MessageBoxIcon]::Warning) | Out-Null
    exit 1
}

if (-not $Quiet) {
    $confirm = [System.Windows.Forms.MessageBox]::Show('确定要卸载 WinGlow吗？', $appDisplayName, [System.Windows.Forms.MessageBoxButtons]::YesNo, [System.Windows.Forms.MessageBoxIcon]::Question)
    if ($confirm -ne [System.Windows.Forms.DialogResult]::Yes) {
        exit 0
    }
}

Remove-Item -LiteralPath $desktopShortcut -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $startMenuDir -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -Path $registryPath -Recurse -Force -ErrorAction SilentlyContinue

$cleanupScript = Join-Path $env:TEMP ('WGF-Uninstall-' + [Guid]::NewGuid().ToString('N') + '.cmd')
$cleanupContent = @"
@echo off
timeout /t 1 /nobreak >nul
rmdir /s /q "$installDir"
del "%~f0"
"@
Set-Content -LiteralPath $cleanupScript -Value $cleanupContent -Encoding ASCII
Start-Process -FilePath 'cmd.exe' -ArgumentList "/c `"$cleanupScript`"" -WindowStyle Hidden

if (-not $Quiet) {
    [System.Windows.Forms.MessageBox]::Show('卸载已完成。', $appDisplayName, [System.Windows.Forms.MessageBoxButtons]::OK, [System.Windows.Forms.MessageBoxIcon]::Information) | Out-Null
}




