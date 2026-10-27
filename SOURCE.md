# 源码与版本清单

本项目：<https://github.com/lldvm/LHDC-Switch>，首次公开版本 `v2.4.2`。所有本项目控制器源码、构建 / 配置脚本、许可证与修改后的引擎源码均在此仓库；完整子模块源码另随同一 Release 提供，免费获取。

## 引擎与编码器

- 上游：<https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge>，v2.0.1，`3a094af733c830a2f4f5bd7418e6c7ba1db94749`。
- `reference/`：该提交的源码快照加本项目修改，保留原版权和锁定依赖；没有复制上游 Git 历史或开发者运行数据。
- `engine-source/adaptive.patch`：从上述上游提交生成的历史完整改动补丁（包含新源文件与 Rust 编码器 overlay），供审阅和重建对照。仓库中的已修改源码为构建入口，无需重复应用。公开快照另外添加了逐文件修改声明，原补丁未包含这些仅注释的声明。
- AOSP Rust：<https://github.com/DBeidachazi/liblhdcv5/tree/0df76b08196dc26e460404189279b17a7101af74/aosp>；mirror 标明 Android tag `android-17.0.0_r1`、`system/audio/codecs/lhdcv5`、`c77db469de80f86660bc053bec4dee0c5d4b947c`。来源详情见 `reference/extern/aosp-lhdcv5/SOURCE.txt`。
- 上游驱动辅助程序来自原版 v2.0.1 便携包；对应源码为 `reference/app/src/driver/`。SHA256：`f57f4b24613ad0ad07168fb01d2243dbd7a1a1fc6c06ca94165be987cb157b7e`。
- 当前发布的 Rust 核心 SHA256：`aa4f62a6c5c7299c7012066f72f9dfc598c799e1449b112014ac3703447add33`。编译器路径、版本和环境不同可能产生不同二进制；不声称可逐字节复现。

## 锁定 C 子模块

| 路径（位于 reference/） | 上游 | 提交 |
| --- | --- | --- |
| `extern/btstack` | https://github.com/bluekitchen/btstack.git | `5bc5cbdbeec33be1fdbd0d50e04c0f6deab99d2d` |
| `extern/fdk-aac` | https://github.com/mstorsjo/fdk-aac.git | `d8e6b1a3aa606c450241632b64b703f21ea31ce3` |
| `extern/lhdcv5-enc` | https://github.com/WillyBilly06/LHDC-V5-Encoder.git | `3f9d1980fa9c57cdfb78792f9455a7dbac50edda` |
| `extern/libldac` | https://android.googlesource.com/platform/external/libldac | `eeee1a3f5f8df1282e3a6d297085885fd886737b` |
| `extern/libopenaptx` | https://github.com/pali/libopenaptx.git | `2459ed4686eaef0a19dfa3f330a960813c5f60de` |
| `extern/opus` | https://github.com/xiph/opus.git | `ddbe48383984d56acd9e1ab6a090c54ca6b735a6` |

Rust crates 由 `reference/Cargo.lock` 固定；外部固件由 `firmware/SOURCE.md` 和 WHENCE 记录，VB-CABLE 来源见 `dependencies/SOURCE.md`。

取得源码后可更换 LGPL 库并完整重新链接，步骤见 [BUILDING.zh-CN.md](docs/BUILDING.zh-CN.md)。如项目迁移，版本标签、源码归档和此清单应与二进制同时保留。
