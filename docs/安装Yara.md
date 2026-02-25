# 如何安装 Yara（以便带规则扫描编译 boaz-core）

作者：Samuel · 2026-02-23

boaz-core 的「Yara 规则扫描」需要链接系统里的 **libyara**（Windows 上即 `yara.lib`）。本机没装 Yara 时，用 `cargo build --release --no-default-features` 可以编出无 Yara 版本；若想带规则扫描，可按下面任选一种方式。

---

## 方式一：Windows 用 vcpkg 安装（推荐）

1. **安装 vcpkg**（若还没有）  
   - 打开：<https://github.com/microsoft/vcpkg>  
   - 克隆并执行 `bootstrap-vcpkg.bat`，把 vcpkg 加到 PATH 或记住安装路径。

2. **安装 Yara**  
   在 PowerShell 或 CMD 里（vcpkg 所在目录）执行：
   ```text
   .\vcpkg install yara:x64-windows
   ```
   安装完成后记下输出里的「安装路径」，例如：  
   `C:\vcpkg\installed\x64-windows`

3. **找到 yara.lib 所在目录**  
   vcpkg 有时不会在 `installed\x64-windows` 下建 `lib` 文件夹，或把文件放在别处。在 PowerShell 里先搜一下：
   ```powershell
   Get-ChildItem -Path "D:\vcpkg\installed\x64-windows" -Recurse -Filter "yara*" -ErrorAction SilentlyContinue | Select-Object FullName
   ```
   - 若看到 **`...\lib\yara.lib`**，则 `YARA_LIBRARY_PATH` 设为该 **lib 目录**，例如：  
     `D:\vcpkg\installed\x64-windows\lib`
   - 若只有 **`...\bin\yara.dll`**，没有单独的 lib 目录，可先试把 `YARA_LIBRARY_PATH` 设为 **bin 目录**：  
     `D:\vcpkg\installed\x64-windows\bin`  
     若链接仍报找不到 yara.lib，再改用下面的「用 LIB 指定」方式。

4. **让 Rust 找到 yara.lib**  
   在 **boaz-core** 目录下执行（路径按上一步结果改）：
   ```powershell
   # 情况 A：有 lib 目录且里面有 yara.lib
   $env:YARA_LIBRARY_PATH = "D:\vcpkg\installed\x64-windows\lib"

   # 情况 B：只有 bin 目录，先试 bin
   $env:YARA_LIBRARY_PATH = "D:\vcpkg\installed\x64-windows\bin"

   cargo build --release
   ```
   若仍报 LNK1181，可用 MSVC 的 **LIB** 环境变量直接指定包含 `yara.lib` 的目录（同上，有 lib 就填 lib，否则填 bin）：
   ```powershell
   $env:LIB = "D:\vcpkg\installed\x64-windows\lib;$env:LIB"
   cargo build --release
   ```
   若使用 **MSVC 工具链**且 vcpkg 已做 `integrate install`，有时只需：
   ```powershell
   $env:VCPKG_ROOT = "D:\vcpkg"
   cargo build --release
   ```

5. **验证**  
   在 `boaz-core` 目录下能成功执行 `cargo build --release`（不报 LNK1181 / 找不到 yara.lib），即说明 Yara 已可用。

---

## 方式二：尝试 vendored（不装系统 Yara，从源码编）

boaz 使用的 Rust 库 **yara-sys** 支持 `vendored` feature：在编译时自动下载并编译 Yara 源码，无需本机预先安装 Yara。

1. **改 Cargo.toml**  
   在 `boaz-core/Cargo.toml` 里，把 yara 依赖的 features 从 `bundled-4_5_5` 改成 `vendored`：
   ```toml
   yara = { version = "0.31", optional = true, default-features = false, features = ["vendored"] }
   ```

2. **编译**  
   在项目根或 `boaz-core` 目录执行：
   ```text
   cargo build --release
   ```
   首次会拉取并编译 Yara 源码，时间较长。若在 Windows 上失败（缺少 CMake/编译环境等），可改回 `bundled-4_5_5` 并用方式一安装系统 Yara。

---

## 方式三：Linux / WSL 下安装系统 Yara

在 Debian/Ubuntu 上：
```bash
sudo apt-get update
sudo apt-get install libyara-dev
```
在 Fedora/RHEL 上：
```bash
sudo dnf install yara yara-devel
```

然后在本机或 WSL 里进入 `boaz-core` 目录执行 `cargo build --release` 即可。Linux 下一般能直接找到 `libyara.so`，无需再设环境变量。

---

## 小结

| 环境        | 建议做法 |
|-------------|----------|
| Windows 本机 | 方式一（vcpkg）或方式二（vendored 试一次） |
| WSL / Linux | 方式三（系统包）或方式二（vendored） |

装好 Yara 并成功 `cargo build --release` 后，打包脚本若仍用「无 Yara」编译，可在 `boaz-core` 里先单独编好带 Yara 的 exe，再复制到 `release\Boaz` 替换原来的 `boaz-core.exe` 使用。

---

## 没有 lib 文件夹时

若 `D:\vcpkg\installed\x64-windows` 下确实没有 `lib` 文件夹，说明当前 yara 端口可能只安装了 DLL（在 `bin` 里）。可以：

1. **先确认实际有哪些文件**（在 PowerShell 里）：
   ```powershell
   Get-ChildItem "D:\vcpkg\installed\x64-windows" -Recurse -Include "*.lib","*.dll" -ErrorAction SilentlyContinue | Where-Object { $_.Name -like "*yara*" }
   ```
2. **若只有 yara.dll**：把 `YARA_LIBRARY_PATH` 设为 `D:\vcpkg\installed\x64-windows\bin` 试一次；若仍报找不到 yara.lib，可改用 **方式二（vendored）**，让 Rust 从源码编一份 Yara，不依赖 vcpkg 的 lib 路径。
3. **若希望 vcpkg 生成 .lib**：可尝试静态 triplet：  
   `.\vcpkg install yara:x64-windows-static`  
   静态安装后通常会在 `installed\x64-windows-static\lib` 下出现 `yara.lib`，再把 `YARA_LIBRARY_PATH` 设为该路径并编译（注意：静态链接时可能需要同时设 `YARA_STATIC=1` 或使用 crate 的 `yara-static` feature，按 yara-sys 文档操作）。
