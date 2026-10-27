---
title: ビルド
layout: default
parent: ホーム
nav_order: 3
---

# ソースからビルド
{: .no_toc }

## 目次
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 必要なもの

- **Rust**（stable、1.90 以降）。[rustup](https://rustup.rs/) で入れます
- **Visual Studio 2022 以降**（Build Tools でも可）と「C++ によるデスクトップ開発」ワークロード。C のライブラリ（BTstack、コーデック）を MSVC でコンパイルします
- UI 用に **Node.js** 22 と **pnpm**（`corepack enable` で pnpm が使えます）
- **Git**（サブモジュールの取得）
- `build.ps1` / `build.bat` 用に **PowerShell 7**（`pwsh`。`winget install Microsoft.PowerShell`）
- ARM64 版をビルドするとき: Visual Studio のコンポーネント「MSVC v143 - VS 2022 C++ ARM64 ビルド ツール」と、x64 の PC では「Windows 用 C++ Clang コンパイラ」（rustls の `ring` が ARM64 のアセンブリを clang で組むため。`installer/build-release.ps1` は Visual Studio の clang を自分で探す）、それに `rustup target add aarch64-pc-windows-msvc`

## クイックビルド

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge.git
cd A2DP-Windows-Bridge
pwsh ./build.ps1
```

エクスプローラーで `build.bat` をダブルクリックしても同じです（`build.ps1` を実行し、最後にキー入力を待ちます。`build.bat -Test` のようにスイッチも渡せます）。`build.ps1` は最初に Rust、Node.js、pnpm、サブモジュールがそろっているかを確かめ、`node_modules` がなければ `pnpm install` を実行し、ウィンドウのページを組んでから 3 つのプログラムを `target/release/` に作ります。`a2dpwb-core.exe`（常駐するコア。Bluetooth、音声、タスクトレイ）、`a2dpwb.exe`（ウィンドウ）、`a2dpwb-driver.exe`（WinUSB ドライバの割り当てと解除をする、管理者権限のヘルパー）です。3 つは同じフォルダに置いてください。`a2dpwb-core.exe --cli` ならウィンドウなしで配信できます。

スイッチ: `-Debug`（開発プロファイル、`target/debug/`）、`-Test`（続けて「テスト」のチェック）、`-Decoder`（デコーダツールも）、`-Emu`（[実機なしでのテスト](#emu)も）、`-Release -Arch x64|arm64`（リリーススクリプトのみ。「リリースパッケージ」参照）。

**手順を分けて行う場合**（`build.ps1` がしていること。`a2dpwb.exe` はウィンドウのページを埋め込むので、ページを先に組みます）:

```bash
cargo build --release -p a2dpwb-core-exe -p a2dpwb-driver
cd app/src/ui
pnpm install --frozen-lockfile
pnpm build
cd ../../..
cargo build --release -p a2dpwb-ui
```

## `--recursive` なしでクローン済みの場合

```bash
git submodule update --init --recursive
```

## プロジェクト構成

```
A2DP-Windows-Bridge/
├── app/
│   ├── src/
│   │   ├── core/           a2dpwb-core.exe: 常駐コア、タスクトレイ、IPC サーバー、--cli
│   │   ├── ui/             a2dpwb.exe: Tauri 2 + SolidJS（pnpm dev でモックのコアとブラウザで動く）
│   │   ├── driver/         a2dpwb-driver.exe: WinUSB の割り当て / 解除（管理者権限）
│   │   └── crates/         ライブラリ: btstack-sys、hci-transport、bt-host、codec、コーデックの -sys クレート、
│   │                       pipeline、audio-capture、win-sys、core（a2dpwb-core）、ipc、le-audio、lc3-sys
│   ├── lang/               翻訳（en.json が原本。他の言語は tools/check_lang.py で確認）
│   └── resources/          アイコンとマニフェスト
├── Cargo.toml              Cargo workspace
├── installer/              リリース用スクリプト、NSIS のフック、更新の手順（installer/README.md）
├── compat/                 AOSP コード用の MSVC 互換ヘッダー
├── docs/                   ドキュメント（このサイト）と設計書（v2-design.md）
├── extern/                 サードパーティの C ライブラリ（git サブモジュール）
│   ├── btstack/            BTstack Bluetooth スタック
│   ├── libldac/            AOSP LDAC エンコーダ
│   ├── libopenaptx/        aptX / aptX HD / aptX LL エンコーダ
│   ├── fdk-aac/            Fraunhofer AAC エンコーダ
│   ├── lhdcv5-enc/         LHDC V5 エンコーダ
│   └── opus/               libopus（Opus エンコーダ）
└── tools/
    ├── a2dp_decode/        a2dpwb_decode: HCI キャプチャ（.pklg）内のメディアストリームを検査 / デコード
    ├── emu/                仮想 Bluetooth シンクを相手にしたエンドツーエンドテスト（Python）
    └── linux_sink/         a2dpwb_sink: Linux で動く計測用 A2DP 受信機（Python、Bumble）
```

## テスト

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd app/src/ui && pnpm exec tsc --noEmit && pnpm exec vitest run
python tools/check_lang.py
```

## リリースパッケージ

```powershell
pwsh installer/build-release.ps1 -Arch x64
```

`target/dist` にインストーラ（`A2DPWB-<バージョン>-win64-setup.exe`）、ポータブル版の zip、更新用のマニフェストを書き出します。アプリ内更新の署名などは `installer/README.md` を見てください。

## 実機なしでのテスト（tools/emu） {#emu}

`tools/emu` は、同じ PC 上の仮想 Bluetooth シンクへ全コーデックを配信して確かめます。アダプタもヘッドホンも要りません。Windows 専用で、**Python 3.11 以降**と、デコーダツール用の **CMake**（Visual Studio に同梱）が必要です。

```bash
python tools/emu/setup_env.py                       # 初回のみ: tools/emu/.venv を作る（パッケージは固定版）
cmake -S tools/a2dp_decode -B build/a2dp_decode     # 初回のみ: デコーダツール
cmake --build build/a2dp_decode --config Release
cargo build --release -p a2dpwb-core-exe
python tools/emu/run_test.py                        # 全コーデックで約 35 秒
```

仕組み:

- `emu_sink.py` は、プロセス内のリンクでつないだ 2 つの [Bumble](https://github.com/google/bumble) 仮想コントローラを動かします。1 つは A2DPWB 向けに H4 over TCP（`127.0.0.1`）で公開し、もう 1 つは SBC、AAC、aptX、aptX HD、aptX LL、LDAC を受けられる Bumble の A2DP シンクが使い、その HCI 通信を記録します。
- `run_test.py` はコーデックごとに `a2dpwb-core.exe --cli` を開発用のオプション付きで動かします。`--hci-tcp`（WinUSB の代わりに仮想コントローラ）、`--test-tone`（システムの音の代わりに左 1 kHz / 右 1.5 kHz）、`--duration`、`--hci-capture`。環境変数 `A2DPWB_CONFIG_DIR` で別の設定フォルダを使うので、仮想シンクとのペアリングが本物のリンクキーに触れることはありません。
- 最後に両方の HCI キャプチャを `a2dpwb_decode` で調べます（シンク側は `--received`）。

次のすべてを満たしたコーデックが合格です。

- 指定したコーデックで接続した
- 送信側と受信側のどちらでも `a2dpwb_decode` が問題を見つけない
- A2DPWB が送ったメディアパケットとフレームをシンクがすべて受け取り、指定した長さの 90% 以上を送った
- LDAC 以外（オープンソースのデコーダがないため）: デコードした音が両側で一致し、テストトーンである

| オプション | 説明 |
|:-----------|:-----|
| `--codecs sbc aac ...` | このコーデックだけを試す（`sbc aac aptx aptxhd aptxll ldac`、既定は全部） |
| `--duration <秒>` | コーデックごとの配信時間（既定 5） |
| `--max-packet <バイト>` | A2DPWB に渡す（最大メディアパケットサイズ。[使い方](usage)を参照） |
| `--a2dpwb <exe>` | 試すコア（既定 `target/release/a2dpwb-core.exe`） |
| `--decoder <exe>` | デコーダツール（既定 `build/a2dp_decode/Release/a2dpwb_decode.exe`） |
| `--out <フォルダ>` | 出力先（既定 `tools/emu/out`） |

`tools/emu/out/<コーデック>/` には、キャプチャ（`a2dpwb.pklg`、`sink.pklg`）、ログ、`a2dpwb_decode` の結果、デコードした WAV が残ります。全コーデックが合格すると終了コードは 0 です。

{: .note }
仮想リンクには電波がありません。エンコード、パケット化、AVDTP / L2CAP のやり取り、独立した Bluetooth スタック（Bumble）との相互接続は確かめられますが、電波の状態、実機でのタイミング、実際のヘッドホンでの鳴り方は確かめられません。Bumble のパッケージはテスト用の道具で、A2DPWB には含まれません（THIRD_PARTY_LICENSES.md を参照）。

`emu_vhci.py` は同じことを Linux の Bluetooth スタックに向けて行います。Linux 上で `/dev/vhci` から BlueZ に仮想アダプタを付け、計測用受信機 `tools/linux_sink` がそれを本物のアダプタと同じように使うので、実機なしで試せます（メディアパケットの欠落や遅延も `--drop`、`--stall` で再現できます）。`tools/linux_sink/README.md` を見てください。

## 依存関係

C のライブラリは git サブモジュールです。Rust のクレートと UI の npm パッケージは Cargo と pnpm が取得します（バージョンは `Cargo.lock` と `app/src/ui/pnpm-lock.yaml` で固定。ライセンスは `cargo deny check` で確認）。

| ライブラリ | 取得方法 | 固定バージョン | ライセンス |
|:-----------|:---------|:---------------|:-----------|
| BTstack | git サブモジュール | v1.8.1-6-g5bc5cbdbe | BTstack License（非商用条項付きの BSD-3-Clause 類似ライセンス） |
| Bluedroid SBC コーデック | エンコーダは Rust に移植。デコーダ（テストとツール）は BTstack 同梱のもの | BTstack に準拠 | Apache-2.0 |
| rijndael | BTstack に同梱（`3rd-party/rijndael`） | BTstack に準拠 | パブリックドメイン |
| liblc3 (lc3-google) | BTstack に同梱（`3rd-party/lc3-google`） | BTstack に準拠 | Apache-2.0 |
| micro-ecc | BTstack に同梱（`3rd-party/micro-ecc`） | BTstack に準拠 | BSD-2-Clause |
| libldac (AOSP) | git サブモジュール | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 |
| libopenaptx | git サブモジュール | 0.2.0（更新しないこと） | LGPL-2.1+ |
| fdk-aac | git サブモジュール | v2.0.3-158-gd8e6b1a | FDK AAC License |
| LHDC V5 エンコーダ | git サブモジュール | 固定したコミット | Apache-2.0 |
| libopus | git サブモジュール | v1.5.2 | BSD-3-Clause |
| Tauri、SolidJS、Rust のクレート | Cargo / pnpm | `Cargo.lock`、`pnpm-lock.yaml` | MIT / Apache-2.0 など（リリースの `licenses/rust-crates.txt`） |

{: .warning }
BTstack をリンクしているため、A2DPWB のソースコード自体は Apache-2.0 / MIT のデュアルライセンスであっても、ビルドした A2DPWB は個人的かつ非商用の目的でのみ使用・再配布できます。

ライセンスの全文は [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md) を参照してください。
