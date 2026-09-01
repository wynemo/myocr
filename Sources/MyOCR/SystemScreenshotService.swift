import AppKit
import CoreGraphics

enum SystemScreenshotError: LocalizedError, Equatable {
    case cancelled
    case noImage
    case launchFailed(String)
    case captureFailed(String)

    var errorDescription: String? {
        switch self {
        case .cancelled:
            return "已取消截图。"
        case .noImage:
            return "无法读取系统截图。"
        case let .launchFailed(message):
            return "无法启动系统截图：\(message)"
        case let .captureFailed(message):
            return "系统截图失败：\(message)"
        }
    }
}

@MainActor
final class SystemScreenshotService {
    private let executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    private var activeProcess: Process?

    func requestPermissionIfNeeded() -> Bool {
        if CGPreflightScreenCaptureAccess() {
            return true
        }
        return CGRequestScreenCaptureAccess()
    }

    func captureSelectedImage() async throws -> CGImage {
        let pasteboard = NSPasteboard.general
        let previousChangeCount = pasteboard.changeCount
        let process = Process()
        process.executableURL = executableURL
        process.arguments = ["-i", "-s", "-c", "-x"]
        activeProcess = process

        return try await withCheckedThrowingContinuation { continuation in
            process.terminationHandler = { process in
                let status = process.terminationStatus

                Task { @MainActor in
                    self.activeProcess = nil

                    guard status == 0 else {
                        continuation.resume(
                            throwing: SystemScreenshotError.captureFailed(
                                "进程退出码：\(status)"
                            )
                        )
                        return
                    }

                    guard NSPasteboard.general.changeCount != previousChangeCount else {
                        continuation.resume(throwing: SystemScreenshotError.cancelled)
                        return
                    }

                    guard let image = Self.imageFromPasteboard(NSPasteboard.general) else {
                        continuation.resume(throwing: SystemScreenshotError.noImage)
                        return
                    }
                    continuation.resume(returning: image)
                }
            }

            do {
                try process.run()
            } catch {
                self.activeProcess = nil
                continuation.resume(
                    throwing: SystemScreenshotError.launchFailed(error.localizedDescription)
                )
            }
        }
    }

    private static func imageFromPasteboard(_ pasteboard: NSPasteboard) -> CGImage? {
        let imageTypes: [NSPasteboard.PasteboardType] = [.tiff, .png]

        for type in imageTypes {
            guard let data = pasteboard.data(forType: type),
                  let image = NSImage(data: data) else {
                continue
            }

            var proposedRect = NSRect(origin: .zero, size: image.size)
            if let cgImage = image.cgImage(
                forProposedRect: &proposedRect,
                context: nil,
                hints: nil
            ) {
                return cgImage
            }
        }

        return nil
    }
}
