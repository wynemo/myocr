# MyOCR for Windows

Rust 编写的 Windows 11 x64 托盘 OCR 工具，使用系统 Windows.Media.Ocr，离线识别，无需 API Key。macOS 版保留原来的 Swift 实现。

## 下载与安装

1. 打开 https://github.com/wynemo/myocr/actions/workflows/windows.yml ，选择绿色成功的构建，在页面底部 Artifacts 下载 **MyOCR-Windows-x64**（下载需要登录 GitHub）。
2. 解压到一个目录，在该目录用普通用户 PowerShell 执行：

   ```powershell
   powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\Install.ps1
   ```

3. 首次安装需要接受一次管理员 UAC 提示，用于将该测试构建的公开签名证书导入“本地计算机 → 受信任人”。安装脚本随后在原来的普通用户会话安装 MSIX。不要用管理员身份启动整个脚本。只信任来自本仓库的产物；公司策略可能禁止侧载或证书导入。
4. 从开始菜单打开 **MyOCR**。托盘图标可能收在右下角的隐藏图标区域。

这是自签名测试包，尚未上架 Microsoft Store。每次 CI 重新生成证书，更新时可能再次要求证书信任。CI 不分发私钥。安装脚本会结束已有 MyOCR 进程以便更新。安装后启动、截图和 OCR 均不需要管理员权限。程序静态链接 C 运行库，无需另装 Rust 或 Visual C++ 运行库。

## 使用

- 按 **Ctrl+Alt+O**，或左键点击托盘图标；拖动鼠标框选，松开后识别并自动复制文字。
- **Esc**、右键或切换到其他应用取消框选，不改变剪贴板。
- 右键托盘图标，选择 **OCR language** 或退出。默认使用 Windows 用户语言配置，不等同于 macOS Vision 的多语言自动检测；可以手动选择系统提供的识别语言。当前语言选择仅在本次运行有效。
- 有结果时播放系统提示音并显示通知；无文字时保留原剪贴板。关闭 Windows 通知不影响复制功能。
- 支持多显示器、负坐标布局和每显示器 DPI 感知。截图是启动框选时的静态画面。受保护内容、UAC 安全桌面等无法正常截取。
- 需要 Windows 已安装对应的 OCR 语言包。若中文或英语没有出现在菜单，去“设置 → 时间和语言 → 语言和区域 → 语言选项”安装对应语言的 OCR 功能。首次启动若没有任何可用识别语言，会显示提示后退出。
- 单次选区尺寸受系统 `OcrEngine.MaxImageDimension` 限制，超限时提示缩小区域。
- 快捷键被其他程序占用时会提示并退出。暂不提供自定义快捷键、开机自启、ARM64 原生包或复杂表格重建。

## 卸载

在 Windows 设置的“已安装的应用”卸载 MyOCR，或普通用户 PowerShell 执行：

```powershell
Get-AppxPackage Wynemo.MyOCR | Remove-AppxPackage
```

测试证书不会随应用卸载自动移除。可打开 `certlm.msc`（需要管理员权限），在“受信任人 → 证书”中对照下载包 `MyOCR-Test.cer` 的指纹移除该构建的证书。不要仅凭同名删除其他证书。

## 构建

通过 GitHub Actions 的 **Windows → Run workflow** 编译、运行几何逻辑单元测试、Clippy 检查，并生成签名 MSIX、公开测试证书、安装脚本、说明和 SHA-256 校验清单。编译和 Windows SDK 打包全部在 GitHub 的 Windows runner 上完成，macOS 本地无需安装 Windows 工具链。

CI 的编译与单元测试不能替代真实 Windows 11 桌面上的框选、OCR 语言包、通知和多屏缩放验收。请在目标 Windows 机器测试这些交互。
