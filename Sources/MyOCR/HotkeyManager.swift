import Carbon.HIToolbox

enum HotkeyError: LocalizedError {
    case installHandler(OSStatus)
    case registerHotKey(OSStatus)

    var errorDescription: String? {
        switch self {
        case let .installHandler(status):
            return "无法安装全局快捷键监听器（错误码 \(status)）。"
        case let .registerHotKey(status):
            return "无法注册 ⌘⇧O（错误码 \(status)），可能与其他应用的快捷键冲突。"
        }
    }
}

final class HotkeyManager {
    var onHotKey: (() -> Void)?

    private var hotKeyRef: EventHotKeyRef?
    private var eventHandlerRef: EventHandlerRef?
    private var hotKeyID = EventHotKeyID(signature: 0, id: 0)

    func register(keyCode: UInt32, modifiers: UInt32) throws {
        unregister()

        hotKeyID = EventHotKeyID(signature: 0x4D594F43, id: 1) // “MYOC”
        var eventType = EventTypeSpec(
            eventClass: OSType(kEventClassKeyboard),
            eventKind: OSType(kEventHotKeyPressed)
        )

        let userData = UnsafeMutableRawPointer(Unmanaged.passUnretained(self).toOpaque())
        let installStatus = InstallEventHandler(
            GetApplicationEventTarget(),
            Self.eventHandler,
            1,
            &eventType,
            userData,
            &eventHandlerRef
        )
        guard installStatus == noErr else {
            throw HotkeyError.installHandler(installStatus)
        }

        let registerStatus = RegisterEventHotKey(
            keyCode,
            modifiers,
            hotKeyID,
            GetApplicationEventTarget(),
            0,
            &hotKeyRef
        )
        guard registerStatus == noErr else {
            RemoveEventHandler(eventHandlerRef)
            eventHandlerRef = nil
            throw HotkeyError.registerHotKey(registerStatus)
        }
    }

    func unregister() {
        if let hotKeyRef {
            UnregisterEventHotKey(hotKeyRef)
            self.hotKeyRef = nil
        }
        if let eventHandlerRef {
            RemoveEventHandler(eventHandlerRef)
            self.eventHandlerRef = nil
        }
    }

    deinit {
        unregister()
    }

    private static let eventHandler: EventHandlerUPP = { _, event, userData in
        guard let event, let userData else { return noErr }

        let manager = Unmanaged<HotkeyManager>
            .fromOpaque(userData)
            .takeUnretainedValue()
        manager.handle(event)
        return noErr
    }

    private func handle(_ event: EventRef) {
        var receivedID = EventHotKeyID()
        let status = GetEventParameter(
            event,
            EventParamName(kEventParamDirectObject),
            EventParamType(typeEventHotKeyID),
            nil,
            MemoryLayout<EventHotKeyID>.size,
            nil,
            &receivedID
        )

        guard status == noErr,
              receivedID.signature == hotKeyID.signature,
              receivedID.id == hotKeyID.id else { return }

        onHotKey?()
    }
}
