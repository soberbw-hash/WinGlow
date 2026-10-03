[CmdletBinding()]
param(
    [string]$InstallDir,
    [switch]$Quiet,
    [switch]$NoLaunch,
    [switch]$SkipShortcuts,
    [switch]$SkipRegistration
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

Add-Type -AssemblyName System.Windows.Forms

$appDisplayName = 'WinGlow'
$appPublisher = 'soberbw-hash'
$appExeName = 'WinGlow.exe'
$installFolderName = 'WinGlowLegacy'
$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$payloadZip = Join-Path $scriptRoot 'AppPackage.zip'
$uninstallScriptSource = Join-Path $scriptRoot 'Uninstall-App.ps1'

if (-not (Test-Path -LiteralPath $payloadZip)) {
    throw "未找到安装包内容：$payloadZip"
}

if (-not $InstallDir) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "Programs\$installFolderName"
}

$tempExtractDir = Join-Path $env:TEMP ("WGFSetup-" + [Guid]::NewGuid().ToString('N'))
$startMenuDir = Join-Path ([Environment]::GetFolderPath('Programs')) $appDisplayName
$desktopShortcutPath = Join-Path ([Environment]::GetFolderPath('Desktop')) ($appDisplayName + '.lnk')
$startMenuShortcutPath = Join-Path $startMenuDir ($appDisplayName + '.lnk')
$uninstallShortcutPath = Join-Path $startMenuDir ('卸载 ' + $appDisplayName + '.lnk')
$uninstallCmdPath = Join-Path $InstallDir ('卸载 ' + $appDisplayName + '.cmd')
$registryKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\WinGlowLegacy'

function New-AppShortcut {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$TargetPath,
        [string]$Arguments = '',
        [string]$WorkingDirectory = '',
        [string]$Description = '',
        [string]$IconLocation = ''
    )

    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($Path)
    $shortcut.TargetPath = $TargetPath
    $shortcut.Arguments = $Arguments
    $shortcut.WorkingDirectory = $WorkingDirectory
    $shortcut.Description = $Description
    if ($IconLocation) {
        $shortcut.IconLocation = $IconLocation
    }
    $shortcut.Save()
}

try {
    New-Item -ItemType Directory -Path $tempExtractDir -Force | Out-Null
    Expand-Archive -LiteralPath $payloadZip -DestinationPath $tempExtractDir -Force

    if (Test-Path -LiteralPath $InstallDir) {
        Remove-Item -LiteralPath $InstallDir -Recurse -Force
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item -Path (Join-Path $tempExtractDir '*') -Destination $InstallDir -Recurse -Force
    Copy-Item -LiteralPath $uninstallScriptSource -Destination (Join-Path $InstallDir 'Uninstall-App.ps1') -Force

    $uninstallCmd = @"
@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Uninstall-App.ps1"
"@
    Set-Content -LiteralPath $uninstallCmdPath -Value $uninstallCmd -Encoding ASCII

    $installedExe = Join-Path $InstallDir $appExeName
    if (-not $SkipShortcuts) {
        New-Item -ItemType Directory -Path $startMenuDir -Force | Out-Null
        New-AppShortcut -Path $desktopShortcutPath -TargetPath $installedExe -WorkingDirectory $InstallDir -Description $appDisplayName -IconLocation $installedExe
        New-AppShortcut -Path $startMenuShortcutPath -TargetPath $installedExe -WorkingDirectory $InstallDir -Description $appDisplayName -IconLocation $installedExe
        New-AppShortcut -Path $uninstallShortcutPath -TargetPath 'powershell.exe' -Arguments ('-NoProfile -ExecutionPolicy Bypass -File "' + (Join-Path $InstallDir 'Uninstall-App.ps1') + '"') -WorkingDirectory $InstallDir -Description ('卸载 ' + $appDisplayName) -IconLocation $installedExe
    }

    if (-not $SkipRegistration) {
        $displayVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($installedExe).FileVersion
        New-Item -Path $registryKey -Force | Out-Null
        Set-ItemProperty -Path $registryKey -Name DisplayName -Value $appDisplayName
        Set-ItemProperty -Path $registryKey -Name Publisher -Value $appPublisher
        Set-ItemProperty -Path $registryKey -Name DisplayVersion -Value $displayVersion
        Set-ItemProperty -Path $registryKey -Name DisplayIcon -Value $installedExe
        Set-ItemProperty -Path $registryKey -Name InstallLocation -Value $InstallDir
        Set-ItemProperty -Path $registryKey -Name UninstallString -Value ('powershell.exe -NoProfile -ExecutionPolicy Bypass -File "' + (Join-Path $InstallDir 'Uninstall-App.ps1') + '"')
        Set-ItemProperty -Path $registryKey -Name QuietUninstallString -Value ('powershell.exe -NoProfile -ExecutionPolicy Bypass -File "' + (Join-Path $InstallDir 'Uninstall-App.ps1') + '" -Quiet')
        Set-ItemProperty -Path $registryKey -Name NoModify -Value 1 -Type DWord
        Set-ItemProperty -Path $registryKey -Name NoRepair -Value 1 -Type DWord
    }

    if (-not $Quiet) {
        $message = "安装完成，已创建桌面和开始菜单快捷方式。"
        if (-not $NoLaunch) {
            $message += "`r`n`r`n现在打开软件吗？"
            $result = [System.Windows.Forms.MessageBox]::Show($message, $appDisplayName, [System.Windows.Forms.MessageBoxButtons]::YesNo, [System.Windows.Forms.MessageBoxIcon]::Information)
            if ($result -eq [System.Windows.Forms.DialogResult]::Yes) {
                Start-Process -FilePath $installedExe -WorkingDirectory $InstallDir
            }
        }
        else {
            [System.Windows.Forms.MessageBox]::Show($message, $appDisplayName, [System.Windows.Forms.MessageBoxButtons]::OK, [System.Windows.Forms.MessageBoxIcon]::Information) | Out-Null
        }
    }
    elseif (-not $NoLaunch) {
        Start-Process -FilePath $installedExe -WorkingDirectory $InstallDir
    }
}
finally {
    if (Test-Path -LiteralPath $tempExtractDir) {
        Remove-Item -LiteralPath $tempExtractDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}




