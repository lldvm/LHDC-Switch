# 源码构建与重新链接

## 工具

Windows x64；Rust stable MSVC（最低 1.90，开发验证 1.99.1）；Visual Studio C++ Build Tools（MSVC、Windows SDK）；.NET Framework C# 编译器和 .NET Framework 4.8 运行时。PowerShell 5.1 或 7，Git。

## 获取和构建

```powershell
git clone --recurse-submodules https://github.com/lldvm/LHDC-Switch.git
cd LHDC-Switch
git checkout v2.4.3
git submodule update --init --recursive
powershell -ExecutionPolicy Bypass -File .\build-public.ps1
```

`reference/` 是已应用改动的上游源代码快照，并非待打补丁的原版，**不要再次套用 adaptive.patch**。六个 C 库以根目录 `.gitmodules` 和 Git 提交锁定；AOSP Rust 编码器完整收录于 `reference/extern/aosp-lhdcv5/`。`Cargo.lock` 固定 Rust 依赖。

Release 同时提供 `LHDC-Switch-v2.4.3-source-with-submodules.zip`，包含本项目及六个子模块的完整源码，解压即可构建，无需从 Git 初始化子模块；Cargo 首次构建仍需下载锁定的 crates。GitHub 自动生成的 Source code 压缩包不包含子模块，请勿混淆。

`build-engine.ps1` 从 `reference/` 构建 `a2dpwb-core-exe --features aosp-rust` 并运行相关检查；`build-public.ps1` 另外构建 `a2dpwb-driver`、复制两个程序，调用 `build.ps1` 编译本项目 C++ 辅助程序和 C# 界面，按本次生成文件重新生成 `src/Integrity.cs`。保留 `.cargo/config.toml` 的静态 CRT 配置。上游 Tauri 图形界面不随本项目发布，也不需要编译。

源码、源码压缩包和构建过程不含开发者的配对记录或驱动备份。运行前先配置自己的设备。仅在需要修改本机驱动的实际连接阶段才请求 UAC，普通编译和单元检查不应切换驱动。

## 自检

先用 `configure.ps1` 设置本机实际支持的控制器和耳机地址，再运行不接管蓝牙的检查。控制器自检包含只读设备查询，因此不能用 `CONFIGURE_ME` 占位符通过完整自检：

```powershell
Start-Process .\LHDC-Switch.exe -ArgumentList '--self-test' -Wait
Start-Process .\LHDC-Switch.exe -ArgumentList '--ipc-test' -Wait
Get-Content data/selftest-results.json
Get-Content data/ipc-test-results.json
```

控制器自检 47 项；2.4.3 编码器定向单元检查 15 项、分包集成检查 11 项，覆盖所有四种采样率、新固定档 / 自动上限的实际字节量与 MTU、码率切换及恢复。传输 73 项、流水线 24 项、运行循环 2 项检查通过。额外传输 / 流水线检查由 `build-engine.ps1` 执行。检查通过不等于所有无线环境和耳机通过听音验证。

## 修改依赖与重新链接

引擎静态链接 LGPL-2.1-or-later 的 `libopenaptx`。可修改 `reference/extern/libopenaptx/openaptx.c`（或检出自己的兼容版本），再运行上述完整构建流程；其构建脚本会重新编译并链接该文件。构建后的控制器会接受新引擎哈希，不存在禁止用户重链接的签名锁。重新链接、调试或逆向以调试 LGPL 库修改时，请遵守适用许可证；本项目不额外禁止这些操作。

FDK AAC 及其他静态库的完整对应源码同样通过本仓库锁定子模块及随 Release 提供的源码包免费取得。不同许可证、专利与认证声明见 `THIRD_PARTY_NOTICES.md` 和上游许可证，重编译不会改变 BTstack 的非商业限制。

## 打包

```powershell
.\package-public.ps1
```

使用固定允许列表，包含程序、示例配置、文档、许可、原始依赖与固件；不包含实际 `settings.json`、整个 `data/`、配对密钥、恢复日志或驱动备份。打包前需要完整构建。源码压缩包的六个库须与当前 Git gitlink 一致。
