$ErrorActionPreference = "Stop"
$appDir = Join-Path $PSScriptRoot "WinGlow.App"
if (-not (Test-Path (Join-Path $appDir "package.json"))) {
  Write-Host "找不到 WinGlow.App 项目目录：$appDir" -ForegroundColor Red
  Read-Host "按回车关闭"
  exit 1
}
$nodeDir = Join-Path $env:USERPROFILE ".cache\codex-runtimes\codex-primary-runtime\dependencies\node\bin"
$cargoDir = Join-Path $env:USERPROFILE ".cargo\bin"
if (-not (Get-Command npm.cmd -ErrorAction SilentlyContinue) -and (Test-Path $nodeDir)) {
  $env:PATH = $nodeDir + ";" + $env:PATH
}
if (Test-Path $cargoDir) { $env:PATH = $cargoDir + ";" + $env:PATH }
if (-not (Get-Command npm.cmd -ErrorAction SilentlyContinue)) {
  Write-Host "没有找到 npm。请先安装 Node.js，再重新打开此入口。" -ForegroundColor Red
  Read-Host "按回车关闭"
  exit 1
}
Set-Location $appDir
if ((@("node_modules/typescript/bin/tsc", "node_modules/.bin/vite.cmd", "node_modules/.bin/tauri.cmd") |
    Where-Object { -not (Test-Path -LiteralPath $_) })) {
  Write-Host "正在补齐开发依赖…" -ForegroundColor Cyan
  & npm.cmd ci --no-audit --no-fund
  if ($LASTEXITCODE -ne 0) {
    Write-Host "开发依赖安装失败。" -ForegroundColor Red
    Read-Host "按回车关闭"
    exit 1
  }
}
$devPort = 1420
while (Get-NetTCPConnection -LocalPort $devPort -State Listen -ErrorAction SilentlyContinue) { $devPort++ }
$devConfigPath = Join-Path $env:TEMP "WinGlow-dev-$PID.json"
$devConfig = @{
  identifier = "com.sober.windowsfonttuner2.dev"
  build = @{
    beforeDevCommand = "node node_modules/vite/bin/vite.js --port $devPort --strictPort"
    devUrl = "http://localhost:$devPort"
  }
}
[IO.File]::WriteAllText($devConfigPath, ($devConfig | ConvertTo-Json -Depth 4))
Write-Host "正在启动 WinGlow 开发版。首次启动会先编译，请稍候。" -ForegroundColor Cyan
Write-Host "开发端口：$devPort" -ForegroundColor DarkGray
Write-Host "此窗口显示运行日志；关闭此窗口会停止开发服务。" -ForegroundColor DarkGray
try {
  & .\node_modules\.bin\tauri.cmd dev --config $devConfigPath
  $exitCode = $LASTEXITCODE
} finally {
  Remove-Item -LiteralPath $devConfigPath -ErrorAction SilentlyContinue
}
if ($exitCode -ne 0) {
  Write-Host "开发版启动失败，错误代码：$exitCode" -ForegroundColor Red
  Read-Host "按回车关闭"
}
exit $exitCode
