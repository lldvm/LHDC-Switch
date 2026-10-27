param([string]$Address,[string]$Headphone='LHDC 耳机',[string]$InstanceId,[switch]$ListDevices)
$ErrorActionPreference='Stop'
$taskDevices=@(Get-PnpDevice -PresentOnly | Where-Object {$_.InstanceId -like 'USB\VID_8087&PID_0026\*'})
if($ListDevices){$taskDevices | Select-Object FriendlyName,InstanceId,Status; return}
if(Test-Path -LiteralPath (Join-Path $PSScriptRoot 'data\pending-restore.json')){throw '请先恢复蓝牙，再修改目标设备。'}
foreach($taskProcess in Get-Process LHDC-Switch -ErrorAction SilentlyContinue){
    if($taskProcess.Path -eq (Join-Path $PSScriptRoot 'LHDC-Switch.exe')){throw '请从托盘退出本目录的程序，再配置设备。'}
}
if(!$InstanceId){
    if($taskDevices.Count -ne 1){throw '未找到唯一的 Intel 8087:0026 设备；此版本不支持其他控制器。'}
    $InstanceId=$taskDevices[0].InstanceId
}
if(!($taskDevices | Where-Object {$_.InstanceId -eq $InstanceId})){throw '目标不是当前存在的 Intel 8087:0026 设备。'}
if(!$Address){$Address=Read-Host '输入耳机蓝牙地址（例 AA:BB:CC:DD:EE:FF，可在设备管理器查看）'}
if($Address -notmatch '\A(?:[0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}\z' -or $Address -eq '00:00:00:00:00:00'){throw '耳机蓝牙地址无效。'}
$taskPath=Join-Path $PSScriptRoot 'settings.json'
$taskTemplate=if(Test-Path -LiteralPath $taskPath){$taskPath}else{Join-Path $PSScriptRoot 'settings.example.json'}
$taskConfig=Get-Content -LiteralPath $taskTemplate -Raw -Encoding UTF8 | ConvertFrom-Json
$taskConfig.InstanceId=$InstanceId; $taskConfig.Address=$Address.ToUpperInvariant(); $taskConfig.Headphone=$Headphone
[IO.File]::WriteAllText($taskPath,($taskConfig | ConvertTo-Json),[Text.UTF8Encoding]::new($false))
Write-Output '配置已保存。此操作没有切换蓝牙驱动；现在可以打开 LHDC-Switch.exe。'
