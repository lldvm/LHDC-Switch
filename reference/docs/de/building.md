---
title: Bauen
layout: default
parent: Startseite
nav_order: 3
---

# Aus dem Quellcode bauen
{: .no_toc }

## Inhalt
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Voraussetzungen

- **Rust** (stable, 1.90 oder neuer) über [rustup](https://rustup.rs/)
- **Visual Studio 2022 oder neuer** (oder dessen Build Tools) mit der Workload „Desktopentwicklung mit C++“: Die C-Bibliotheken (BTstack, die Codecs) werden mit MSVC kompiliert
- **Node.js** 22 und **pnpm** für die Oberfläche (`corepack enable` stellt pnpm bereit)
- **Git** (für die Submodule)
- **PowerShell 7** (`pwsh`) für `build.ps1` / `build.bat` (`winget install Microsoft.PowerShell`)
- Für ARM64-Builds: die Visual-Studio-Komponente „MSVC v143 - VS 2022 C++ ARM64 build tools“, auf einem x64-PC zusätzlich „C++ Clang Compiler for Windows“ (das `ring` von rustls assembliert seinen ARM64-Code mit clang; `installer/build-release.ps1` findet das clang von Visual Studio selbst), und `rustup target add aarch64-pc-windows-msvc`

## Schnellstart

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge.git
cd A2DP-Windows-Bridge
pwsh ./build.ps1
```

Oder doppelklicken Sie im Explorer auf `build.bat`: Es führt `build.ps1` aus und wartet am Ende auf eine Taste (`build.bat -Test` reicht die Schalter weiter). `build.ps1` prüft zuerst, ob Rust, Node.js, pnpm und die Submodule vorhanden sind, führt `pnpm install` aus, wenn `node_modules` fehlt, baut die Seite des Fensters und danach die drei Programme nach `target/release/`: `a2dpwb-core.exe` (der residente Kern: Bluetooth, Audio, Infobereich), `a2dpwb.exe` (das Fenster) und `a2dpwb-driver.exe` (das Hilfsprogramm mit Administratorrechten, das den WinUSB-Treiber zuweist / wiederherstellt). Lassen Sie die drei in einem Ordner; `a2dpwb-core.exe --cli` streamt ohne das Fenster.

Schalter: `-Debug` (Dev-Profil, `target/debug/`), `-Test` (danach die Prüfungen unter „Tests“), `-Decoder` (auch das Decoder-Werkzeug), `-Emu` (auch der [Test ohne Hardware](#emu)), `-Release -Arch x64|arm64` (nur das Release-Skript, siehe „Release-Pakete“).

**Schritt für Schritt** (was `build.ps1` tut; die Seite des Fensters muss vor `a2dpwb.exe` gebaut werden, das sie einbettet):

```bash
cargo build --release -p a2dpwb-core-exe -p a2dpwb-driver
cd app/src/ui
pnpm install --frozen-lockfile
pnpm build
cd ../../..
cargo build --release -p a2dpwb-ui
```

## Wenn Sie bereits ohne `--recursive` geklont haben

```bash
git submodule update --init --recursive
```

## Projektstruktur

```
A2DP-Windows-Bridge/
├── app/
│   ├── src/
│   │   ├── core/           a2dpwb-core.exe: residenter Kern, Infobereich, IPC-Server, --cli
│   │   ├── ui/             a2dpwb.exe: Tauri 2 + SolidJS (pnpm dev führt es mit einem Mock-Kern im Browser aus)
│   │   ├── driver/         a2dpwb-driver.exe: WinUSB zuweisen / wiederherstellen (mit Administratorrechten)
│   │   └── crates/         Bibliotheken: btstack-sys, hci-transport, bt-host, codec, die -sys-Crates der Codecs,
│   │                       pipeline, audio-capture, win-sys, core (a2dpwb-core), ipc, le-audio, lc3-sys
│   ├── lang/               Lokalisierung (en.json ist die Vorlage; tools/check_lang.py prüft die übrigen)
│   └── resources/          Symbol und Manifest
├── Cargo.toml              Cargo-Workspace
├── installer/              Release-Skript, NSIS-Hooks, Hinweise zum Updater (installer/README.md)
├── compat/                 MSVC-Kompatibilitätsheader für den AOSP-Code
├── docs/                   Dokumentation (diese Website) und das Designdokument (v2-design.md)
├── extern/                 C-Bibliotheken von Drittanbietern (git submodules)
│   ├── btstack/            Bluetooth-Stack BTstack
│   ├── libldac/            LDAC-Encoder aus AOSP
│   ├── libopenaptx/        Encoder für aptX / aptX HD / aptX LL
│   ├── fdk-aac/            AAC-Encoder von Fraunhofer
│   ├── lhdcv5-enc/         LHDC-V5-Encoder
│   └── opus/               libopus (Opus-Encoder)
└── tools/
    ├── a2dp_decode/        a2dpwb_decode: prüft / dekodiert den Medienstream in einer HCI-Aufzeichnung (.pklg)
    ├── emu/                End-to-End-Test gegen eine virtuelle Bluetooth-Senke (Python)
    └── linux_sink/         a2dpwb_sink: messender A2DP-Empfänger unter Linux (Python, Bumble)
```

## Tests

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd app/src/ui && pnpm exec tsc --noEmit && pnpm exec vitest run
python tools/check_lang.py
```

## Release-Pakete

```powershell
pwsh installer/build-release.ps1 -Arch x64
```

schreibt das Installationsprogramm (`A2DPWB-<version>-win64-setup.exe`), das portable ZIP und das Updater-Manifest nach `target/dist`. Die Signierung der In-App-Updates und die übrigen Details stehen in `installer/README.md`.

## Testen ohne Hardware (tools/emu) {#emu}

`tools/emu` streamt jeden Codec von A2DPWB an eine virtuelle Bluetooth-Senke auf demselben PC, ganz ohne Adapter oder Kopfhörer. Es läuft nur unter Windows und benötigt **Python 3.11 oder neuer** sowie **CMake** (in Visual Studio enthalten) für das Decoder-Werkzeug.

```bash
python tools/emu/setup_env.py                       # einmalig: legt tools/emu/.venv mit festen Paketversionen an
cmake -S tools/a2dp_decode -B build/a2dp_decode     # einmalig: das Decoder-Werkzeug
cmake --build build/a2dp_decode --config Release
cargo build --release -p a2dpwb-core-exe
python tools/emu/run_test.py                        # etwa 35 s für alle Codecs
```

Funktionsweise:

- `emu_sink.py` betreibt zwei virtuelle [Bumble](https://github.com/google/bumble)-Controller auf einer prozessinternen Verbindung. Einer wird für A2DPWB als H4 über TCP (`127.0.0.1`) bereitgestellt. Den anderen nutzt eine Bumble-A2DP-Senke, die SBC, AAC, aptX, aptX HD, aptX LL und LDAC anbietet und ihren HCI-Verkehr aufzeichnet.
- `run_test.py` führt `a2dpwb-core.exe --cli` für jeden Codec mit Entwicklungsoptionen aus: `--hci-tcp` (virtueller Controller statt WinUSB), `--test-tone` (1 kHz links / 1,5 kHz rechts statt Systemaudio), `--duration`, `--hci-capture`. Die Umgebungsvariable `A2DPWB_CONFIG_DIR` lässt A2DPWB einen separaten Konfigurationsordner verwenden, sodass die Kopplung mit der virtuellen Senke Ihre echten Link Keys nie berührt.
- Beide HCI-Aufzeichnungen werden anschließend mit `a2dpwb_decode` geprüft (die Senkenseite mit `--received`).

Ein Codec besteht den Test, wenn all dies zutrifft:

- der angeforderte Codec wurde ausgehandelt
- `a2dpwb_decode` findet weder auf der Sende- noch auf der Empfangsseite ein Problem
- die Senke hat jedes von A2DPWB gesendete Medienpaket und jeden Frame empfangen, und mindestens 90 % der angeforderten Audiodauer wurden gesendet
- außer bei LDAC (kein Open-Source-Decoder): das dekodierte Audio ist auf beiden Seiten identisch und entspricht dem Testton

| Option | Beschreibung |
|:-------|:-------------|
| `--codecs sbc aac ...` | Nur diese Codecs testen (`sbc aac aptx aptxhd aptxll ldac`, Standard: alle) |
| `--duration <s>` | Sekunden Streaming pro Codec (Standard 5) |
| `--max-packet <bytes>` | Wird an A2DPWB übergeben (max. Mediapaketgröße, siehe [Verwendung](usage)) |
| `--a2dpwb <exe>` | Der zu testende Kern (Standard `target/release/a2dpwb-core.exe`) |
| `--decoder <exe>` | Das Decoder-Werkzeug (Standard `build/a2dp_decode/Release/a2dpwb_decode.exe`) |
| `--out <dir>` | Ausgabeordner (Standard `tools/emu/out`) |

Für jeden Codec enthält `tools/emu/out/<codec>/` die Aufzeichnungen (`a2dpwb.pklg`, `sink.pklg`), Protokolle, die Berichte von `a2dpwb_decode` und die dekodierten WAV-Dateien. Der Exitcode ist 0, wenn jeder Codec bestanden hat.

{: .note }
Die virtuelle Verbindung hat keinen Funk: Sie prüft Kodierung, Paketierung, AVDTP- / L2CAP-Signalisierung und die Interoperabilität mit einem unabhängigen Bluetooth-Stack (Bumble), aber nicht die Funkbedingungen, das Timing auf echter Hardware oder wie echte Kopfhörer das Audio wiedergeben. Die Bumble-Pakete dienen nur als Testwerkzeug und sind nicht Teil von A2DPWB (siehe THIRD_PARTY_LICENSES.md).

`emu_vhci.py` tut dasselbe gegenüber dem Linux-Bluetooth-Stack: Unter Linux bindet es über `/dev/vhci` einen virtuellen Adapter an BlueZ an, den der Messempfänger `tools/linux_sink` dann wie einen echten Adapter übernimmt - so lässt er sich ohne Hardware testen, optional mit verlorenen oder zurückgehaltenen Medienpaketen (`--drop`, `--stall`). Siehe `tools/linux_sink/README.md`.

## Abhängigkeiten

Die C-Bibliotheken sind git submodules; Rust-Crates und die npm-Pakete der Oberfläche werden von Cargo und pnpm abgerufen (Versionen festgelegt in `Cargo.lock` und `app/src/ui/pnpm-lock.yaml`; `cargo deny check` prüft ihre Lizenzen).

| Bibliothek | Methode | Feste Version | Lizenz |
|:-----------|:--------|:--------------|:-------|
| BTstack | git submodule | v1.8.1-6-g5bc5cbdbe | BTstack License (BSD-3-Clause-artig mit einer Klausel gegen kommerzielle Nutzung) |
| Bluedroid SBC codec | Encoder nach Rust portiert; der Decoder (Tests, Werkzeuge) aus der Kopie in BTstack | wie in BTstack | Apache-2.0 |
| rijndael | in BTstack enthalten (`3rd-party/rijndael`) | wie in BTstack | Gemeinfrei (Public Domain) |
| liblc3 (lc3-google) | in BTstack enthalten (`3rd-party/lc3-google`) | wie in BTstack | Apache-2.0 |
| micro-ecc | in BTstack enthalten (`3rd-party/micro-ecc`) | wie in BTstack | BSD-2-Clause |
| libldac (AOSP) | git submodule | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 |
| libopenaptx | git submodule | 0.2.0 (nicht aktualisieren) | LGPL-2.1+ |
| fdk-aac | git submodule | v2.0.3-158-gd8e6b1a | FDK AAC License |
| LHDC-V5-Encoder | git submodule | wie festgelegt | Apache-2.0 |
| libopus | git submodule | v1.5.2 | BSD-3-Clause |
| Tauri, SolidJS, Rust-Crates | Cargo / pnpm | `Cargo.lock`, `pnpm-lock.yaml` | MIT / Apache-2.0 und weitere (`licenses/rust-crates.txt` im Release) |

{: .warning }
Da BTstack eingebunden ist, darf jedes von Ihnen gebaute A2DPWB nur für private, nicht kommerzielle Zwecke verwendet und weitergegeben werden, auch wenn der Quellcode von A2DPWB selbst unter Apache-2.0 oder MIT doppelt lizenziert ist.

Ausführliche Lizenzinformationen finden Sie in [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md).
