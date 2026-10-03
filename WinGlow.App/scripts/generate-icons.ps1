$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
& python (Join-Path $repoRoot 'scripts\generate-brand-assets.py')
if ($LASTEXITCODE -ne 0) { throw 'WinGlow icon encoding failed. Python with Pillow is required.' }
