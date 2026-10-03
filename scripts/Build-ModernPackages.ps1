param([switch]$SkipChecks)
$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$appRoot = Join-Path $repoRoot 'WinGlow.App'
$config = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $appRoot 'src-tauri\tauri.conf.json') | ConvertFrom-Json
$package = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $appRoot 'package.json') | ConvertFrom-Json
if ($config.version -ne $package.version) { throw 'Frontend and package versions do not match.' }
$version = $config.version
$outRoot = Join-Path $repoRoot "artifacts\WinGlow-$version"
$portable = Join-Path $outRoot "WinGlow-$version-Portable"
function Invoke-Checked([string]$program, [string[]]$arguments) {
    & $program @arguments
    if ($LASTEXITCODE -ne 0) { throw "$program failed with exit code $LASTEXITCODE." }
}
Push-Location $appRoot
try {
    if (-not $SkipChecks) {
        Invoke-Checked 'npm.cmd' @('ci')
        Invoke-Checked 'cargo' @('fmt', '--manifest-path', 'src-tauri/Cargo.toml', '--check')
        Invoke-Checked 'cargo' @('clippy', '--manifest-path', 'src-tauri/Cargo.toml', '--all-targets', '--', '-D', 'warnings')
        Invoke-Checked 'cargo' @('test', '--manifest-path', 'src-tauri/Cargo.toml')
    }
    Invoke-Checked 'npm.cmd' @('run', 'tauri', 'build', '--', '--bundles', 'nsis')
    New-Item -ItemType Directory -Path $portable -Force | Out-Null
    $setup = Join-Path $appRoot "src-tauri\target\release\bundle\nsis\$($config.productName)_${version}_x64-setup.exe"
    Copy-Item -LiteralPath $setup -Destination (Join-Path $outRoot "WinGlow-$version-Setup.exe") -Force
    # Tauri mainBinaryName supplies the renamed executable for both package formats.
    Copy-Item -LiteralPath (Join-Path $appRoot 'src-tauri\target\release\WinGlow.exe') -Destination $portable -Force
    $licenses = Join-Path $portable 'licenses'
    New-Item -ItemType Directory -Path $licenses -Force | Out-Null
    foreach ($family in @('harmonyos-sc','source-han-sans-cn')) {
        Copy-Item -LiteralPath (Join-Path $repoRoot "FontPackages\$family\LICENSE.txt") -Destination (Join-Path $licenses "$family-LICENSE.txt") -Force
        Copy-Item -LiteralPath (Join-Path $repoRoot "FontPackages\$family\SOURCE.txt") -Destination (Join-Path $licenses "$family-SOURCE.txt") -Force
    }
    Copy-Item -LiteralPath (Join-Path $repoRoot 'LICENSE') -Destination (Join-Path $licenses 'APP-LICENSE.txt') -Force
    Copy-Item -LiteralPath (Join-Path $repoRoot 'licenses\Breeze-AGPL-3.0.txt') -Destination $licenses -Force
    Copy-Item -LiteralPath (Join-Path $repoRoot 'licenses\THIRD-PARTY.txt') -Destination $licenses -Force
    Copy-Item -LiteralPath (Join-Path $repoRoot 'FontPackages\pingfang-sc\NOTICE.txt') -Destination (Join-Path $licenses 'PingFang-NOTICE.txt') -Force
    Copy-Item -LiteralPath (Join-Path $repoRoot 'FontPackages\pingfang-sc\SOURCE.txt') -Destination (Join-Path $licenses 'PingFang-SOURCE.txt') -Force
    $branding = Join-Path $portable 'branding'
    New-Item -ItemType Directory -Path $branding -Force | Out-Null
    foreach ($name in @('icon-master.png','brand-source.json','README.md')) {
        Copy-Item -LiteralPath (Join-Path $repoRoot "docs\branding\$name") -Destination $branding -Force
    }
    # Ship the usage guide rather than repository documentation with source-only links.
    $guide = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $repoRoot '使用说明.txt')
    $guide | Set-Content -LiteralPath (Join-Path $portable 'README.md') -Encoding UTF8
    Compress-Archive -LiteralPath $portable -DestinationPath (Join-Path $outRoot "WinGlow-$version-Portable.zip") -Force
    $lines = Get-ChildItem -LiteralPath $outRoot -File | Where-Object {$_.Extension -in '.exe','.zip'} | ForEach-Object { "$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $($_.Name)" }
    $lines | Set-Content -LiteralPath (Join-Path $outRoot 'SHA256SUMS.txt') -Encoding utf8
    Write-Output "Packages saved to $outRoot"
} finally { Pop-Location }
