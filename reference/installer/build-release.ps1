<#
.SYNOPSIS
Builds the release files of A2DPWB for one architecture: the installer
(Tauri's NSIS bundler), the portable zip and the updater manifest.

.DESCRIPTION
Run on the release machine from any folder (PowerShell 7):

    pwsh installer/build-release.ps1 -Arch x64
    pwsh installer/build-release.ps1 -Arch arm64   # adds to latest.json

Writes to target/dist (or -OutDir):

    A2DPWB-<version>-win64-setup.exe        (arm64: -arm64-setup.exe)
    A2DPWB-<version>-win64-setup.exe.sig    updater signature
    A2DPWB-<version>-win64-portable.zip
    latest.json                             updater manifest of this release

Updater signing (see installer/README.md): TAURI_SIGNING_PRIVATE_KEY (the
private key file's path or content) and TAURI_SIGNING_PRIVATE_KEY_PASSWORD
sign the installer; A2DPWB_UPDATER_PUBKEY (the .pub file's path or content)
is built into a2dpwb.exe so that it accepts only updates signed with that
key. Without them the build still works, but its in-app update stays off
(the update check only shows the release page, as the portable version).
Neither key is part of the repository.

Needs: Rust (with the target), pnpm, the UI's node_modules (pnpm install),
the submodules under extern/, cargo-about (cargo install cargo-about).
Tauri downloads NSIS itself on first use.
#>
[CmdletBinding()]
param(
    [ValidateSet('x64', 'arm64')]
    [string]$Arch = 'x64',
    [string]$OutDir,
    # Release notes for latest.json (shown nowhere yet; kept for the record)
    [string]$NotesFile
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = Split-Path $PSScriptRoot -Parent
$ui = Join-Path $repo 'app/src/ui'
if (-not $OutDir) { $OutDir = Join-Path $repo 'target/dist' }
$OutDir = [IO.Path]::GetFullPath($OutDir)

$triple = @{ x64 = 'x86_64-pc-windows-msvc'; arm64 = 'aarch64-pc-windows-msvc' }[$Arch]
$platform = @{ x64 = 'windows-x86_64'; arm64 = 'windows-aarch64' }[$Arch]
$archName = @{ x64 = 'win64'; arm64 = 'arm64' }[$Arch]
$releaseUrl = 'https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/download'

function Invoke-Checked([string]$what, [scriptblock]$cmd) {
    & $cmd
    if ($LASTEXITCODE -ne 0) { throw "$what failed (exit code $LASTEXITCODE)" }
}

# --- Version: the workspace's, which package.json must repeat ---
$cargoToml = Get-Content -Raw (Join-Path $repo 'Cargo.toml')
if ($cargoToml -notmatch '(?ms)^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"') {
    throw 'no [workspace.package] version in Cargo.toml'
}
$version = $Matches[1]
$pkgVersion = (Get-Content -Raw (Join-Path $ui 'package.json') | ConvertFrom-Json).version
if ($pkgVersion -ne $version) {
    throw "app/src/ui/package.json has version $pkgVersion, the workspace ${version}: set both"
}
Write-Host "A2DPWB $version for $Arch ($triple)"

# --- Updater keys (from the environment, never from the repository) ---
$signing = [bool]$env:TAURI_SIGNING_PRIVATE_KEY
$pubkey = $env:A2DPWB_UPDATER_PUBKEY
if ($pubkey -and (Test-Path -LiteralPath $pubkey -PathType Leaf)) {
    $pubkey = (Get-Content -Raw -LiteralPath $pubkey).Trim()
}
if ($signing -and -not $pubkey) {
    throw 'TAURI_SIGNING_PRIVATE_KEY is set but A2DPWB_UPDATER_PUBKEY is not: the app could not check what it installs'
}
if (-not $signing) {
    Write-Warning 'TAURI_SIGNING_PRIVATE_KEY is not set: no updater signature, and the in-app update of this build stays off'
    $pubkey = ''
}

# --- ARM64 on an x64 machine: ring (rustls) builds its assembly with clang ---
if ($Arch -eq 'arm64' -and -not (Get-Command clang -ErrorAction SilentlyContinue)) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    $clang = if (Test-Path -LiteralPath $vswhere) {
        & $vswhere -latest -prerelease -products * -find 'VC\Tools\Llvm\x64\bin\clang.exe' | Select-Object -First 1
    }
    if (-not $clang) {
        throw 'ARM64 needs clang: add the Visual Studio component "C++ Clang Compiler for Windows" (or put LLVM on PATH)'
    }
    $env:PATH = "$(Split-Path $clang -Parent);$env:PATH"
}

# --- Folders ---
$stage = Join-Path $OutDir "stage-$Arch"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
$binDir = Join-Path $stage 'bin'
$docDir = Join-Path $stage 'doc'
New-Item -ItemType Directory -Force $binDir, (Join-Path $docDir 'licenses') | Out-Null

# --- The core and the driver helper (the UI is built by tauri build) ---
Invoke-Checked 'cargo build' {
    cargo build --release --locked --manifest-path (Join-Path $repo 'Cargo.toml') --target $triple -p a2dpwb-core-exe -p a2dpwb-driver
}
$release = Join-Path $repo "target/$triple/release"
# Sidecars: Tauri looks for <name>-<target triple>.exe and installs <name>.exe
foreach ($name in 'a2dpwb-core', 'a2dpwb-driver') {
    Copy-Item (Join-Path $release "$name.exe") (Join-Path $binDir "$name-$triple.exe")
}

# --- Licence documents (installed next to the programs, and in the zip) ---
$ext = Join-Path $repo 'extern'
$aptxLicense = Get-Content -Raw (Join-Path $ext 'libopenaptx/COPYING')
if ($aptxLicense -notmatch 'GNU LESSER GENERAL PUBLIC LICENSE\s+Version 2\.1') {
    throw 'extern/libopenaptx/COPYING is not LGPL-2.1: libopenaptx must be pinned to 0.2.0 (THIRD_PARTY_LICENSES.md)'
}
$docs = [ordered]@{
    'LICENSE'                              = Join-Path $repo 'LICENSE'
    'LICENSE-APACHE'                       = Join-Path $repo 'LICENSE-APACHE'
    'LICENSE-MIT'                          = Join-Path $repo 'LICENSE-MIT'
    'THIRD_PARTY_LICENSES.md'              = Join-Path $repo 'THIRD_PARTY_LICENSES.md'
    'licenses/BTstack-LICENSE.txt'         = Join-Path $ext 'btstack/LICENSE'
    'licenses/libldac-LICENSE.txt'         = Join-Path $ext 'libldac/LICENSE'
    'licenses/libldac-NOTICE.txt'          = Join-Path $ext 'libldac/NOTICE'
    # The Bluedroid SBC encoder in BTstack (ported to Rust) is Apache-2.0
    # without a licence file of its own; libldac's LICENSE is the full text
    'licenses/Apache-2.0.txt'              = Join-Path $ext 'libldac/LICENSE'
    'licenses/fdk-aac-NOTICE.txt'          = Join-Path $ext 'fdk-aac/NOTICE'
    'licenses/libopenaptx-LGPL-2.1.txt'    = Join-Path $ext 'libopenaptx/COPYING'
    'licenses/lhdcv5-enc-LICENSE.txt'      = Join-Path $ext 'lhdcv5-enc/LICENSE'
    'licenses/opus-COPYING.txt'            = Join-Path $ext 'opus/COPYING'
    # LE Audio (experimental): LC3 encoder and LE Secure Connections' P-256
    'licenses/liblc3-LICENSE.txt'          = Join-Path $ext 'btstack/3rd-party/lc3-google/LICENSE'
    'licenses/micro-ecc-LICENSE.txt'       = Join-Path $ext 'btstack/3rd-party/micro-ecc/LICENSE.txt'
    'licenses/fluentui-system-icons.txt'   = Join-Path $ui 'LICENSES/fluentui-system-icons.txt'
    'licenses/solid-js-LICENSE.txt'        = Join-Path $ui 'node_modules/solid-js/LICENSE'
    'licenses/tauri-apps-api-LICENSE-MIT.txt' = Join-Path $ui 'node_modules/@tauri-apps/api/LICENSE-MIT'
}
foreach ($entry in $docs.GetEnumerator()) {
    if (-not (Test-Path -LiteralPath $entry.Value -PathType Leaf)) {
        throw "required file not found: $($entry.Value) (git submodule update --init --recursive; pnpm install in app/src/ui)"
    }
    Copy-Item -LiteralPath $entry.Value (Join-Path $docDir $entry.Key)
}
Invoke-Checked 'cargo about' {
    cargo about generate --workspace --locked --fail --manifest-path (Join-Path $repo 'Cargo.toml') `
        -c (Join-Path $repo 'about.toml') --target $triple `
        -o (Join-Path $docDir 'licenses/rust-crates.txt') (Join-Path $PSScriptRoot 'about.hbs')
}

# SOURCE.txt: where the corresponding source is (LGPL-2.1 libopenaptx)
$commit = (git -C $repo rev-parse HEAD).Trim()
$dirty = git -C $repo status --porcelain --untracked-files=no -- app Cargo.toml Cargo.lock extern THIRD_PARTY_LICENSES.md
$submodules = (git -C $repo submodule status) -join "`n"
$dirtyNote = if ($dirty) {
    "`nNOTE: this package was built from a working tree with uncommitted`nlocal modifications on top of the commit above.`n"
} else { '' }
@"
A2DP Windows Bridge (A2DPWB) $version ($Arch)

Source code
-----------
Repository: https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge
Commit:     $commit
$dirtyNote
Pinned submodules (git submodule status: commit, path, description):
$submodules

The complete corresponding source code of these programs and of every
library statically linked into them - including the LGPL-2.1-or-later
libopenaptx and the Fraunhofer FDK AAC codec - is available free of charge
at the repository URL and commit above.
It contains everything needed to rebuild and relink the programs (e.g. with
a modified version of libopenaptx):

    git clone --recursive https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge.git
    git checkout $commit
    git submodule update --init --recursive
    pwsh installer/build-release.ps1

See LICENSE, THIRD_PARTY_LICENSES.md and the licenses/ folder for the terms
of each component; licenses/rust-crates.txt lists the Rust crates.
"@ | Set-Content -Encoding utf8NoBOM (Join-Path $docDir 'SOURCE.txt')

# --- The installer: Tauri's configuration plus this build's files ---
# Paths relative to src-tauri: Tauri mangles absolute ones
$tauriDir = Join-Path $ui 'src-tauri'
if ([IO.Path]::GetPathRoot($stage) -ne [IO.Path]::GetPathRoot($tauriDir)) {
    throw "-OutDir must be on the repository's drive ($([IO.Path]::GetPathRoot($tauriDir)))"
}
$rel = { param($base, $p) [IO.Path]::GetRelativePath($base, $p) -replace '\\', '/' }
$resources = [ordered]@{}
Get-ChildItem -Recurse -File $docDir | ForEach-Object {
    $resources[(& $rel $tauriDir $_.FullName)] = (& $rel $docDir $_.FullName)
}
$overlay = [ordered]@{
    bundle  = [ordered]@{
        externalBin            = @((& $rel $tauriDir (Join-Path $binDir 'a2dpwb-core')), (& $rel $tauriDir (Join-Path $binDir 'a2dpwb-driver')))
        resources              = $resources
        createUpdaterArtifacts = $signing
    }
    plugins = @{ updater = @{ pubkey = $pubkey } }
}
$overlayPath = Join-Path $stage 'tauri.release.conf.json'
$overlay | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8NoBOM $overlayPath

Push-Location $ui
try {
    Invoke-Checked 'tauri build' { pnpm tauri build --target $triple --bundles nsis --config $overlayPath }
} finally {
    Pop-Location
}

$nsisDir = Join-Path $release 'bundle/nsis'
$built = Get-ChildItem (Join-Path $nsisDir "*_${version}_*-setup.exe") | Sort-Object LastWriteTime | Select-Object -Last 1
if (-not $built) { throw "no installer in $nsisDir" }
New-Item -ItemType Directory -Force $OutDir | Out-Null
$setupName = "A2DPWB-$version-$archName-setup.exe"
$setup = Join-Path $OutDir $setupName
Copy-Item $built.FullName $setup -Force
if ($signing) {
    Copy-Item "$($built.FullName).sig" "$setup.sig" -Force
}

# --- Portable zip: the same files, no installer, notify-only updates ---
$pkgName = "A2DPWB-$version"
$pkgDir = Join-Path $stage $pkgName
New-Item -ItemType Directory -Force $pkgDir | Out-Null
Copy-Item (Join-Path $release 'a2dpwb.exe') $pkgDir
foreach ($name in 'a2dpwb-core', 'a2dpwb-driver') {
    Copy-Item (Join-Path $release "$name.exe") $pkgDir
}
Copy-Item -Recurse (Join-Path $docDir '*') $pkgDir
$zip = Join-Path $OutDir "$pkgName-$archName-portable.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $pkgDir -DestinationPath $zip

# --- Updater manifest (one per release; arm64 is added to x64's) ---
if ($signing) {
    $manifestPath = Join-Path $OutDir 'latest.json'
    $manifest = $null
    if (Test-Path $manifestPath) {
        $manifest = Get-Content -Raw $manifestPath | ConvertFrom-Json -AsHashtable
        if ($manifest.version -ne $version) { $manifest = $null }
    }
    if (-not $manifest) {
        $notes = if ($NotesFile) { Get-Content -Raw $NotesFile } else { '' }
        $manifest = [ordered]@{
            version   = $version
            notes     = $notes
            pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd'T'HH:mm:ss'Z'")
            platforms = [ordered]@{}
        }
    }
    $manifest.platforms[$platform] = [ordered]@{
        signature = (Get-Content -Raw "$setup.sig").Trim()
        url       = "$releaseUrl/v$version/$setupName"
    }
    $manifest | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8NoBOM $manifestPath
}

Write-Host ''
Write-Host 'Release files:'
Get-ChildItem $OutDir -File | Where-Object { $_.Name -like "A2DPWB-$version-*" -or $_.Name -eq 'latest.json' } |
    ForEach-Object { Write-Host ("  {0}  ({1:N1} MB)" -f $_.Name, ($_.Length / 1MB)) }
