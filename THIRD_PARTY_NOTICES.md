# 第三方许可与发行说明

## 许可范围

`src/` 中 LHDC Switch 自编控制器 / 辅助程序及本项目原创脚本、文档采用根目录 MIT 许可证。`reference/` 中 A2DP Windows Bridge 源码及沿用其许可的引擎改动采用 Apache-2.0 OR MIT；AOSP Rust 编码器及其接口改动保留 Apache-2.0。第三方代码、固件、依赖包不受根目录 MIT 重新许可。

**随附 `a2dpwb-core.exe` 静态链接 BTstack。其第 4 条限制为个人受益、非商业目的及无金钱收益的使用、修改和再分发。整个随附运行包不能当作可商业使用的 MIT 软件。** 商业需求须向 BlueKitchen 和其他相关权利人另行确认授权。完整文本：[BTstack-LICENSE.txt](engine/A2DPWB-2.0.1/licenses/BTstack-LICENSE.txt)。

## 本发行版与上游的差别

上游原始 [`THIRD_PARTY_LICENSES.md`](engine/A2DPWB-2.0.1/THIRD_PARTY_LICENSES.md) 及所有版权 / NOTICE 文本保持随附。它描述上游全套程序，需结合本页理解本发行版：

- 只随附修改后的 `a2dpwb-core.exe` 和原始 `a2dpwb-driver.exe`，不随附上游 Tauri `a2dpwb.exe`、安装器或 WebView2。
- 当前 LHDC 后端为 **AOSP Rust**，C 移植编码器不参与该构建；源码保留可选 C 后端。Rust 来源与更改见 `reference/extern/aosp-lhdcv5/SOURCE.txt`，许可全文另随附在 `licenses/aosp-lhdcv5-LICENSE-APACHE.txt`。
- Windows BTstack 媒体运行循环派生文件已作修改，原始版权 / 许可保留。修改清单见 `engine-source/MODIFICATIONS.md`。
- 原有 Rust 许可合集 `licenses/rust-crates.txt` 保留；新增 / 更新依赖的版本、声明和许可原文在 `licenses/rust-overlay/`。`Cargo.lock` 是本次源码的依赖锁定记录。
- Intel 固件按 `firmware/LICENCE.ibt_firmware` 未修改再分发，来源、哈希及 WHENCE 随附。
- VB-CABLE 为 VB-Audio 的 donationware，来源 **www.vb-cable.com**，欢迎向原作者捐赠。`dependencies/` 保留完整、未修改的 Pack45 ZIP 及原始解压文件与许可。用户通过官方独立安装器手动安装；本项目不静默安装或修改该驱动。它不是 MIT 软件。
- OppoPods、QuickBuds、Linux_LHDC 与 PipeWire 为公开协议 / 设计参考；未将这些参考项目整体收录、编译或链接进本运行包。

## 完整对应源码与重新链接

核心还链接 **libopenaptx 0.2.0（LGPL-2.1-or-later）**、Fraunhofer FDK AAC 和其他上游静态库，即使本界面只提供 LHDC 也需保留其声明。完整对应源码通过本 GitHub 仓库的 `reference/`、六个锁定子模块、Cargo.lock 以及同一 Release 的 `source-with-submodules.zip` 免费提供；包含 LGPL 库和 FDK AAC 的源码及构建文件。可使用自己的修改版本重编译、重新链接引擎并重新生成控制器哈希。见 [构建说明](docs/BUILDING.zh-CN.md)。

FDK AAC 许可不授予专利许可；libldac 的 NOTICE 含产品认证声明。各编解码器、Bluetooth、LHDC 和产品名称的商标、专利与认证条件不因源码公开而消失。本项目未声称获得相关商业、专利或产品认证，也没有使用厂商品牌作认可宣传。具体文本请阅读随附原始许可。
