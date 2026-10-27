param([string]$OutputPath=(Join-Path $PSScriptRoot 'dist\LHDC-Switch-v2.4.2-win-x64.zip'))
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$taskOutput=[IO.Path]::GetFullPath($OutputPath)
if(Test-Path -LiteralPath $taskOutput){throw '输出文件已存在，请指定新的文件名。'}
[IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($taskOutput)) | Out-Null
$taskNames=@('LHDC-Switch.exe','LHDC-Switch.exe.config','AudioControl.exe','EarbudControl.exe','启动.cmd','恢复蓝牙.cmd',
 '配置设备.cmd','configure.ps1','settings.example.json','variant.json','README.md','使用说明.md','ACKNOWLEDGEMENTS.md',
 'THIRD_PARTY_NOTICES.md','SOURCE.md','LICENSE','docs','firmware','dependencies','engine')
$taskStream=[IO.File]::Open($taskOutput,[IO.FileMode]::CreateNew)
$taskZip=[IO.Compression.ZipArchive]::new($taskStream,[IO.Compression.ZipArchiveMode]::Create,$false)
try {
 foreach($taskName in $taskNames){
  $taskPath=Join-Path $PSScriptRoot $taskName
  if(!(Test-Path -LiteralPath $taskPath)){throw ('缺少文件：'+$taskName)}
  $taskFiles=if(Test-Path -LiteralPath $taskPath -PathType Container){Get-ChildItem -LiteralPath $taskPath -Recurse -File}else{Get-Item -LiteralPath $taskPath}
  foreach($taskFile in $taskFiles){
   $taskEntry=$taskFile.FullName.Substring($PSScriptRoot.Length+1).Replace('\','/')
   if($taskFile.Extension -eq '.exe' -and $taskEntry -eq 'engine/A2DPWB-2.0.1/a2dpwb.exe'){continue}
   [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($taskZip,$taskFile.FullName,$taskEntry,[IO.Compression.CompressionLevel]::Optimal) | Out-Null
  }
 }
} finally {$taskZip.Dispose()}
Get-FileHash -LiteralPath $taskOutput -Algorithm SHA256
