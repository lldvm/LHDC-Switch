---
title: ライセンス
layout: default
parent: ホーム
nav_order: 5
---

# ライセンス
{: .no_toc }

## 目次
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## プロジェクトライセンス

A2DP Windows Bridge (A2DPWB) は **Apache License 2.0** と **MIT License** のデュアルライセンスです（SPDX `Apache-2.0 OR MIT`）。どちらか一方を選んで利用できます。[LICENSE](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE)、[LICENSE-APACHE](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE-APACHE)、[LICENSE-MIT](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE-MIT) を参照してください。

{: .warning }
A2DPWB のソースコードは Apache-2.0 / MIT のデュアルライセンスです。ただし、配布している `a2dpwb-core.exe` には BTstack が含まれており、A2DPWB はプログラム一式としてだけ配布しているため、リリースは個人的かつ非商用の目的でのみ使用・再配布できます。

## サードパーティライブラリ

| コンポーネント | 固定バージョン | ライセンス | 用途 |
|:---------------|:---------------|:-----------|:-----|
| [BTstack](https://github.com/bluekitchen/btstack) | v1.8.1-6-g5bc5cbdbe | BTstack License（非商用条項付きの BSD-3-Clause 類似ライセンス）/ 商用 | ユーザーモード Bluetooth スタック |
| rijndael（BTstack に同梱） | BTstack に準拠 | パブリックドメイン | AES（BTstack が使用） |
| tinydir（BTstack に同梱） | BTstack に準拠 | BSD-2-Clause | BTstack の Broadcom チップセットドライバーでのフォルダー一覧 |
| liblc3（lc3-google、BTstack に同梱） | BTstack に準拠 | Apache-2.0 | LC3 エンコーダー（LE Audio、実験的） |
| micro-ecc（BTstack に同梱） | BTstack に準拠 | BSD-2-Clause | LE Secure Connections のペアリングの P-256（LE Audio、実験的） |
| Bluedroid SBC エンコーダー（Rust に移植） | BTstack 同梱版から移植 | Apache-2.0 | SBC エンコーダー |
| [libldac (AOSP)](https://android.googlesource.com/platform/external/libldac) | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 | LDAC エンコーダー |
| [libopenaptx](https://github.com/pali/libopenaptx) | 0.2.0 | LGPL-2.1+ | aptX / aptX HD / aptX LL エンコーダー |
| [fdk-aac](https://github.com/mstorsjo/fdk-aac) | v2.0.3-158-gd8e6b1a | FDK AAC License | AAC-LC エンコーダー |
| [LHDC-V5-Encoder](https://github.com/WillyBilly06/LHDC-V5-Encoder)（AOSP エンコーダーの C 移植版） | commit 3f9d198 | Apache-2.0（FFT 部分は BSD-3-Clause、KISS FFT） | LHDC V5 エンコーダー |
| [libopus](https://opus-codec.org/) | 1.5.2 | BSD-3-Clause | Opus エンコーダー |
| Rust のクレート: [Tauri](https://tauri.app/) 2.12、tokio、serde、windows、nusb、wasapi、tray-icon など | `Cargo.lock` | ほとんどが Apache-2.0 / MIT。ほかに BSD、ISC、Zlib、Unicode-3.0、CDLA-Permissive-2.0、MPL-2.0 | ウィンドウ、トレイ、IPC、USB、音声、更新 |
| [SolidJS](https://www.solidjs.com/) | 1.9.17 | MIT | ウィンドウの UI ライブラリ |
| [`@tauri-apps/api`](https://github.com/tauri-apps/tauri) | 2.12.4 | Apache-2.0 OR MIT | Tauri の JavaScript 側 |
| [Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons) | UI にコピーしたもの | MIT | アイコン |

BTstack と各コーデックのライブラリは `a2dpwb-core.exe`、ウィンドウ（Tauri、SolidJS、アイコン）は `a2dpwb.exe` に入っています。`a2dpwb-driver.exe` は Rust のコードだけです。ウィンドウの表示には Microsoft Edge WebView2 ランタイムを使いますが、A2DPWB には含めていません（ない場合はインストーラーが Microsoft のブートストラッパーを実行します）。

ライセンス全文とコンプライアンスの詳細は [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md) を参照してください。インストーラーとポータブル zip は、プログラムの隣の `licenses/` フォルダーに各ライセンス文（Rust のクレートは cargo-about で生成した `licenses/rust-crates.txt`）を置き、`THIRD_PARTY_LICENSES.md` と、正確なソースのコミットを記した `SOURCE.txt` も同梱します。Realtek ファームウェアと Zadig は A2DPWB と一緒には配布していません（リンクのみ）。

## 重要な注意事項

### BTstack ライセンス（非商用）

BTstack のライセンスは BSD-3-Clause **ではなく**、OSI 承認のオープンソースライセンスでもありません。このライセンス（SPDX `LicenseRef-BTstack`）は BSD-3-Clause の 3 条件に、次の第 4 条を加えたものです（原文）。

> 4. Any redistribution, use, or modification is done solely for personal benefit and not for any commercial purpose or for monetary gain.

（参考訳: 再配布・使用・改変は、専ら個人的な利益のためにのみ行われ、商業目的や金銭的利益のために行われないこと。）

`a2dpwb-core.exe` には BTstack が含まれるため、この制限は配布するプログラムに及びます。商用利用には [BlueKitchen GmbH](https://bluekitchen-gmbh.com/)（contact@bluekitchen-gmbh.com）から商用ライセンスを取得する必要があります。

### LDAC 認証

libldac の NOTICE には "Taking the certification process is required to use LDAC in your products."（LDAC を製品で使用するには認証プロセスを受ける必要がある）と記載されています。[sony.net/Products/LDAC/aosp](https://www.sony.net/Products/LDAC/aosp/) を参照してください。A2DPWB のような無償・非商用のアプリにこの認証が適用されるかどうかは確認できていません。A2DPWB は認証を受けていません。

### LHDC の商標と認証

LHDC は Savitech Corp. のコーデックで、HWA（Hi-Res Wireless Audio）アライアンスが推進しています。A2DPWB が使用するエンコーダーは Google が AOSP（Android 17）で公開したものに由来する Apache-2.0 のコードですが、Apache-2.0 は商標の使用権を与えるものではなく、LHDC 対応を謳う製品は通常 Savitech / HWA の認証を受けます。A2DPWB は認証を受けていません。LHDC V5 対応は非商用の相互運用機能です。

### libopenaptx のバージョン

libopenaptx は LGPL-2.1+ の最終版である **0.2.0** に固定しています。0.2.1 以降は GPL-3.0-or-later（追加の制限あり）となり、A2DPWB がリンクしている他のライブラリと両立しません。v1.0.4 までのリリースは誤って 0.2.1 でビルドされていたため、v1.0.5 以降を使用してください。

libopenaptx は `a2dpwb-core.exe` に静的リンクしています。インストーラーと zip には LGPL-2.1 の全文と、libopenaptx の著作権表示を載せた THIRD_PARTY_LICENSES.md を同梱し、**設定 > A2DPWB について** からサードパーティライセンスを開けます。改変した libopenaptx で再ビルド（再リンク）するための完全な対応ソースコードは、リリースのタグが付いたコミット（サブモジュールを含む）としてリポジトリから入手できます。

### aptX 特許に関する注意

aptX（クラシック）、aptX HD、aptX Low Latency、aptX Adaptive は Qualcomm の商標です。libopenaptx は README によれば「ffmpeg 4.0 プロジェクトから派生した」（derived from ffmpeg 4.0 project）オープンソースの aptX 実装で、本ツールでは aptX、aptX HD、aptX Low Latency に使用しています。aptX エンコーディングは一部の法域で特許の対象となる場合があります。A2DPWB は aptX Adaptive エンコーダーを含みません。

### fdk-aac のライセンスと AAC 特許に関する注意

fdk-aac のライセンスでは、バイナリを再配布する際にライセンス全文を添付すること、およびバイナリの受領者に FDK AAC Codec（と改変部分）の完全なソースコードを無償で提供することが求められます。また、このライセンスは**特許ライセンスを一切許諾しません**。AAC は特許の対象となる場合があり（NOTICE では Via Licensing に言及）、必要な AAC 特許ライセンスの取得は利用者の責任となります。

### Rust のクレートと MPL-2.0

Rust のクレートのライセンスは、リリースのビルド時に確認します。認めたライセンスの一覧（`about.toml`。`deny.toml` と同じ）にないものがあれば、`cargo-about` が失敗します。MPL-2.0 のクレートがいくつか（cssparser、selectors、dtoa-short、option-ext）ありますが、改変せずに使っており、ソースは `Cargo.lock` のバージョンで crates.io から入手できます。詳しくは THIRD_PARTY_LICENSES.md の §9 を参照してください。

### 開発者向けツール a2dpwb_decode

`a2dpwb_decode`（[使い方](usage#verify-stream)を参照）はソースからのみビルドするツールで、リリースには含まれません。上記の固定バージョンの Bluedroid SBC デコーダー（Apache-2.0）、fdk-aac（FDK AAC License）、libopenaptx 0.2.0（LGPL-2.1+）、libopus（BSD-3-Clause）、liblc3（Apache-2.0、BTstack に同梱）をリンクし、BTstack と libldac は**リンクしません**。詳しくは THIRD_PARTY_LICENSES.md の §14 を参照してください。

### テスト環境 tools/emu と tools/linux_sink

`tools/emu`（[ビルド](building)を参照）と測定用受信機 `tools/linux_sink`（[使い方](usage#receiver)を参照）は、[Bumble](https://github.com/google/bumble) 0.0.234（Apache-2.0）とその依存パッケージを使います。これらは固定バージョンで git の管理外のローカル環境にダウンロードするもので、A2DPWB にリンクも同梱もされません。パッケージ一覧とライセンスは THIRD_PARTY_LICENSES.md の §15 を参照してください。

### Bluetooth 認証

商用 Bluetooth 製品は Bluetooth SIG が管理する [Bluetooth 認証プロセス](https://www.bluetooth.com/)を受ける必要があります。
