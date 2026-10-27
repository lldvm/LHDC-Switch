param([switch]$SkipNative)
$ErrorActionPreference = 'Stop'
$taskRoot = $PSScriptRoot
if (Test-Path (Join-Path $taskRoot 'data\pending-restore.json')) { throw 'Stop the LHDC session and restore Bluetooth before replacing binaries.' }
$csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
$engineRoot = Join-Path $taskRoot 'engine\A2DPWB-2.0.1'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$visualStudio = & $vswhere -latest -products '*' -property installationPath
$vcvars = Join-Path $visualStudio 'VC\Auxiliary\Build\vcvars64.bat'
if(!$SkipNative) {
Push-Location $taskRoot
try {
    $nativeCommand = '"' + $vcvars + '" >nul && cl.exe /nologo /EHsc /O2 /MT /std:c++17 src\AudioControl.cpp /Fe:AudioControl.exe /Fo:src\AudioControl.obj ole32.lib uuid.lib'
    & cmd.exe /d /c $nativeCommand
    if ($LASTEXITCODE -ne 0) { throw 'Audio control compilation failed' }
    $earbudCommand = '"' + $vcvars + '" >nul && cl.exe /nologo /EHsc /O2 /MT /std:c++17 src\EarbudControl.cpp /Fe:EarbudControl.exe /Fo:src\EarbudControl.obj ws2_32.lib ole32.lib uuid.lib'
    & cmd.exe /d /c $earbudCommand
    if ($LASTEXITCODE -ne 0) { throw 'Earbud control compilation failed' }
} finally { Pop-Location }
}
$coreHash = (Get-FileHash (Join-Path $engineRoot 'a2dpwb-core.exe') -Algorithm SHA256).Hash
$driverHash = (Get-FileHash (Join-Path $engineRoot 'a2dpwb-driver.exe') -Algorithm SHA256).Hash
$audioHash = (Get-FileHash (Join-Path $taskRoot 'AudioControl.exe') -Algorithm SHA256).Hash
$earbudHash = (Get-FileHash (Join-Path $taskRoot 'EarbudControl.exe') -Algorithm SHA256).Hash
$integrity = 'internal static class EngineIntegrity { internal const string Core = "' + $coreHash + '"; internal const string Driver = "' + $driverHash + '"; internal const string Audio = "' + $audioHash + '"; internal const string Earbud = "' + $earbudHash + '"; }'
[IO.File]::WriteAllText((Join-Path $taskRoot 'src\Integrity.cs'), $integrity, [Text.Encoding]::UTF8)
& $csc /nologo /target:winexe /platform:x64 /optimize+ /utf8output /win32manifest:"$taskRoot\src\app.manifest" /r:System.Windows.Forms.dll /r:System.Drawing.dll /r:System.Web.Extensions.dll /out:"$taskRoot\LHDC-Switch.exe" "$taskRoot\src\Program.cs" "$taskRoot\src\Integrity.cs" "$taskRoot\src\EngineClient.cs" "$taskRoot\src\ModernForm.cs" "$taskRoot\src\TraySupport.cs" "$taskRoot\src\EarbudPanel.cs" "$taskRoot\src\AssemblyInfo.cs"
if ($LASTEXITCODE -ne 0) { throw 'C# compilation failed' }
Write-Output 'Built LHDC-Switch.exe'
