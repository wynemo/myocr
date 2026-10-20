# MyOCR for Windows

Rust 编写的 Windows 11 x64 托盘 OCR 工具，优先使用本机截图工具的 OneOCR，失败时回退 Windows.Media.Ocr；全程离线识别，无需 API Key。macOS 版保留原来的 Swift 实现。

## 下载与运行

1. 打开 https://github.com/wynemo/myocr/actions/workflows/windows.yml ，选择成功的构建，在页面底部 Artifacts 下载 **MyOCR-Windows-portable-x64**（下载需要登录 GitHub）。
2. 将 ZIP 解压到一个目录，双击 **myocr.exe**。
3. 程序常驻托盘，按 **Ctrl+Alt+O** 框选文字。托盘图标可能收在右下角的隐藏图标区域。

这是未签名的便携版，无需安装、导入证书或管理员权限。Windows 可能显示 SmartScreen 未识别应用提示，是否允许运行受本机安全策略控制。程序静态链接 C 运行库，无需另装 Rust 或 Visual C++ 运行库。

更新时先通过托盘菜单退出 MyOCR，再用新版文件替换旧文件。首次使用建议在 Microsoft Store 安装或更新“截图工具”（Snipping Tool）。旧版回退引擎需要系统对应的 OCR 语言包。

## 使用

- 按 **Ctrl+Alt+O**，或左键点击托盘图标；拖动鼠标框选，松开后识别并自动复制文字。
- **Esc**、右键或切换到其他应用取消框选，不改变剪贴板。
- 右键托盘 → **OCR engine** 默认选择 **Automatic (prefer OneOCR)**。OneOCR 自动识别语言，不使用旧版语言菜单。可切换 **Windows OCR (legacy)** 比较两套引擎。
- **Engine status...** 显示 OneOCR 是否加载成功、加载目录、最近一次实际使用的引擎及回退原因；复制通知也会标出引擎。
- **Open in system browser** 使用 Windows 默认浏览器打开项目 GitHub 页面。
- **Windows OCR language (fallback)** 仅控制旧版引擎：启动时优先选已安装的中文，没有中文则使用 Windows 用户语言配置。它不会自动识别所有语言，当前选择仅在本次运行有效。
- 有结果时播放系统提示音并显示通知；无文字时保留原剪贴板。关闭 Windows 通知不影响复制功能。
- 支持多显示器、负坐标布局和每显示器 DPI 感知。截图是启动框选时的静态画面。受保护内容、UAC 安全桌面等无法正常截取。
- OneOCR 使用截图工具自带模型。仅当使用 Windows OCR 回退引擎时，才需要系统对应的 OCR 语言包；没有旧版语言包也可以启动程序并尝试 OneOCR。
- 使用 Windows OCR 回退引擎时，选区受 `OcrEngine.MaxImageDimension` 限制；OneOCR 单次最多提取 1000 行。
- 快捷键被其他程序占用时会提示并退出。暂不提供自定义快捷键、开机自启、ARM64 原生包或复杂表格重建。

## OneOCR 加载与诊断

程序会枚举**当前用户**安装的 `Microsoft.ScreenSketch`，在安装目录及其 `SnippingTool` 子目录中查找 `oneocr.dll`、`onnxruntime.dll` 和 `oneocr.onemodel`。为避免 `WindowsApps` 的 DLL 加载权限限制，首次运行会将本机的三个文件复制到 `%LOCALAPPDATA%\MyOCR\OneOCR\` 下的版本缓存，再从缓存加载。复制只读取原文件内容，不继承其受限权限；无需安装证书、提权、修改系统目录权限或改变 DLL 全局搜索路径。初始化和模型保留在一个后台线程，重复框选时复用。

如果自动发现失败，可先更新 Microsoft Store 的“截图工具”并重启 MyOCR。也支持将**自己机器上同一版本截图工具**的上述三个文件一起放在 `myocr.exe` 旁的 `oneocr/` 文件夹（或直接放在 EXE 旁）再重启。可用普通 PowerShell 查询截图工具路径：

```powershell
Get-AppxPackage Microsoft.ScreenSketch | Select-Object -ExpandProperty InstallLocation
```

缓存只在三个文件复制完整后发布；截图工具版本或源文件变化时会创建新缓存。缓存读取或复制失败时，**Engine status...** 会显示源路径、缓存路径及失败原因，并回退旧版引擎。

本仓库与 CI 产物不附带微软 DLL 或模型，也不会自动联网下载它们。OneOCR 是截图工具的非公开接口，版本变化可能导致加载或调用不兼容；遇到可恢复的加载/识别错误会回退旧版引擎。成功识别为空时保留空结果，不用另一个引擎覆盖。可在 **Engine status...** 查看实际状态及原因，更新截图工具后需重启 MyOCR。

## 无法识别中文

先检查 **Engine status...** 最近一次是否使用 `OneOCR`。如果是回退的 `Windows.Media.Ocr`，请确认 **Windows OCR language (fallback)** 勾选中文（例如 `zh-Hans` / `zh-Hant`）。Windows 用户配置模式可能选中英语引擎，并不会自动识别其他语言。

如果菜单没有中文，说明系统没有提供可用的中文 OCR 模型。安装中文输入法或使用中文界面，并不等于已安装 OCR 组件。可以打开**管理员 PowerShell**安装简体中文 OCR：

```powershell
Add-WindowsCapability -Online -Name Language.OCR~~~zh-CN~0.0.1.0
```

安装系统组件需要管理员权限和 Windows Update 可用；MyOCR 自身仍按普通用户运行。安装后从托盘退出 MyOCR 再重新打开，中文应出现在语言菜单中。繁体中文对应组件是 `Language.OCR~~~zh-TW~0.0.1.0`。托盘里的 **Chinese OCR setup...** 也提供这些操作提示。

如果已经明确选择中文仍无法识别，请记录是空结果、错误提示还是乱码，并确认选区中的文字足够清晰，以便进一步排查。

## 删除与旧版迁移

便携版退出后可删除解压目录。自动复制的 OneOCR 文件位于 `%LOCALAPPDATA%\MyOCR\OneOCR\`；如需完整清理，可在退出 MyOCR 后删除该缓存目录，系统截图工具不受影响。

如果之前安装过 MSIX 测试版，先从托盘退出旧版，并在 Windows 设置的“已安装的应用”中卸载 MyOCR，避免两个版本同时运行。也可以在普通用户 PowerShell 执行：

```powershell
Get-AppxPackage Wynemo.MyOCR | Remove-AppxPackage
```

旧版导入的测试证书可由管理员在 `certlm.msc` 的“受信任人 → 证书”中，按照旧下载包 `MyOCR-Test.cer` 的指纹移除。便携版不使用该证书。

## 构建

通过 GitHub Actions 的 **Windows → Run workflow** 编译、运行截图几何、引擎回退、像素格式、原生资源清理、缓存完整性及 DLL 执行权限回归测试和 Clippy 检查，生成便携版 ZIP。产物包含 `myocr.exe`、本说明、`THIRD_PARTY_NOTICES.md` 和 `SHA256SUMS.txt` 校验清单。Windows 编译全部在 GitHub 的 Windows runner 上完成，macOS 本地无需安装 Windows 工具链。

程序直接从普通 Win32 进程动态加载 OneOCR，并保留系统 Windows.Media.Ocr 回退，不注册应用包身份。CI 的模拟 ABI 测试覆盖文本提取及资源释放，但不代表已验证目标机器上的微软模型。真实 OneOCR 效果、框选、通知和多屏缩放仍需在目标 Windows 11 机器验收。
