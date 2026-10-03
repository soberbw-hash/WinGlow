[CmdletBinding()]
param(
    [string]$Version,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not $Version) {
    $assemblyInfo = Get-Content -LiteralPath (Join-Path $root 'Properties\AssemblyInfo.cs') -Raw -Encoding UTF8
    $match = [regex]::Match($assemblyInfo, 'AssemblyFileVersion\("(?<version>\d+\.\d+\.\d+)\.\d+"\)')
    if (-not $match.Success) {
        throw '无法从 AssemblyInfo.cs 读取版本号。'
    }

    $Version = $match.Groups['version'].Value
}

if (-not $SkipBuild) {
    & (Join-Path $root 'build.bat')
    if ($LASTEXITCODE -ne 0) {
        throw '构建失败。'
    }
}

$distDir = Join-Path $root 'dist'
$artifactsDir = Join-Path $root 'artifacts'
$zipStageDir = Join-Path $artifactsDir ("WinGlow-v$Version-win64")
$zipPath = Join-Path $distDir ("WinGlow-v$Version-win64.zip")
$installerWorkDir = Join-Path $artifactsDir ("installer-v$Version")
$installerExePath = Join-Path $distDir ("WinGlow-Setup-v$Version.exe")

if (Test-Path -LiteralPath $zipStageDir) { Remove-Item -LiteralPath $zipStageDir -Recurse -Force }
if (Test-Path -LiteralPath $zipPath) { Remove-Item -LiteralPath $zipPath -Force }
if (Test-Path -LiteralPath $installerWorkDir) { Remove-Item -LiteralPath $installerWorkDir -Recurse -Force }
if (Test-Path -LiteralPath $installerExePath) { Remove-Item -LiteralPath $installerExePath -Force }

New-Item -ItemType Directory -Path $zipStageDir -Force | Out-Null
New-Item -ItemType Directory -Path $installerWorkDir -Force | Out-Null
New-Item -ItemType Directory -Path $distDir -Force | Out-Null

Copy-Item -Path (Join-Path $root 'bin\Release\*') -Destination $zipStageDir -Recurse -Force
$pdbPath = Join-Path $zipStageDir 'WinGlow.pdb'
if (Test-Path -LiteralPath $pdbPath) {
    Remove-Item -LiteralPath $pdbPath -Force
}

Compress-Archive -Path (Join-Path $zipStageDir '*') -DestinationPath $zipPath -CompressionLevel Optimal

Copy-Item -LiteralPath $zipPath -Destination (Join-Path $installerWorkDir 'AppPackage.zip') -Force
Copy-Item -LiteralPath (Join-Path $root 'installer\Install-App.ps1') -Destination (Join-Path $installerWorkDir 'Install-App.ps1') -Force
Copy-Item -LiteralPath (Join-Path $root 'installer\Uninstall-App.ps1') -Destination (Join-Path $installerWorkDir 'Uninstall-App.ps1') -Force

$sedPath = Join-Path $installerWorkDir 'installer.sed'
$targetName = $installerExePath.Replace('\', '\\')
$sourcePath = $installerWorkDir.Replace('\', '\\')

$sedContent = @"
[Version]
Class=IEXPRESS
SEDVersion=3
[Options]
PackagePurpose=InstallApp
ShowInstallProgramWindow=0
HideExtractAnimation=1
UseLongFileName=1
InsideCompressed=0
CAB_FixedSize=0
CAB_ResvCodeSigning=0
RebootMode=I
InstallPrompt=
DisplayLicense=
FinishMessage=
TargetName=$targetName
FriendlyName=WinGlow Setup
AppLaunched=powershell.exe -NoProfile -ExecutionPolicy Bypass -File Install-App.ps1
PostInstallCmd=<None>
AdminQuietInstCmd=
UserQuietInstCmd=
SourceFiles=SourceFiles
[Strings]
FILE0=Install-App.ps1
FILE1=Uninstall-App.ps1
FILE2=AppPackage.zip
[SourceFiles]
SourceFiles0=$sourcePath
[SourceFiles0]
%FILE0%=
%FILE1%=
%FILE2%=
"@
Set-Content -LiteralPath $sedPath -Value $sedContent -Encoding ASCII

& "$env:WINDIR\System32\iexpress.exe" /N /Q $sedPath

$deadline = (Get-Date).AddMinutes(3)
while ((Get-Date) -lt $deadline) {
    if ((Test-Path -LiteralPath $installerExePath) -and ((Get-Item -LiteralPath $installerExePath).Length -gt 0)) {
        break
    }

    Start-Sleep -Seconds 2
}

if (-not (Test-Path -LiteralPath $installerExePath)) {
    throw 'IExpress 安装包构建失败。'
}

Get-ChildItem -LiteralPath $distDir -Force | Where-Object { $_.Name -like ("~WinGlow-Setup-v$Version.*") } | Remove-Item -Force -ErrorAction SilentlyContinue

[PSCustomObject]@{
    Version = $Version
    ZipPath = $zipPath
    InstallerPath = $installerExePath
} | Format-List





