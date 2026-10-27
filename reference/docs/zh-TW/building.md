---
title: 建置
layout: default
parent: 首頁
nav_order: 3
---

# 從原始碼建置
{: .no_toc }

## 目錄
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 需求

- 透過 [rustup](https://rustup.rs/) 安裝的 **Rust**（stable，1.90 以上）
- **Visual Studio 2022 以上**（或其 Build Tools），並安裝「使用 C++ 的桌面開發」工作負載：C 程式庫（BTstack、各轉碼器）以 MSVC 編譯
- 建置 UI 所需的 **Node.js** 22 與 **pnpm**（`corepack enable` 即可提供 pnpm）
- **Git**（用於子模組）
- 供 `build.ps1` / `build.bat` 使用的 **PowerShell 7**（`pwsh`，`winget install Microsoft.PowerShell`）
- 建置 ARM64 版本時：Visual Studio 元件「MSVC v143 - VS 2022 C++ ARM64 build tools」，在 x64 電腦上還需要「適用於 Windows 的 C++ Clang 編譯器」（rustls 的 `ring` 以 clang 組譯其 ARM64 程式碼；`installer/build-release.ps1` 會自行找到 Visual Studio 的 clang），以及 `rustup target add aarch64-pc-windows-msvc`

## 快速建置

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge.git
cd A2DP-Windows-Bridge
pwsh ./build.ps1
```

也可以在檔案總管中連按兩下 `build.bat`：它會執行 `build.ps1`，結束時等待按鍵（`build.bat -Test` 會把參數傳給指令碼）。`build.ps1` 會先檢查 Rust、Node.js、pnpm 與子模組是否齊全，`node_modules` 不存在時執行 `pnpm install`，先建置視窗的頁面，再把三個程式輸出到 `target/release/`：`a2dpwb-core.exe`（常駐核心：藍牙、音訊、系統匣）、`a2dpwb.exe`（視窗）與 `a2dpwb-driver.exe`（以提高權限指派 / 還原 WinUSB 驅動程式的輔助程式）。請將這三個程式放在同一個資料夾中；`a2dpwb-core.exe --cli` 可在沒有視窗的情況下串流。

參數：`-Debug`（開發設定檔，`target/debug/`）、`-Test`（接著執行「測試」中的檢查）、`-Decoder`（同時建置解碼工具）、`-Emu`（同時執行[不使用硬體進行測試](#emu)）、`-Release -Arch x64|arm64`（僅執行發行指令碼，見「發行套件」）。

**分步建置**（即 `build.ps1` 所做的事；`a2dpwb.exe` 會嵌入視窗的頁面，因此必須先建置頁面）：

```bash
cargo build --release -p a2dpwb-core-exe -p a2dpwb-driver
cd app/src/ui
pnpm install --frozen-lockfile
pnpm build
cd ../../..
cargo build --release -p a2dpwb-ui
```

## 若複製時未使用 `--recursive`

```bash
git submodule update --init --recursive
```

## 專案結構

```
A2DP-Windows-Bridge/
├── app/
│   ├── src/
│   │   ├── core/           a2dpwb-core.exe：常駐核心、系統匣、IPC 伺服器、--cli
│   │   ├── ui/             a2dpwb.exe：Tauri 2 + SolidJS（pnpm dev 會搭配模擬核心在瀏覽器中執行）
│   │   ├── driver/         a2dpwb-driver.exe：指派 / 還原 WinUSB（提高權限）
│   │   └── crates/         程式庫：btstack-sys、hci-transport、bt-host、codec、各轉碼器的 -sys crate、
│   │                       pipeline、audio-capture、win-sys、core (a2dpwb-core)、ipc、le-audio、lc3-sys
│   ├── lang/               多語系（en.json 為原文；tools/check_lang.py 會檢查其他語言）
│   └── resources/          圖示與資訊清單
├── Cargo.toml              Cargo 工作區
├── installer/              發行指令碼、NSIS 掛勾、更新程式說明（installer/README.md）
├── compat/                 AOSP 程式碼用的 MSVC 相容性標頭
├── docs/                   說明文件（本網站）與設計文件（v2-design.md）
├── extern/                 第三方 C 程式庫（git 子模組）
│   ├── btstack/            BTstack 藍牙堆疊
│   ├── libldac/            AOSP LDAC 編碼器
│   ├── libopenaptx/        aptX / aptX HD / aptX LL 編碼器
│   ├── fdk-aac/            Fraunhofer AAC 編碼器
│   ├── lhdcv5-enc/         LHDC V5 編碼器
│   └── opus/               libopus（Opus 編碼器）
└── tools/
    ├── a2dp_decode/        a2dpwb_decode：檢查 / 解碼 HCI 擷取檔（.pklg）中的媒體串流
    ├── emu/                針對虛擬藍牙接收端的端對端測試（Python）
    └── linux_sink/         a2dpwb_sink：Linux 上的測量用 A2DP 接收器（Python、Bumble）
```

## 測試

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd app/src/ui && pnpm exec tsc --noEmit && pnpm exec vitest run
python tools/check_lang.py
```

## 發行套件

```powershell
pwsh installer/build-release.ps1 -Arch x64
```

會將安裝程式（`A2DPWB-<version>-win64-setup.exe`）、免安裝版 zip 與更新程式資訊清單輸出到 `target/dist`。應用程式內更新的簽署方式及其他細節，請參閱 `installer/README.md`。

## 不使用硬體進行測試 (tools/emu) {#emu}

`tools/emu` 會在同一台 PC 上，將 A2DPWB 的每種轉碼器串流到虛擬藍牙接收端，不需要介面卡或耳機。它只能在 Windows 上執行，需要 **Python 3.11 以上**，以及用於建置解碼工具的 **CMake**（隨附於 Visual Studio）。

```bash
python tools/emu/setup_env.py                       # once: creates tools/emu/.venv with pinned packages
cmake -S tools/a2dp_decode -B build/a2dp_decode     # once: the decoder tool
cmake --build build/a2dp_decode --config Release
cargo build --release -p a2dpwb-core-exe
python tools/emu/run_test.py                        # about 35 s for all codecs
```

運作方式：

- `emu_sink.py` 在同一處理程序內的連結上執行兩個 [Bumble](https://github.com/google/bumble) 虛擬控制器。其中一個以 H4 over TCP（`127.0.0.1`）提供給 A2DPWB 使用。另一個由 Bumble A2DP 接收端使用，提供 SBC、AAC、aptX、aptX HD、aptX LL 與 LDAC，並記錄其 HCI 流量。
- `run_test.py` 會以開發用選項，針對每種轉碼器執行 `a2dpwb-core.exe --cli`：`--hci-tcp`（以虛擬控制器取代 WinUSB）、`--test-tone`（以左 1 kHz / 右 1.5 kHz 取代系統音訊）、`--duration`、`--hci-capture`。環境變數 `A2DPWB_CONFIG_DIR` 會讓 A2DPWB 使用另一個設定資料夾，因此與虛擬接收端配對時絕不會動到您實際的連結金鑰。
- 接著以 `a2dpwb_decode` 檢查兩份 HCI 擷取檔（接收端使用 `--received`）。

符合以下所有條件時，該轉碼器即通過：

- 協商的是所要求的轉碼器
- `a2dpwb_decode` 在傳送端與接收端都沒有發現問題
- 接收端收到 A2DPWB 傳送的每個媒體封包與訊框，且至少傳送了所要求音訊長度的 90%
- LDAC 除外（沒有開放原始碼的解碼器）：兩端解碼後的音訊完全相同，且為測試音

| 選項 | 說明 |
|:-------|:------------|
| `--codecs sbc aac ...` | 只測試這些轉碼器（`sbc aac aptx aptxhd aptxll ldac`，預設：全部） |
| `--duration <s>` | 每種轉碼器的串流秒數（預設 5） |
| `--max-packet <bytes>` | 傳給 A2DPWB（媒體封包大小上限，請參閱 [使用方式](usage)） |
| `--a2dpwb <exe>` | 要測試的核心（預設 `target/release/a2dpwb-core.exe`） |
| `--decoder <exe>` | 解碼工具（預設 `build/a2dp_decode/Release/a2dpwb_decode.exe`） |
| `--out <dir>` | 輸出資料夾（預設 `tools/emu/out`） |

每種轉碼器的 `tools/emu/out/<codec>/` 中包含擷取檔（`a2dpwb.pklg`、`sink.pklg`）、記錄、`a2dpwb_decode` 報告與解碼後的 WAV 檔案。所有轉碼器都通過時，結束代碼為 0。

{: .note }
虛擬連結沒有無線電：它會檢查編碼、封包化、AVDTP / L2CAP 訊號，以及與獨立藍牙堆疊 (Bumble) 的互通性，但不會檢查無線電環境、實際硬體上的時序，或實際耳機播放音訊的情形。Bumble 套件僅為測試工具，不屬於 A2DPWB 的一部分（請參閱 THIRD_PARTY_LICENSES.md）。

`emu_vhci.py` 則針對 Linux 藍牙堆疊進行相同的工作：在 Linux 上，它會透過 `/dev/vhci` 將虛擬介面卡連接到 BlueZ，測量用接收器 `tools/linux_sink` 會像使用實體介面卡一樣使用它 - 因此不需要硬體即可測試，也可選擇模擬遺失或延遲的媒體封包（`--drop`、`--stall`）。請參閱 `tools/linux_sink/README.md`。

## 相依套件

C 程式庫為 git 子模組；Rust crate 與 UI 的 npm 套件由 Cargo 與 pnpm 取得（版本固定於 `Cargo.lock` 與 `app/src/ui/pnpm-lock.yaml`；`cargo deny check` 會檢查其授權）。

| 程式庫 | 取得方式 | 固定版本 | 授權 |
|:--------|:-------|:---------------|:--------|
| BTstack | git 子模組 | v1.8.1-6-g5bc5cbdbe | BTstack License（類似 BSD-3-Clause，附非商業條款） |
| Bluedroid SBC codec | 編碼器移植為 Rust；解碼器（測試、工具用）取自 BTstack 隨附的副本 | 同 BTstack | Apache-2.0 |
| rijndael | 隨附於 BTstack（`3rd-party/rijndael`） | 同 BTstack | 公眾領域 |
| liblc3 (lc3-google) | 隨附於 BTstack（`3rd-party/lc3-google`） | 同 BTstack | Apache-2.0 |
| micro-ecc | 隨附於 BTstack（`3rd-party/micro-ecc`） | 同 BTstack | BSD-2-Clause |
| libldac (AOSP) | git 子模組 | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 |
| libopenaptx | git 子模組 | 0.2.0（請勿更新） | LGPL-2.1+ |
| fdk-aac | git 子模組 | v2.0.3-158-gd8e6b1a | FDK AAC License |
| LHDC V5 encoder | git 子模組 | 依固定的版本 | Apache-2.0 |
| libopus | git 子模組 | v1.5.2 | BSD-3-Clause |
| Tauri、SolidJS、Rust crate | Cargo / pnpm | `Cargo.lock`、`pnpm-lock.yaml` | MIT / Apache-2.0 等（發行版中的 `licenses/rust-crates.txt`） |

{: .warning }
由於連結了 BTstack，即使 A2DPWB 原始碼本身採用 Apache-2.0 或 MIT 雙重授權，您自行建置的任何 A2DPWB 都只能用於個人、非商業目的的使用與再散布。

完整授權資訊請參閱 [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md)。
