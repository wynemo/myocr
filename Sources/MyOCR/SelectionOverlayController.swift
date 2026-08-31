import AppKit

@MainActor
final class SelectionOverlayController {
    struct Selection {
        let screen: NSScreen
        let rect: CGRect
    }

    private var panels: [SelectionPanel] = []
    private var onSelection: ((Selection) -> Void)?
    private var onCancel: (() -> Void)?
    private var isSelecting = false

    func begin(
        onSelection: @escaping (Selection) -> Void,
        onCancel: @escaping () -> Void
    ) {
        dismiss()
        self.onSelection = onSelection
        self.onCancel = onCancel
        isSelecting = true

        let screens = NSScreen.screens
        guard !screens.isEmpty else {
            isSelecting = false
            onCancel()
            return
        }

        panels = screens.map { screen in
            let panel = SelectionPanel(screen: screen)
            panel.onSelection = { [weak self] localRect in
                guard let self else { return }
                let globalRect = CGRect(
                    x: screen.frame.minX + localRect.minX,
                    y: screen.frame.minY + localRect.minY,
                    width: localRect.width,
                    height: localRect.height
                )
                self.finish(Selection(screen: screen, rect: globalRect))
            }
            panel.onCancel = { [weak self] in
                self?.cancel()
            }
            return panel
        }

        for panel in panels {
            panel.orderFrontRegardless()
        }

        let mouseLocation = NSEvent.mouseLocation
        let preferredPanel = panels.first(where: { $0.targetScreen.frame.contains(mouseLocation) }) ?? panels[0]
        preferredPanel.makeKey()
        preferredPanel.makeFirstResponder(preferredPanel.selectionView)
    }

    func dismiss() {
        for panel in panels {
            panel.orderOut(nil)
            panel.close()
        }
        panels.removeAll()
        isSelecting = false
        onSelection = nil
        onCancel = nil
    }

    private func finish(_ selection: Selection) {
        guard isSelecting else { return }
        let callback = onSelection
        dismiss()
        callback?(selection)
    }

    private func cancel() {
        guard isSelecting else { return }
        let callback = onCancel
        dismiss()
        callback?()
    }
}

private final class SelectionPanel: NSPanel {
    let targetScreen: NSScreen
    let selectionView: SelectionView

    var onSelection: ((CGRect) -> Void)?
    var onCancel: (() -> Void)?

    init(screen: NSScreen) {
        self.targetScreen = screen
        self.selectionView = SelectionView()

        super.init(
            contentRect: screen.frame,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )

        level = .screenSaver
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        hidesOnDeactivate = false
        becomesKeyOnlyIfNeeded = false
        acceptsMouseMovedEvents = true
        isReleasedWhenClosed = false
        ignoresMouseEvents = false

        selectionView.frame = NSRect(origin: .zero, size: screen.frame.size)
        selectionView.autoresizingMask = [.width, .height]
        selectionView.onSelection = { [weak self] rect in
            self?.onSelection?(rect)
        }
        selectionView.onCancel = { [weak self] in
            self?.onCancel?()
        }
        contentView = selectionView
        initialFirstResponder = selectionView
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }
}

private final class SelectionView: NSView {
    var onSelection: ((CGRect) -> Void)?
    var onCancel: (() -> Void)?

    private var startPoint: CGPoint?
    private var currentPoint: CGPoint?

    override var acceptsFirstResponder: Bool { true }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.contentsScale = NSScreen.main?.backingScaleFactor ?? 2
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func resetCursorRects() {
        addCursorRect(bounds, cursor: .crosshair)
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)

        NSColor.black.withAlphaComponent(0.46).setFill()
        dirtyRect.fill()

        if let selectionRect {
            NSGraphicsContext.current?.compositingOperation = .clear
            NSBezierPath(rect: selectionRect).fill()
            NSGraphicsContext.current?.compositingOperation = .sourceOver

            NSColor.systemBlue.setStroke()
            let border = NSBezierPath(rect: selectionRect)
            border.lineWidth = 2
            border.stroke()

            let sizeText = "\(Int(selectionRect.width)) × \(Int(selectionRect.height))"
            let attributes: [NSAttributedString.Key: Any] = [
                .font: NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .medium),
                .foregroundColor: NSColor.white
            ]
            let textSize = (sizeText as NSString).size(withAttributes: attributes)
            let labelRect = NSRect(
                x: selectionRect.maxX - textSize.width - 12,
                y: selectionRect.minY - textSize.height - 10,
                width: textSize.width + 12,
                height: textSize.height + 6
            )
            NSColor.systemBlue.withAlphaComponent(0.92).setFill()
            NSBezierPath(roundedRect: labelRect, xRadius: 4, yRadius: 4).fill()
            (sizeText as NSString).draw(
                at: NSPoint(x: labelRect.minX + 6, y: labelRect.minY + 3),
                withAttributes: attributes
            )
        }

        let hint = "拖动选择文字区域   ·   Esc 取消"
        let hintAttributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 13, weight: .medium),
            .foregroundColor: NSColor.white
        ]
        let hintSize = (hint as NSString).size(withAttributes: hintAttributes)
        let hintRect = NSRect(
            x: (bounds.width - hintSize.width - 28) / 2,
            y: bounds.height - hintSize.height - 22,
            width: hintSize.width + 28,
            height: hintSize.height + 12
        )
        NSColor.black.withAlphaComponent(0.68).setFill()
        NSBezierPath(roundedRect: hintRect, xRadius: 8, yRadius: 8).fill()
        (hint as NSString).draw(
            at: NSPoint(x: hintRect.minX + 14, y: hintRect.minY + 6),
            withAttributes: hintAttributes
        )
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        startPoint = clampedPoint(from: event)
        currentPoint = startPoint
        needsDisplay = true
    }

    override func mouseDragged(with event: NSEvent) {
        guard startPoint != nil else { return }
        currentPoint = clampedPoint(from: event)
        needsDisplay = true
    }

    override func mouseUp(with event: NSEvent) {
        guard startPoint != nil else { return }
        currentPoint = clampedPoint(from: event)
        let rect = selectionRect ?? .zero
        self.startPoint = nil
        self.currentPoint = nil

        guard rect.width >= 4, rect.height >= 4 else {
            needsDisplay = true
            return
        }
        onSelection?(rect)
    }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 {
            onCancel?()
        } else {
            super.keyDown(with: event)
        }
    }

    override func cancelOperation(_ sender: Any?) {
        onCancel?()
    }

    private var selectionRect: CGRect? {
        guard let startPoint, let currentPoint else { return nil }
        return CGRect(
            x: min(startPoint.x, currentPoint.x),
            y: min(startPoint.y, currentPoint.y),
            width: abs(startPoint.x - currentPoint.x),
            height: abs(startPoint.y - currentPoint.y)
        )
    }

    private func clampedPoint(from event: NSEvent) -> CGPoint {
        let point = convert(event.locationInWindow, from: nil)
        return CGPoint(
            x: min(max(point.x, bounds.minX), bounds.maxX),
            y: min(max(point.y, bounds.minY), bounds.maxY)
        )
    }
}
