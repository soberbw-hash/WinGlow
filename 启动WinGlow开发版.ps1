$ErrorActionPreference = "Stop"
$appDir = Join-Path $PSScriptRoot "WinGlow.App"
if (-not (Test-Path (Join-Path $appDir "package.json"))) {
  Write-Host "找不到 WinGlow.App 项目目录：$appDir" -ForegroundColor Red
  Read-Host "按回车关闭"
  exit 1
}
$pnpmCommand = Get-Command pnpm.cmd -ErrorAction SilentlyContinue
if (-not $pnpmCommand) {
  $fallbackPnpm = Join-Path $env:USERPROFILE ".cache\codex-runtimes\codex-primary-runtime\dependencies\bin\fallback\pnpm.cmd"
  if (Test-Path $fallbackPnpm) { $pnpmPath = $fallbackPnpm }
} else {
  $pnpmPath = $pnpmCommand.Source
}
if (-not $pnpmPath) {
  Write-Host "没有找到 pnpm。请先安装 Node.js 和 pnpm，再重新打开此入口。" -ForegroundColor Red
  Read-Host "按回车关闭"
  exit 1
}
$nodeDir = Join-Path $env:USERPROFILE ".cache\codex-runtimes\codex-primary-runtime\dependencies\node\bin"
$cargoDir = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $nodeDir) { $env:PATH = $nodeDir + ";" + $env:PATH }
if (Test-Path $cargoDir) { $env:PATH = $cargoDir + ";" + $env:PATH }
$shimDir = Join-Path $env:TEMP "WinGlow-dev-tools"
New-Item -ItemType Directory -Path $shimDir -Force | Out-Null
$npmShimPath = Join-Path $shimDir "npm.cmd"
$devPort = 1420
while (Get-NetTCPConnection -LocalPort $devPort -State Listen -ErrorAction SilentlyContinue) { $devPort++ }
$devConfigPath = Join-Path $env:TEMP "WinGlow-dev-$devPort.json"
$npmShim = "@echo off`r`ncall `"$pnpmPath`" exec vite --port $devPort --strictPort`r`nexit /b %ERRORLEVEL%"
Set-Content -LiteralPath $npmShimPath -Value $npmShim -Encoding ASCII
$env:PATH = $shimDir + ";" + $env:PATH
$devConfig = @{ identifier = "com.sober.windowsfonttuner2.dev"; build = @{ beforeDevCommand = "npm run dev"; devUrl = "http://localhost:$devPort" } }
$devConfig | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $devConfigPath -Encoding UTF8
Set-Location $appDir
Write-Host "正在启动 WinGlow 开发版。首次启动会先编译，请稍候。" -ForegroundColor Cyan
Write-Host "开发端口：$devPort" -ForegroundColor DarkGray
Write-Host "此窗口显示运行日志；关闭此窗口会停止开发服务。" -ForegroundColor DarkGray
& $pnpmPath tauri dev --config $devConfigPath
$exitCode = $LASTEXITCODE
if ($exitCode -ne 0) {
  Write-Host "开发版启动失败，错误代码：$exitCode" -ForegroundColor Red
  Read-Host "按回车关闭"
}
exit $exitCode
