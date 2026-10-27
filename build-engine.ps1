$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot 'reference')
try {
    $localCargo = Join-Path $PSScriptRoot 'tools\cargo'
    if (Test-Path (Join-Path $localCargo 'bin\cargo.exe')) {
        $env:CARGO_HOME = $localCargo
        $env:RUSTUP_HOME = Join-Path $PSScriptRoot 'tools\rustup'
        $env:PATH = (Join-Path $localCargo 'bin') + ';' + $env:PATH
    }
    cargo build --release --locked -p a2dpwb-core-exe --features aosp-rust
    if ($LASTEXITCODE -ne 0) { throw 'Engine compilation failed' }
    cargo test --release --locked -p codec --features aosp-rust --lib lhdcv5
    if ($LASTEXITCODE -ne 0) { throw 'Adaptive encoder tests failed' }
    cargo test --release --locked -p codec --features aosp-rust --test lhdcv5
    if ($LASTEXITCODE -ne 0) { throw 'LHDC packet tests failed' }
    cargo test --release --locked -p bt-host --lib radio_tests
    if ($LASTEXITCODE -ne 0) { throw 'HCI radio response tests failed' }
    cargo test --release --locked -p bt-host --test afh
    if ($LASTEXITCODE -ne 0) { throw 'AFH tests failed' }
    cargo test --release --locked -p ipc -p pipeline --lib
    if ($LASTEXITCODE -ne 0) { throw 'Telemetry and pipeline tests failed' }
    Write-Output 'Verified engine: reference\target\release\a2dpwb-core.exe. Stop the session before replacing the packaged engine and rebuilding the controller.'
} finally { Pop-Location }
