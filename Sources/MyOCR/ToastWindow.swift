import AppKit

@MainActor
final class ToastWindow {
    enum Style {
        case success
        case warning
        case information

        var symbolName: String {
            switch self {
            case .success: return "checkmark.circle.fill"
            case .warning: return "exclamationmark.triangle.fill"
            case .information: return "textformat"
            }
        }

        var color: NSColor {
            switch self {
            case .success: return .systemGreen
            case .warning: return .systemOrange
            case .information: return .systemBlue
            }
        }
    }

    private let panel: NSPanel
    private let iconView: NSImageView
    private let label: NSTextField
    private var dismissTask: Task<Void, Never>?

    init() {
        panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 300, height: 52),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        iconView = NSImageView(frame: NSRect(x: 16, y: 16, width: 20, height: 20))
        label = NSTextField(labelWithString: "")

        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.level = .statusBar
        panel.hidesOnDeactivate = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.ignoresMouseEvents = true

        let effectView = NSVisualEffectView(frame: panel.contentView?.bounds ?? .zero)
        effectView.autoresizingMask = [.width, .height]
        effectView.material = .hudWindow
        effectView.blendingMode = .behindWindow
        effectView.state = .active
        effectView.wantsLayer = true
        effectView.layer?.cornerRadius = 12
        effectView.layer?.masksToBounds = true

        iconView.imageScaling = .scaleProportionallyUpOrDown

        label.font = NSFont.systemFont(ofSize: 14, weight: .medium)
        label.textColor = .labelColor
        label.alignment = .left
        label.lineBreakMode = .byTruncatingTail

        effectView.addSubview(iconView)
        effectView.addSubview(label)
        panel.contentView = effectView
    }

    func show(_ message: String, style: Style) {
        dismissTask?.cancel()

        iconView.image = NSImage(
            systemSymbolName: style.symbolName,
            accessibilityDescription: message
        )
        iconView.contentTintColor = style.color
        label.stringValue = message
        label.frame = NSRect(x: 48, y: 16, width: 232, height: 20)

        if let screen = NSScreen.main ?? NSScreen.screens.first {
            let frame = screen.visibleFrame
            let size = panel.frame.size
            panel.setFrameOrigin(NSPoint(
                x: frame.maxX - size.width - 20,
                y: frame.maxY - size.height - 20
            ))
        }

        panel.alphaValue = 1
        panel.orderFrontRegardless()

        dismissTask = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 1_800_000_000)
            guard !Task.isCancelled else { return }
            self?.panel.orderOut(nil)
        }
    }

    deinit {
        dismissTask?.cancel()
    }
}
