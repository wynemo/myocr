# MyOCR for Windows

Rust 编写的 Windows 11 x64 托盘 OCR 工具，使用系统 Windows.Media.Ocr，离线识别，无需 API Key。macOS 版保留原来的 Swift 实现。

## 下载与运行

1. 打开 https://github.com/wynemo/myocr/actions/workflows/windows.yml ，选择成功的构建，在页面底部 Artifacts 下载 **MyOCR-Windows-portable-x64**（下载需要登录 GitHub）。
2. 将 ZIP 解压到一个目录，双击 **myocr.exe**。
3. 程序常驻托盘，按 **Ctrl+Alt+O** 框选文字。托盘图标可能收在右下角的隐藏图标区域。

这是未签名的便携版，无需安装、导入证书或管理员权限。Windows 可能显示 SmartScreen 未识别应用提示，是否允许运行受本机安全策略控制。程序静态链接 C 运行库，无需另装 Rust 或 Visual C++ 运行库。

更新时先通过托盘菜单退出 MyOCR，再用新版文件替换旧文件。首次使用请确认系统已安装对应的 OCR 语言包。

## 使用

- 按 **Ctrl+Alt+O**，或左键点击托盘图标；拖动鼠标框选，松开后识别并自动复制文字。
- **Esc**、右键或切换到其他应用取消框选，不改变剪贴板。
- 右键托盘图标，选择 **OCR language** 或退出。启动时优先选择已安装的中文 OCR 引擎；没有中文时使用 Windows 用户语言配置。它不会自动检测所有语言，可在菜单手动选语言，或选择 **Use Windows profile language** 恢复 Windows 配置。当前语言选择仅在本次运行有效。
- 有结果时播放系统提示音并显示通知；无文字时保留原剪贴板。关闭 Windows 通知不影响复制功能。
- 支持多显示器、负坐标布局和每显示器 DPI 感知。截图是启动框选时的静态画面。受保护内容、UAC 安全桌面等无法正常截取。
- 需要 Windows 已安装对应的 OCR 语言包。若中文或英语没有出现在菜单，去“设置 → 时间和语言 → 语言和区域 → 语言选项”安装对应语言的 OCR 功能。首次启动若没有任何可用识别语言，会显示提示后退出。
- 单次选区尺寸受系统 `OcrEngine.MaxImageDimension` 限制，超限时提示缩小区域。
- 快捷键被其他程序占用时会提示并退出。暂不提供自定义快捷键、开机自启、ARM64 原生包或复杂表格重建。

## 无法识别中文

先右键托盘 → **OCR language**，确认当前勾选的是中文（例如 `zh-Hans` / `zh-Hant`）。Windows 用户配置模式可能选中英语引擎，并不会自动识别其他语言。

如果菜单没有中文，说明系统没有提供可用的中文 OCR 模型。安装中文输入法或使用中文界面，并不等于已安装 OCR 组件。可以打开**管理员 PowerShell**安装简体中文 OCR：

```powershell
Add-WindowsCapability -Online -Name Language.OCR~~~zh-CN~0.0.1.0
```

安装系统组件需要管理员权限和 Windows Update 可用；MyOCR 自身仍按普通用户运行。安装后从托盘退出 MyOCR 再重新打开，中文应出现在语言菜单中。繁体中文对应组件是 `Language.OCR~~~zh-TW~0.0.1.0`。托盘里的 **Chinese OCR setup...** 也提供这些操作提示。

如果已经明确选择中文仍无法识别，请记录是空结果、错误提示还是乱码，并确认选区中的文字足够清晰，以便进一步排查。

## 删除与旧版迁移

便携版退出后直接删除解压目录即可。

如果之前安装过 MSIX 测试版，先从托盘退出旧版，并在 Windows 设置的“已安装的应用”中卸载 MyOCR，避免两个版本同时运行。也可以在普通用户 PowerShell 执行：

```powershell
Get-AppxPackage Wynemo.MyOCR | Remove-AppxPackage
```

旧版导入的测试证书可由管理员在 `certlm.msc` 的“受信任人 → 证书”中，按照旧下载包 `MyOCR-Test.cer` 的指纹移除。便携版不使用该证书。

## 构建

通过 GitHub Actions 的 **Windows → Run workflow** 编译、运行几何逻辑单元测试和 Clippy 检查，生成便携版 ZIP。产物内只有 `myocr.exe`、本说明和 `SHA256SUMS.txt` 校验清单。Windows 编译全部在 GitHub 的 Windows runner 上完成，macOS 本地无需安装 Windows 工具链。

程序直接从普通 Win32 进程调用系统 Windows.Media.Ocr，不注册应用包身份。CI 的编译与单元测试不能替代真实 Windows 11 桌面上的框选、OCR 语言包、通知和多屏缩放验收。请在目标 Windows 机器测试这些交互。
