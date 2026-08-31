# MyOCR 代码审核报告

**审核日期:** 2026-08-31
**项目:** MyOCR — macOS 菜单栏 OCR 工具
**技术栈:** Swift 5.9 / macOS 14+ / AppKit / Vision / ScreenCaptureKit

---

## 总体评价

项目结构清晰，代码简洁，功能完整。作为一个轻量级菜单栏 OCR 工具，整体质量不错。以下是按严重程度分类的问题和改进建议。

---

## 🔴 严重问题

### 1. 成功路径未重置 `isBusy`，导致后续 OCR 无法触发

**文件:** `Sources/MyOCR/OCRMenuBarApp.swift:151-181`

`process()` 方法中，只有空文本和错误分支重置了 `isBusy = false`，成功路径（复制到剪贴板后）**没有重置**。成功完成后用户将无法再次触发 OCR。

**修复方案:** 在方法开头加 `defer { isBusy = false }`：

```swift
private func process(_ selection: SelectionOverlayController.Selection) async {
    selectionController.dismiss()
    defer { isBusy = false }  // ← 确保所有路径都重置状态

    do {
        try await Task.sleep(nanoseconds: 100_000_000)
        let image = try await captureService.capture(selection)
        // ...
    } catch {
        // ...
    }
}
```

---

## 🟡 中等问题

### 2. `HotkeyManager` 使用 `passUnretained` 存在潜在内存泄漏风险

**文件:** `Sources/MyOCR/HotkeyManager.swift:33`

```swift
let userData = UnsafeMutableRawPointer(Unmanaged.passUnretained(self).toOpaque())
```

`passUnretained` 不会增加引用计数。当前 `HotkeyManager` 作为 `OCRMenuBarApp` 的 `let` 属性持有（生命周期与 App 一致），实际不会出问题。但如果未来有人将 `HotkeyManager` 放入局部变量或临时持有，就会变成悬垂指针。

**建议:** 改用 `passRetained` 并在 `unregister()` 中释放，更安全。

---

### 3. Core Graphics 截图 API 已过时

**文件:** `Sources/MyOCR/ScreenCaptureService.swift:67`

```swift
@available(macOS, introduced: 10.6, deprecated: 14.4, obsoleted: 15.0)
private func captureWithCoreGraphics(selection:) -> CGImage?
```

`CGDisplayCreateImage` 在 macOS 15.0 已被标记为 obsoleted。当前代码在 macOS 14 上作为 fallback 使用是正确的，但 `Package.swift` 声明 `.macOS(.v14)`，如果用户升级到 macOS 15 但未授权 ScreenCaptureKit，fallback 仍会走 Core Graphics 路径，可能在更高版本上出问题。

**建议:** 考虑在 macOS 15+ 时强制要求 ScreenCaptureKit 权限，不再 fallback 到 Core Graphics。

---

### 4. 文本格式化中行合并阈值硬编码

**文件:** `Sources/MyOCR/OCRService.swift:128, 139`

```swift
// 行排序阈值
abs($0.box.midY - $1.box.midY) > max($0.box.height, $1.box.height) * 0.45

// 行分组阈值
let threshold = max(first.box.height, line.box.height) * 0.55
```

这两个魔法数字 (0.45, 0.55) 对于不同字体大小、不同 DPI 的截图可能不稳定。处理表格或小字体文本时可能出现错误合并或错误拆分。

**建议:** 提供一个配置项，或者根据字号动态调整阈值。

---

### 5. Toast 提示框的 label 宽度写死

**文件:** `Sources/MyOCR/ToastWindow.swift:80`

```swift
label.frame = NSRect(x: 48, y: 16, width: 232, height: 20)
```

标签宽度固定为 232pt，但面板宽度为 300pt。消息文字过长时会被截断（虽然有 `lineBreakMode = .byTruncatingTail`），但如果将来修改面板尺寸，这里不会自动适配。

**建议:** 使用 Auto Layout 或基于面板宽度动态计算。

---

### 6. `SettingsStore` 非线程安全

**文件:** `Sources/MyOCR/OCRService.swift:42-57`

`SettingsStore` 是一个单例，通过 `UserDefaults` 读写语言设置。虽然当前所有访问都在 `@MainActor` 上下文中，但作为公共 API 缺少线程安全保证。

**建议:** 如果确认只在主线程使用，加上 `@MainActor` 注解。

---

## 🟢 轻微问题 / 改进建议

### 7. `HotkeyManager` 的 event handler 回调缺少 `@convention(c)` 标注

**文件:** `Sources/MyOCR/HotkeyManager.swift:76-84`

```swift
private static let eventHandler: EventHandlerUPP = { _, event, userData in
```

`EventHandlerUPP` 的回调签名是 `@convention(c)`，Swift 编译器目前允许用闭包表达式赋值，但未显式标注。如果未来 Swift 版本收紧规则，可能会编译失败。

**建议:** 显式标注 `@convention(c)` 以增强健壮性。

---

### 8. `isCJK` Unicode 范围不够全面

**文件:** `Sources/MyOCR/OCRService.swift:169-177`

```swift
case 0x2E80...0x2FFF, 0x3040...0x30FF, 0x3400...0x4DBF,
     0x4E00...0x9FFF, 0xAC00...0xD7AF, 0xF900...0xFAFF:
```

缺少一些常用 CJK 范围：

| 范围 | 说明 |
|------|------|
| `0x3000...0x303F` | CJK 标点符号 |
| `0xFE30...0xFE4F` | CJK 兼容形式 |
| `0x20000...0x2A6DF` | CJK 扩展 B（部分生僻字） |

对于 OCR 场景，缺少 CJK 标点可能导致标点和汉字之间出现多余空格。

---

### 9. 没有单元测试

整个项目没有任何测试文件。建议至少对以下逻辑编写测试：

- `TextPostProcessor.clean()` — 文本清理
- `OCRService.shouldJoinWithoutSpace()` — 无空格拼接判断
- `OCRService.isCJK()` — CJK 字符检测
- `OCRService.resolve()` — 语言代码匹配

---

### 10. README 缺少 macOS 版本差异说明

`Package.swift` 声明 `.macOS(.v14)`，`ScreenCaptureService` 中使用了 `@available(macOS 15.2, *)`。建议在 README 中明确说明：

- macOS 14 用户使用 Core Graphics 截图路径（已有过时 API 警告）
- macOS 15.2+ 用户使用 ScreenCaptureKit（需授权屏幕录制权限）

---

## 📋 问题汇总

| 类别 | 编号 | 问题 | 文件 |
|------|------|------|------|
| 🔴 严重 | #1 | 成功路径未重置 `isBusy` | `OCRMenuBarApp.swift:151` |
| 🟡 中等 | #2 | `passUnretained` 潜在内存风险 | `HotkeyManager.swift:33` |
| 🟡 中等 | #3 | CG API 在 macOS 15 已过时 | `ScreenCaptureService.swift:67` |
| 🟡 中等 | #4 | 行合并阈值硬编码 | `OCRService.swift:128,139` |
| 🟡 中等 | #5 | Toast label 宽度写死 | `ToastWindow.swift:80` |
| 🟡 中等 | #6 | `SettingsStore` 缺少 `@MainActor` | `OCRService.swift:42` |
| 🟢 轻微 | #7 | 缺少 `@convention(c)` 标注 | `HotkeyManager.swift:76` |
| 🟢 轻微 | #8 | `isCJK` Unicode 范围不全 | `OCRService.swift:169` |
| 🟢 轻微 | #9 | 无单元测试 | — |
| 🟢 轻微 | #10 | README 缺少版本差异说明 | `readme.md` |
