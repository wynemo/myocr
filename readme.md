# MyOCR

一个 macOS 菜单栏 OCR 小工具：按下 `⌘⇧O`，拖选屏幕区域，松开鼠标后识别文字并自动复制到剪贴板。

## 已实现

- 全局快捷键 `⌘⇧O`
- 调用 macOS 原生截图工具显示系统框选界面，`Esc` 取消
- 从系统截图写入的剪贴板图片读取选区，再交给 Vision OCR
- Apple Vision 离线 OCR，识别级别为 `.accurate`
- 自动检测语言，以及语言菜单选择
- 中文简体、中文繁体、英语、日语、韩语、法语、德语、西班牙语、意大利语、葡萄牙语、俄语、乌克兰语、阿拉伯语、泰语、越南语、印度尼西亚语、荷兰语、土耳其语、捷克语、丹麦语、瑞典语、罗马尼亚语、马来语、挪威语、波兰语
- 识别结果按文字框位置整理行列，并自动复制到系统剪贴板
- 菜单栏提示和空结果提示

## 运行

要求 macOS 14+、Xcode Command Line Tools 或 Xcode。

```bash
./scripts/build-app.sh
open build/MyOCR.app
```

也可以只验证 Swift 工程：

```bash
swift build
```

MyOCR 需要在“屏幕录制”中授权才能完成截图识别，但框选界面使用 macOS 自带的 `screencapture`，不是 MyOCR 自己绘制的遮罩。

构建脚本会优先使用本机的 Apple Development 签名。也可以手动指定签名：`CODESIGN_IDENTITY="你的签名名称" ./scripts/build-app.sh`。

## 使用方式

1. 启动 `MyOCR.app`，应用会驻留在菜单栏。
2. 按 `⌘⇧O`，或点击菜单栏图标里的“识别屏幕文字”。
3. 使用 macOS 原生截图框选需要识别的区域，松开鼠标；也可以按 `Esc` 取消。
4. MyOCR 从截图剪贴板读取图片，识别完成后会覆盖剪贴板并显示“文字已复制到剪贴板”。

菜单栏的“识别语言”可以切换自动检测或固定语言。自动检测默认覆盖中、英、日、韩及其他主流语言；Vision 会根据当前 macOS 版本实际提供的语言模型进行过滤。

## 项目结构

```text
Sources/MyOCR/
├── OCRMenuBarApp.swift              # 菜单栏 App、流程编排和权限提示
├── HotkeyManager.swift              # Carbon 全局快捷键
├── SystemScreenshotService.swift     # 调用 macOS 原生截图并读取剪贴板图片
├── OCRService.swift                 # Vision OCR、多语言和文本整理
├── ClipboardService.swift           # 写入 NSPasteboard
└── ToastWindow.swift                # 结果提示
```

应用使用 Apple Vision，识别过程离线完成，不会上传截图或识别文本。

## Windows 11 版（Rust）

Windows 版位于 [`windows/`](windows/README.md)，优先调用本机截图工具的 OneOCR，失败时回退系统 Windows OCR，支持 `Ctrl+Alt+O` 框选、托盘菜单和自动复制。

在 [GitHub Actions](https://github.com/wynemo/myocr/actions/workflows/windows.yml) 下载成功构建的 **MyOCR-Windows-portable-x64** 便携版，解压后双击 `myocr.exe` 即可运行，无需安装或导入证书。编译和检查在 GitHub Windows runner 完成，本机不需要安装 Windows 编译工具链。详细使用方法见 [Windows 使用说明](windows/README.md)。
