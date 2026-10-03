[CmdletBinding()]
param([string]$IconPath, [string]$PreviewPath)
$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
& python (Join-Path $repoRoot 'scripts\generate-brand-assets.py')
if ($LASTEXITCODE -ne 0) { throw 'WinGlow icon encoding failed. Python with Pillow is required.' }
if ($IconPath) { Copy-Item -LiteralPath (Join-Path $repoRoot 'Assets\AppIcon.ico') -Destination $IconPath -Force }
if ($PreviewPath) { Copy-Item -LiteralPath (Join-Path $repoRoot 'docs\branding\icon-master.png') -Destination $PreviewPath -Force }
