# 源码来源与致谢

感谢以下项目的作者与维护者。下表区分实际使用的源码和设计 / 互操作参考；致谢不表示原作者参与、认可或担保本项目。

| 项目 | 本项目的使用方式 | 许可 / 版本 |
| --- | --- | --- |
| [SeiyaFunaokaJP/A2DP-Windows-Bridge](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge) | Windows 用户态 A2DP 引擎、WinUSB 驱动切换辅助程序、WASAPI、IPC、蓝牙会话基础；`reference/` 及修改补丁 | v2.0.1，`3a094af733c830a2f4f5bd7418e6c7ba1db94749`；Apache-2.0 OR MIT，第三方另计 |
| [Android Open Source Project](https://android.googlesource.com/platform/system/audio/) / [DBeidachazi/liblhdcv5](https://github.com/DBeidachazi/liblhdcv5) | 发布版实际编译的 LHDC v5 Rust 编码器；来自 mirror 的 `aosp/`，API 增加精确档位，压缩内核保留 | mirror `0df76b08196dc26e460404189279b17a7101af74`；Android `android-17.0.0_r1`，`c77db469de80f86660bc053bec4dee0c5d4b947c`；Apache-2.0 |
| [BlueKitchen/BTstack](https://github.com/bluekitchen/btstack) | 实际链接的用户态蓝牙栈；项目提供了 Windows 媒体运行循环派生文件 | `5bc5cbdbeec33be1fdbd0d50e04c0f6deab99d2d`；带个人非商业条款 |
| [WillyBilly06/LHDC-V5-Encoder](https://github.com/WillyBilly06/LHDC-V5-Encoder) | 早期 C 编码版 / A-B 对比及可选 C 后端来源；当前 Rust 版不编译该 C 编码器 | `3f9d1980fa9c57cdfb78792f9455a7dbac50edda`；Apache-2.0 / KISS FFT BSD 声明 |
| [YuKongA/Linux_LHDC](https://github.com/YuKongA/Linux_LHDC) | 开发起点的 Linux LHDC 启用指南与参考链接，不是 Windows 引擎源码 | 仅链接与设计参考 |
| [PipeWire](https://gitlab.freedesktop.org/pipewire/pipewire) | LHDC 接入、按音频采样时钟发送、队列反馈的设计参考；未把 Linux 蓝牙栈搬入 Windows | 仅设计参考，发行包不链接 PipeWire |
| [Leaf-lsgtky/OppoPods](https://github.com/Leaf-lsgtky/OppoPods) | 公开欢律协议研究、功能 ID 和机型能力资料的互操作参考 | 参考提交 `929d813f1b82d5b09dd39a84b4f18744e8ef7e64`；原项目 GPL-3.0，不随包复制或链接其实现 |
| [spizganed/QuickBuds](https://github.com/spizganed/QuickBuds) | 公开协议笔记、ANC 档位和回报语义参考；本项目使用独立 Windows RFCOMM 实现 | 参考提交 `bae58d4af9eeb8d1f3d7a0037b2dd1b27cc5397d`；原项目 GPL-3.0-or-later，不随包复制或链接其实现 |
| [linux-firmware / Intel](https://gitlab.com/kernel-firmware/linux-firmware) | Intel AX201 的未修改 `.sfi` / `.ddc` 固件 | 自有固件许可；`firmware/SOURCE.md`、`LICENCE.ibt_firmware` 与 `WHENCE` |
| [VB-Audio / Vincent Burel](https://vb-audio.com/Cable/) | 系统音源隔离依赖 VB-CABLE；保留完整原版安装包，用户自行安装 | donationware，独立许可，见 `dependencies/` |

上游还使用了 Sony / AOSP libldac、Pali Rohár / Aurelien Jacobs libopenaptx、Fraunhofer FDK AAC、Xiph.Org Opus、Google liblc3、micro-ecc、Bluedroid SBC、KISS FFT 及 Rust crates。原作者、精确提交和完整许可见 [SOURCE.md](SOURCE.md) 及 [上游第三方许可](engine/A2DPWB-2.0.1/THIRD_PARTY_LICENSES.md)。

本项目新增 C# 控制台、恢复监护、音源路由、功能面板及独立 C++ 辅助程序；引擎改动包括 LHDC 自适应策略、媒体时钟、高精度非阻塞定时发送、拥塞反馈、诊断字段和 Rust 编码器接口适配。详细修改记录见 `engine-source/MODIFICATIONS.md`。本项目与 OPPO、OnePlus、欢律、LHDC、Sony、Intel、Microsoft 或 BlueKitchen 无官方隶属关系。
