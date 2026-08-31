# MyOCR

一个 macOS 菜单栏 OCR 小工具：按下 `⌘⇧O`，拖选屏幕区域，松开鼠标后识别文字并自动复制到剪贴板。

## 已实现

- 全局快捷键 `⌘⇧O`
- 跨显示器显示半透明框选层，`Esc` 取消
- macOS 15.2+ 使用 ScreenCaptureKit 截取选区，macOS 14 使用 Core Graphics 回退方案
- Apple Vision 离线 OCR，识别级别为 `.accurate`
- 自动检测语言，以及语言菜单选择
- 中文简体、中文繁体、英语、日语、韩语、法语、德语、西班牙语、意大利语、葡萄牙语、俄语、乌克兰语、阿拉伯语、泰语、越南语、印度尼西亚语、荷兰语、土耳其语、捷克语、丹麦语、瑞典语、罗马尼亚语、马来语、挪威语、波兰语
- 识别结果按文字框位置整理行列，并自动复制到系统剪贴板
- 菜单栏提示、空结果提示和屏幕录制权限引导

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

第一次按 `⌘⇧O` 时，如果系统没有授权，需要在“系统设置 → 隐私与安全性 → 屏幕录制”中允许 MyOCR，然后重新触发快捷键。

构建脚本会优先使用本机的 Apple Development 签名，避免每次重新构建都生成新的屏幕录制授权身份。也可以手动指定签名：`CODESIGN_IDENTITY="你的签名名称" ./scripts/build-app.sh`。

## 使用方式

1. 启动 `MyOCR.app`，应用会驻留在菜单栏。
2. 按 `⌘⇧O`，或点击菜单栏图标里的“识别屏幕文字”。
3. 拖动框选需要识别的区域，松开鼠标。
4. 识别结果会自动写入剪贴板，并显示“文字已复制到剪贴板”。

菜单栏的“识别语言”可以切换自动检测或固定语言。自动检测默认覆盖中、英、日、韩及其他主流语言；Vision 会根据当前 macOS 版本实际提供的语言模型进行过滤。

## 项目结构

```text
Sources/MyOCR/
├── OCRMenuBarApp.swift              # 菜单栏 App、流程编排和权限提示
├── HotkeyManager.swift              # Carbon 全局快捷键
├── SelectionOverlayController.swift # 多屏框选窗口
├── ScreenCaptureService.swift       # ScreenCaptureKit / Core Graphics 截图
├── OCRService.swift                 # Vision OCR、多语言和文本整理
├── ClipboardService.swift           # 写入 NSPasteboard
└── ToastWindow.swift                # 结果提示
```

应用使用 Apple Vision，识别过程离线完成，不会上传截图或识别文本。
