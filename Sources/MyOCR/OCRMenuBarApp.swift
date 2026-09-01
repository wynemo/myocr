import AppKit
import Carbon.HIToolbox

@MainActor
final class OCRMenuBarApp: NSObject, NSApplicationDelegate {
    private let hotKeyManager = HotkeyManager()
    private let screenshotService = SystemScreenshotService()
    private let ocrService = OCRService()
    private let clipboardService = ClipboardService()
    private let toastWindow = ToastWindow()

    private var statusItem: NSStatusItem!
    private var languageMenu: NSMenu!
    private var isBusy = false

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        NSApp.applicationIconImage = NSImage(systemSymbolName: "text.viewfinder", accessibilityDescription: "MyOCR")

        buildStatusItem()

        hotKeyManager.onHotKey = { [weak self] in
            self?.startOCRSelection()
        }

        do {
            try hotKeyManager.register(
                keyCode: UInt32(kVK_ANSI_O),
                modifiers: UInt32(cmdKey | shiftKey)
            )
        } catch {
            showError(title: "快捷键注册失败", message: error.localizedDescription)
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        hotKeyManager.unregister()
    }

    @objc private func startOCRFromMenu() {
        startOCRSelection()
    }

    @objc private func openScreenRecordingSettings() {
        let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")!
        NSWorkspace.shared.open(url)
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }

    @objc private func selectLanguage(_ sender: NSMenuItem) {
        guard let identifier = sender.representedObject as? String else { return }
        SettingsStore.shared.languageIdentifier = identifier
        updateLanguageMenuState()

        if let language = LanguageCatalog.language(withID: identifier) {
            toastWindow.show("识别语言：\(language.title)", style: .information)
        }
    }

    private func buildStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = statusItem.button {
            button.title = "OCR"
            let icon = NSImage(
                systemSymbolName: "text.viewfinder",
                accessibilityDescription: "MyOCR"
            )
            icon?.size = NSSize(width: 17, height: 17)
            icon?.isTemplate = true
            button.image = icon
            button.imagePosition = .imageLeft
            button.imageScaling = .scaleProportionallyDown
            button.font = NSFont.systemFont(ofSize: 12, weight: .medium)
            button.toolTip = "MyOCR · ⌘⇧O 使用系统截图识别文字"
        }

        let menu = NSMenu()

        let startItem = NSMenuItem(
            title: "识别屏幕文字",
            action: #selector(startOCRFromMenu),
            keyEquivalent: "o"
        )
        startItem.keyEquivalentModifierMask = [.command, .shift]
        menu.addItem(startItem)

        let languageItem = NSMenuItem(title: "识别语言", action: nil, keyEquivalent: "")
        languageMenu = NSMenu(title: "识别语言")
        for language in LanguageCatalog.all {
            let item = NSMenuItem(
                title: language.title,
                action: #selector(selectLanguage(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = language.id
            languageMenu.addItem(item)
        }
        languageItem.submenu = languageMenu
        menu.addItem(languageItem)
        updateLanguageMenuState()

        menu.addItem(.separator())

        let permissionItem = NSMenuItem(
            title: "打开屏幕录制权限设置…",
            action: #selector(openScreenRecordingSettings),
            keyEquivalent: ""
        )
        permissionItem.target = self
        menu.addItem(permissionItem)

        menu.addItem(.separator())

        let quitItem = NSMenuItem(title: "退出 MyOCR", action: #selector(quit), keyEquivalent: "q")
        quitItem.keyEquivalentModifierMask = [.command]
        quitItem.target = self
        menu.addItem(quitItem)

        menu.autoenablesItems = false
        statusItem.menu = menu
    }

    private func updateLanguageMenuState() {
        guard let languageMenu else { return }
        let selectedID = SettingsStore.shared.languageIdentifier
        for item in languageMenu.items {
            item.state = item.representedObject as? String == selectedID ? .on : .off
        }
    }

    private func startOCRSelection() {
        guard !isBusy else { return }

        guard screenshotService.requestPermissionIfNeeded() else {
            playFailureSound()
            showScreenRecordingPermissionAlert()
            return
        }

        isBusy = true
        Task { @MainActor in
            await self.processScreenshot()
        }
    }

    private func processScreenshot() async {
        defer { isBusy = false }

        do {
            let image = try await screenshotService.captureSelectedImage()
            let languageID = SettingsStore.shared.languageIdentifier
            let rawText = try await ocrService.recognize(image, languageID: languageID)
            let text = TextPostProcessor.clean(rawText)

            guard !text.isEmpty else {
                playFailureSound()
                toastWindow.show("没有识别到文字", style: .warning)
                return
            }

            clipboardService.copy(text)
            playSuccessSound()
            toastWindow.show("文字已复制到剪贴板", style: .success)
        } catch {
            if let screenshotError = error as? SystemScreenshotError,
               screenshotError == .cancelled {
                return
            } else {
                playFailureSound()
                showError(title: "识别失败", message: error.localizedDescription)
            }
        }
    }

    private func playSuccessSound() {
        NSSound(named: NSSound.Name("Glass"))?.play()
    }

    private func playFailureSound() {
        NSSound(named: NSSound.Name("Basso"))?.play()
    }

    private func showScreenRecordingPermissionAlert() {
        let alert = NSAlert()
        alert.messageText = "需要屏幕录制权限"
        alert.informativeText = "MyOCR 需要屏幕录制权限才能完成截图识别。请在“系统设置 → 隐私与安全性 → 屏幕录制”中允许 MyOCR，然后重新按 ⌘⇧O。框选界面仍使用 macOS 原生截图工具。"
        alert.addButton(withTitle: "打开系统设置")
        alert.addButton(withTitle: "稍后处理")

        NSApp.activate(ignoringOtherApps: true)
        if alert.runModal() == .alertFirstButtonReturn {
            openScreenRecordingSettings()
        }
    }

    private func showError(title: String, message: String) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.addButton(withTitle: "好")
        NSApp.activate(ignoringOtherApps: true)
        alert.runModal()
    }
}

@main
@MainActor
struct MyOCRMain {
    static func main() {
        let application = NSApplication.shared
        let delegate = OCRMenuBarApp()
        application.delegate = delegate
        withExtendedLifetime(delegate) {
            application.run()
        }
    }
}
