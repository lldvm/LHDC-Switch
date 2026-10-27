# Releasing A2DPWB

`build-release.ps1` builds everything for one architecture. Run it once per
architecture on the release machine, into the same output folder:

```powershell
pwsh installer/build-release.ps1 -Arch x64
pwsh installer/build-release.ps1 -Arch arm64   # needs the VS "MSVC v143 ARM64 build tools" (+ "C++ Clang Compiler" on x64)
```

Output (`target/dist`):

| File | What it is |
| --- | --- |
| `A2DPWB-<ver>-win64-setup.exe` / `-arm64-setup.exe` | Installer (Tauri's NSIS bundler, `app/src/ui/src-tauri/tauri.conf.json`) |
| `A2DPWB-<ver>-win64-setup.exe.sig` / `-arm64-…` | Updater signature of the installer |
| `A2DPWB-<ver>-win64-portable.zip` / `-arm64-…` | Portable copy |
| `latest.json` | Updater manifest of this release (both architectures) |

Attach all of them to the GitHub release `v<ver>` (`gh release create v<ver> ... --prerelease` for a
pre-release). The version comes from `version` in `Cargo.toml` (`[workspace.package]`);
`app/src/ui/package.json` must have the same one, and the tag must be `v` + that version.

Prerequisites: Rust with the target (`rustup target add aarch64-pc-windows-msvc`), pnpm and
`pnpm install` in `app/src/ui`, the submodules (`git submodule update --init --recursive`),
`cargo install cargo-about`. Tauri downloads NSIS on first use. Build from a clean checkout of
the release commit: `SOURCE.txt` in the package names the commit and says so if the tree had
local changes.

## Updater signing keys

The installer is signed with a minisign key pair of the updater (Tauri's `tauri signer`). The
app has the public key built in and installs only updates signed with the private key. **The
keys are not in the repository.** On the release machine:

1. Once: `pnpm tauri signer generate -w %USERPROFILE%\.tauri\a2dpwb-updater.key` (in `app/src/ui`),
   with a password. Keep a backup of the `.key` file and the password offline: without them no
   installed copy can be updated any more (a new key means a manual reinstall for everyone).
2. Before each build, in the same PowerShell:

   ```powershell
   $env:TAURI_SIGNING_PRIVATE_KEY = "$env:USERPROFILE\.tauri\a2dpwb-updater.key"   # path or content
   $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = '<password>'
   $env:A2DPWB_UPDATER_PUBKEY = "$env:USERPROFILE\.tauri\a2dpwb-updater.key.pub"  # path or content
   ```

`build-release.ps1` passes the public key to the build in a generated configuration
(`stage-<arch>/tauri.release.conf.json`, merged with `tauri build --config`). The public key is
not secret; once it is final it may also be committed as `plugins.updater.pubkey` in
`tauri.conf.json`. Without `TAURI_SIGNING_PRIVATE_KEY` the build still works, but writes no
`.sig` and no `latest.json`, and its in-app update stays off.

## How updates find a release

- The resident core checks GitHub once a day (`crates/core/src/update.rs`): the latest release,
  or with "include pre-releases" the highest version of the last 20 releases. It reports the
  release with its manifest URL, `https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/download/<tag>/latest.json`.
- Every release carries its own `latest.json`, so a pre-release's manifest is separate from the
  stable one's and only reaches the pre-release channel. `x64` and `ARM64` are two platforms of the
  same manifest (`windows-x86_64`, `windows-aarch64`), which is why both builds write into the same
  output folder.
- "Update now" (Settings) in an installed copy downloads the installer from the manifest
  (`app/src/ui/src-tauri/src/update.rs`) and checks its signature. While something streams it
  waits; then it asks the core to exit (`app.quit_for_update`, refused while streaming), runs the
  installer in passive mode and exits. The installer starts the UI again, which starts the core.
- The portable copy (no `uninstall.exe` next to `a2dpwb.exe`) and builds without a key only
  open the release page.

## What the installer does (`hooks.nsh`)

- Installs for the current user (no administrator rights), in 8 languages, with the WebView2
  bootstrapper embedded (it runs only where WebView2 is missing).
- Asks a running v1 to exit (its mutex `A2DPWB_SingleInstance`), closes a running v2 core with
  `a2dpwb-core.exe --quit` (stream stopped, adapter released) and starts it again afterwards.
- Uninstalls v1 when Inno Setup installed it (for the user or for all users). `%APPDATA%\A2DPWB`
  stays; at its first start the core copies it to `backup-v1\` and adds `schema_version` to
  `settings.json` (`crates/core/src/migrate.rs`).
- Points "start with Windows" (HKCU Run, value `A2DPWB`) at `a2dpwb-core.exe --background` when
  it was on; the uninstaller removes it when it points at that installation.
- The uninstaller gives the adapters A2DPWB put on WinUSB back to Windows, after asking. The core
  keeps one record per assigned adapter in `%APPDATA%\A2DPWB\driver\` (the `assign` result,
  deleted by **Restore driver**); when there are records, the uninstaller asks (No is the default
  while v1 is running or installed, since v1 needs WinUSB too) and runs
  `a2dpwb-driver.exe restore --records <folder>` elevated: one UAC prompt for all adapters, and the
  records of the adapters given back are deleted. An unplugged adapter keeps its record and the
  uninstaller points to Device Manager. Not on updates, and not in silent mode (`/S`: nobody to
  ask, and no UAC prompt), where the adapters stay on WinUSB. An adapter that already had WinUSB
  (Zadig) when A2DPWB was asked to assign it is left alone.
- `/KEEPV1` on the command line leaves v1, a running v1 and the Run value alone. Only for testing
  v2 beside an installed v1 (both use the same settings folder). The core still replaces v1 when it
  starts, as in a normal install: it asks a running v1 to exit and takes over v1's Run value.
  For example, a throwaway install: `A2DPWB-<ver>-win64-setup.exe /S /NS /KEEPV1 /D=C:\temp\a2dpwb-test`
  (`/NS`: no shortcuts; `/D` must come last). Remove it with its `uninstall.exe /S`, then delete
  `HKCU\Software\Seiya Funaoka\A2DP Windows Bridge`: Tauri's uninstaller keeps that folder there,
  and the next install would offer it again.

## Portable copy

`A2DPWB-<ver>/` in the zip: `a2dpwb.exe`, `a2dpwb-core.exe`, `a2dpwb-driver.exe`, the licence
documents and `SOURCE.txt`. It uses the same settings folder as the installed copy. WebView2
must be present (Windows 11 has it). Its update check only shows the release page.

## Development: a second core

`A2DPWB_INSTANCE=<name>` gives a core its own pipe and instance mutex, so it runs beside the
installed one, e.g. for measurements; set `A2DPWB_CONFIG_DIR` to a test folder as well, or it
uses the real settings folder.

```powershell
$env:A2DPWB_CONFIG_DIR = "$env:TEMP\a2dpwb-test"; $env:A2DPWB_INSTANCE = 'test'
v2\target\release\a2dpwb-core.exe --background
v2\target\release\a2dpwb-core.exe --quit
```
