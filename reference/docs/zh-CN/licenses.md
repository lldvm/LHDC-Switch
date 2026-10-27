---
title: 许可证
layout: default
parent: 首页
nav_order: 5
---

# 许可证
{: .no_toc }

## 目录
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 项目许可证

A2DP Windows Bridge (A2DPWB) 采用 **Apache License 2.0** 或 **MIT License** 双重许可，可任选其一（SPDX `Apache-2.0 OR MIT`）。请参阅 [LICENSE](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE)、[LICENSE-APACHE](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE-APACHE) 和 [LICENSE-MIT](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE-MIT)。

{: .warning }
A2DPWB 源代码采用 Apache-2.0 或 MIT 双重许可。分发的 `a2dpwb-core.exe` 包含 BTstack，而 A2DPWB 只以其全部程序的整体形式分发，因此发布版本只能用于个人非商业用途的使用和再分发。

## 第三方库

| 组件 | 固定版本 | 许可证 | 用途 |
|:----------|:---------------|:--------|:------|
| [BTstack](https://github.com/bluekitchen/btstack) | v1.8.1-6-g5bc5cbdbe | BTstack License（BSD-3-Clause 风格，附带非商业条款）/ 商业许可 | 用户模式 Bluetooth 堆栈 |
| rijndael（随 BTstack 附带） | 与 BTstack 相同 | 公有领域 | AES（BTstack 使用） |
| tinydir（随 BTstack 附带） | 与 BTstack 相同 | BSD-2-Clause | BTstack 的 Broadcom 芯片组驱动中的目录列举 |
| liblc3（lc3-google，随 BTstack 附带） | 与 BTstack 相同 | Apache-2.0 | LC3 编码器（LE Audio，实验性） |
| micro-ecc（随 BTstack 附带） | 与 BTstack 相同 | BSD-2-Clause | LE Secure Connections 配对所用的 P-256（LE Audio，实验性） |
| Bluedroid SBC 编码器（已移植为 Rust） | 移植自 BTstack 附带的副本 | Apache-2.0 | SBC 编码器 |
| [libldac (AOSP)](https://android.googlesource.com/platform/external/libldac) | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 | LDAC 编码器 |
| [libopenaptx](https://github.com/pali/libopenaptx) | 0.2.0 | LGPL-2.1+ | aptX / aptX HD / aptX LL 编码器 |
| [fdk-aac](https://github.com/mstorsjo/fdk-aac) | v2.0.3-158-gd8e6b1a | FDK AAC License | AAC-LC 编码器 |
| [LHDC-V5-Encoder](https://github.com/WillyBilly06/LHDC-V5-Encoder)（AOSP 编码器的 C 语言移植版） | commit 3f9d198 | Apache-2.0（FFT：BSD-3-Clause，KISS FFT） | LHDC V5 编码器 |
| [libopus](https://opus-codec.org/) | 1.5.2 | BSD-3-Clause | Opus 编码器 |
| Rust crate：[Tauri](https://tauri.app/) 2.12、tokio、serde、windows、nusb、wasapi、tray-icon 等 | `Cargo.lock` | 大多为 Apache-2.0 / MIT；另有 BSD、ISC、Zlib、Unicode-3.0、CDLA-Permissive-2.0 和 MPL-2.0 | 窗口、托盘、IPC、USB、音频、更新 |
| [SolidJS](https://www.solidjs.com/) | 1.9.17 | MIT | 窗口的 UI 库 |
| [`@tauri-apps/api`](https://github.com/tauri-apps/tauri) | 2.12.4 | Apache-2.0 OR MIT | Tauri 的 JavaScript 端 |
| [Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons) | 以复制到 UI 中的版本为准 | MIT | 图标 |

`a2dpwb-core.exe` 包含 BTstack 和各编解码器库，`a2dpwb.exe` 包含窗口（Tauri、SolidJS、图标），`a2dpwb-driver.exe` 只包含 Rust 代码。窗口由 Microsoft Edge WebView2 Runtime 渲染，A2DPWB 不附带该运行时（缺少时，安装程序会运行 Microsoft 的引导程序）。

完整的许可证文本和合规详情请参阅 [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md)。安装程序和便携版 zip 会在程序旁的 `licenses/` 文件夹中放入各许可证文本（Rust crate 的许可证位于由 cargo-about 生成的 `licenses/rust-crates.txt`），并附有 `THIRD_PARTY_LICENSES.md` 以及列出确切源代码提交的 `SOURCE.txt`。Realtek 固件和 Zadig 不随 A2DPWB 分发（仅提供链接）。

## 重要说明

### BTstack 许可证（非商业）

BTstack **不是** BSD-3-Clause，也不是 OSI 认可的开源许可证。其许可证（SPDX `LicenseRef-BTstack`）在 BSD-3-Clause 的三项条件之外，还附加了第四项条款：

> 4. Any redistribution, use, or modification is done solely for personal benefit and not for any commercial purpose or for monetary gain.

（大意：任何再分发、使用或修改都只能出于个人利益，不得用于任何商业目的或获取金钱收益。）

由于 `a2dpwb-core.exe` 包含 BTstack，此限制适用于分发的程序。商业用途需要从 [BlueKitchen GmbH](https://bluekitchen-gmbh.com/)（contact@bluekitchen-gmbh.com）获得商业许可。

### LDAC 认证

libldac 的 NOTICE 中写道：“Taking the certification process is required to use LDAC in your products.”（在产品中使用 LDAC 需要经过认证流程。）请参阅 [sony.net/Products/LDAC/aosp](https://www.sony.net/Products/LDAC/aosp/)。尚未确认 Sony 的认证流程是否适用于 A2DPWB 这样免费的非商业应用；A2DPWB 未经过认证。

### LHDC 商标与认证

LHDC 是 Savitech Corp. 的编解码器，由 Hi-Res Wireless Audio (HWA) 联盟推广。A2DPWB 使用的编码器是源自 Google 在 AOSP（Android 17）中发布的编码器的 Apache-2.0 代码，但 Apache-2.0 不授予任何商标权，而宣传支持 LHDC 的产品通常需要通过 Savitech / HWA 的认证。A2DPWB 未经过认证；LHDC V5 支持是一项非商业的互操作功能。

### libopenaptx 版本

libopenaptx 固定为 **0.2.0**，这是最后一个采用 LGPL-2.1+ 的版本。从 0.2.1 开始，它改为 GPL-3.0-or-later 并附加了额外限制，与 A2DPWB 链接的其他库不兼容。v1.0.4 及之前的发布版本误用了 0.2.1 进行构建；请使用 v1.0.5 或更高版本。

libopenaptx 静态链接到 `a2dpwb-core.exe` 中。安装程序和 zip 包附带 LGPL-2.1 文本以及包含 libopenaptx 版权信息的 THIRD_PARTY_LICENSES.md，**设置 > 关于 A2DPWB** 中有指向第三方许可证的链接，完整的对应源代码（用于使用修改过的 libopenaptx 重新构建/重新链接）可从仓库中该发布版本标签对应的提交及其子模块获取。

### aptX 专利声明

aptX（经典）、aptX HD、aptX Low Latency 和 aptX Adaptive 是 Qualcomm 的商标。根据其 README，libopenaptx 是一个“derived from ffmpeg 4.0 project”（源自 ffmpeg 4.0 项目）的开源 aptX 实现；本项目将其用于 aptX、aptX HD 和 aptX Low Latency。在某些司法管辖区，aptX 编码可能受专利保护。A2DPWB 不包含 aptX Adaptive 编码器。

### fdk-aac 许可证与 AAC 专利声明

fdk-aac 许可证要求在再分发二进制文件时附带完整的许可证文本，并向二进制文件的接收者免费提供 FDK AAC Codec 的完整源代码（包括任何修改）。它还**不授予任何专利许可**：AAC 可能受专利保护（NOTICE 中提到了 Via Licensing），获取任何所需的 AAC 专利许可是用户自己的责任。

### Rust crate 与 MPL-2.0

构建发布版本时会检查 Rust crate 的许可证：只要出现不在允许列表（`about.toml`，与 `deny.toml` 相同）中的许可证，`cargo-about` 就会失败。少数 crate 采用 MPL-2.0（cssparser、selectors、dtoa-short、option-ext）；它们未经修改直接使用，其源代码可从 crates.io 按 `Cargo.lock` 中的版本获取。请参阅 THIRD_PARTY_LICENSES.md §9。

### 开发者工具 a2dpwb_decode

`a2dpwb_decode`（请参阅[使用方法](usage#verify-stream)）只能从源代码构建，不包含在发布版本中。它链接上述固定版本的 Bluedroid SBC 解码器（Apache-2.0）、fdk-aac（FDK AAC License）、libopenaptx 0.2.0（LGPL-2.1+）、libopus（BSD-3-Clause）和 liblc3（Apache-2.0，随 BTstack 附带），**不**链接 BTstack 或 libldac。请参阅 THIRD_PARTY_LICENSES.md §14。

### 测试环境 tools/emu 和 tools/linux_sink

`tools/emu`（请参阅[构建](building)）和测量用接收器 `tools/linux_sink`（请参阅[使用方法](usage#receiver)）使用 [Bumble](https://github.com/google/bumble) 0.0.234（Apache-2.0）及其依赖项，它们以固定版本下载到本地的、被 git 忽略的环境中。这些都不会链接到 A2DPWB 中，也不随 A2DPWB 分发。软件包列表和许可证请参阅 THIRD_PARTY_LICENSES.md §15。

### Bluetooth 认证

商用 Bluetooth 产品必须通过由 Bluetooth SIG 管理的 [Bluetooth Qualification Process](https://www.bluetooth.com/)（Bluetooth 认证流程）。
