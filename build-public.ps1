param([switch]$SkipEngineBuild)
$ErrorActionPreference='Stop'
if(Test-Path -LiteralPath (Join-Path $PSScriptRoot 'data\pending-restore.json')){throw '请先恢复蓝牙，再重建。'}
foreach($taskProcess in Get-Process LHDC-Switch -ErrorAction SilentlyContinue){
    if($taskProcess.Path -eq (Join-Path $PSScriptRoot 'LHDC-Switch.exe')){throw '请先退出本目录的程序。'}
}
if(!(Test-Path -LiteralPath (Join-Path $PSScriptRoot 'reference\extern\btstack\LICENSE'))){throw '请先执行 git submodule update --init --recursive。'}
if(!$SkipEngineBuild){
    & (Join-Path $PSScriptRoot 'build-engine.ps1')
    Push-Location (Join-Path $PSScriptRoot 'reference')
    try {cargo build --release --locked -p a2dpwb-driver; if($LASTEXITCODE -ne 0){throw 'Driver helper build failed'}} finally {Pop-Location}
}
$taskEngine=Join-Path $PSScriptRoot 'engine\A2DPWB-2.0.1'
foreach($taskName in @('a2dpwb-core.exe','a2dpwb-driver.exe')){
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot ('reference\target\release\'+$taskName)) -Destination $taskEngine -Force
}
& (Join-Path $PSScriptRoot 'build.ps1')
Write-Output '构建完成。先运行 configure.ps1 配置设备。源码自检可临时复制 settings.example.json 为 settings.json。'
