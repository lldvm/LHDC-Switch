# LHDC Switch

在 Windows 上，通过电脑内置蓝牙播放 **LHDC v5** 的实验性工具。使用原生 AOSP Rust 编码器，带实时链路面板、固定 / 自适应码率、耳机音量控制和蓝牙恢复功能。

**[下载便携版](https://github.com/lldvm/LHDC-Switch/releases/latest) · [使用说明](docs/USAGE.zh-CN.md) · [源码来源与致谢](ACKNOWLEDGEMENTS.md) · [构建说明](docs/BUILDING.zh-CN.md)**

> 当前只允许接管 USB ID 为 **8087:0026** 的 Intel 蓝牙功能，实测设备为 Intel AX201 + OPPO Enco Free4 丹拿版。其他控制器不在此版本支持范围。接管期间，同一控制器上的其他蓝牙设备会断开，请准备有线或独立接收器输入设备。
>
> **随附 BTstack 的播放引擎限个人、非商业使用及再分发。** 本项目自编控制器源码为 MIT；上游引擎、编码库、固件和外部驱动各自遵循其许可证，不能把整个发行包理解为 MIT。见 [许可证说明](THIRD_PARTY_NOTICES.md)。

当前版本 **2.4.3** 修正各码率共用发送路径的额外等待、严重积压，以及自动模式停留在 480/500 kbps 的策略问题。见 [更新记录](docs/CHANGELOG.md)。

## 功能

- 按需接管蓝牙，断开 / 退出时恢复原 Windows 蓝牙驱动及默认播放设备。
- 显示实际协商的编码、编码器目标码率、发送速率、采样率、队列时长与本机丢弃计数。
- 固定码率：400 / 500 / **700 / 750 / 800** / 900 / 1000 kbps。
- 自动模式上限：400 / 500 / **700 / 750 / 800** / 900 / 1000 kbps，受耳机能力和 MTU 约束。
- RTP 媒体时钟定时发送、按积压时长和等待时间反馈拥塞；降档后可重新探测升档。
- AVRCP 音量调节与确认回包；关闭窗口驻留托盘，退出时恢复蓝牙。
- 通过 VB-CABLE 隔离系统声音，避免扬声器和耳机同时出声。
- Enco Free4 / 丹拿版实验性功能面板：高清音频、空间音频、降噪等；需先恢复 Windows 蓝牙。

## 快速开始

1. 从 [Releases](https://github.com/lldvm/LHDC-Switch/releases) 下载 `LHDC-Switch-v2.4.3-win-x64.zip`，完整解压到可写目录。需要 Windows x64、.NET Framework 4.8。
2. 需要“只在耳机播放”时，先安装 [VB-Audio 官方 VB-CABLE](https://vb-audio.com/Cable/)，按官方提示重启。发行包保留原始完整安装包及说明；安装由其官方独立安装器手动完成。
3. 双击 `配置设备.cmd`，填写你自己的耳机蓝牙地址；脚本只查询设备并写本地设置，不切换驱动。详见 [获取蓝牙地址](docs/USAGE.zh-CN.md#配置目标设备)。
4. 打开 `LHDC-Switch.exe`，将耳机置于配对模式并断开手机连接；先用 **48 kHz、自动最高 700**，点击“连接 LHDC”并接受驱动切换的 UAC 提示。
5. 不需要播放时点击“断开并恢复蓝牙”；完全退出使用托盘菜单“退出并恢复蓝牙”。异常恢复可运行 `恢复蓝牙.cmd`。

程序会在首次切换前导出原驱动并记录恢复信息。突然断电、系统崩溃或恢复进程被强制结束时不能保证立即恢复，请保留整个目录及 `data/` 记录。详见 [恢复方法](docs/USAGE.zh-CN.md#停止与恢复)。

## 实现与来源

```mermaid
flowchart LR
    A[Windows 应用 / VB-CABLE] --> B[WASAPI 采集]
    B --> C[AOSP Rust LHDC v5]
    C --> D[RTP 媒体时钟 / 发送队列]
    D --> E[BTstack / WinUSB]
    E --> F[Intel 蓝牙 / LHDC 耳机]
```

Windows 原生蓝牙 A2DP 栈被临时替换为用户态引擎；这不是给系统所有蓝牙端点安装一个 LHDC 编解码器。核心基于 [SeiyaFunaokaJP/A2DP-Windows-Bridge](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge)，Rust 编码器来自 [DBeidachazi/liblhdcv5](https://github.com/DBeidachazi/liblhdcv5) 的 AOSP 源码。开发最初参考了 [YuKongA/Linux_LHDC](https://github.com/YuKongA/Linux_LHDC) 和 PipeWire 的 LHDC 路径。精确提交、使用方式和互操作资料列于 [SOURCE.md](SOURCE.md) 与 [致谢](ACKNOWLEDGEMENTS.md)。

## 已知限制与验证

这是针对有限硬件验证的实验项目。用户已确认此前 Rust 版本能播放，但报告部分固定和自动模式存在严重积压。2.4.3 针对共用发送路径和自动升档策略作了修正，通过真实编码字节量、MTU、分包时序、拥塞恢复与平台期回归检查；本次修正尚未完成耳机端试听。高码率仍会受到 2.4 GHz 干扰、距离、控制器吞吐与耳机缓冲影响。

“本机丢弃 0”不等于无线链路没有重传或耳机没有卡顿；“发送速率”含协议开销，不等于纯编码码率。44.1 kHz 的部分原生档位实际为 480 / 960 kbps，最终以协商值和界面为准。功能面板的协议支持也不代表所有耳机固件均已实测。

## 源码与构建

```powershell
git clone --recurse-submodules https://github.com/lldvm/LHDC-Switch.git
cd LHDC-Switch
powershell -ExecutionPolicy Bypass -File .\build-public.ps1
```

依赖 Windows x64、Rust MSVC 工具链（最低 1.90）、Visual Studio C++ Build Tools、Windows SDK 和 .NET Framework 编译器。不需要 Node、Tauri 前端或 Linux 虚拟机来构建本控制台。见 [完整构建与重新链接](docs/BUILDING.zh-CN.md)。

English: Experimental Windows LHDC v5 sender for Intel USB Bluetooth `8087:0026`, using A2DP Windows Bridge and the AOSP Rust encoder. Personal, non-commercial use only for the bundled BTstack-based engine. See source attribution and third-party notices before redistribution. No affiliation with or endorsement by codec, Bluetooth or earbud vendors.
