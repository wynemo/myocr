import AppKit
import CoreGraphics
import ScreenCaptureKit

enum ScreenCaptureError: LocalizedError, Equatable {
    case permissionDenied
    case invalidSelection
    case noImage
    case captureFailed(String)

    var errorDescription: String? {
        switch self {
        case .permissionDenied:
            return "没有屏幕录制权限。"
        case .invalidSelection:
            return "选中的区域太小，无法识别。"
        case .noImage:
            return "无法读取选中的屏幕区域。"
        case let .captureFailed(message):
            return "屏幕截图失败：\(message)"
        }
    }
}

final class ScreenCaptureService {
    func requestPermissionIfNeeded() -> Bool {
        if CGPreflightScreenCaptureAccess() {
            return true
        }
        return CGRequestScreenCaptureAccess()
    }

    func capture(_ selection: SelectionOverlayController.Selection) async throws -> CGImage {
        guard selection.rect.width >= 4, selection.rect.height >= 4 else {
            throw ScreenCaptureError.invalidSelection
        }

        guard CGPreflightScreenCaptureAccess() else {
            throw ScreenCaptureError.permissionDenied
        }

        if #available(macOS 15.2, *) {
            return try await captureWithScreenCaptureKit(selection: selection)
        }

        guard let image = captureWithCoreGraphics(selection: selection) else {
            throw ScreenCaptureError.noImage
        }
        return image
    }

    @available(macOS 15.2, *)
    private func captureWithScreenCaptureKit(
        selection: SelectionOverlayController.Selection
    ) async throws -> CGImage {
        let screen = selection.screen
        guard let displayID = displayID(for: screen) else {
            throw ScreenCaptureError.noImage
        }

        // AppKit screen coordinates have their origin at the bottom-left. The
        // ScreenCaptureKit screen-space API uses the display's top-left origin.
        let displayBounds = CGDisplayBounds(displayID)
        let localRect = CGRect(
            x: selection.rect.minX - screen.frame.minX,
            y: selection.rect.minY - screen.frame.minY,
            width: selection.rect.width,
            height: selection.rect.height
        )
        let scaleX = displayBounds.width / screen.frame.width
        let scaleY = displayBounds.height / screen.frame.height
        let captureRect = CGRect(
            x: displayBounds.minX + localRect.minX * scaleX,
            y: displayBounds.minY + (screen.frame.height - localRect.maxY) * scaleY,
            width: localRect.width * scaleX,
            height: localRect.height * scaleY
        )

        return try await withCheckedThrowingContinuation { continuation in
            SCScreenshotManager.captureImage(in: captureRect) { image, error in
                if let error {
                    continuation.resume(throwing: error)
                } else if let image {
                    continuation.resume(returning: image)
                } else {
                    continuation.resume(throwing: ScreenCaptureError.noImage)
                }
            }
        }
    }

    @available(macOS, introduced: 10.6, deprecated: 14.4, obsoleted: 15.0)
    private func captureWithCoreGraphics(selection: SelectionOverlayController.Selection) -> CGImage? {
        let screen = selection.screen
        guard let displayID = displayID(for: screen) else { return nil }
        guard let fullImage = CGDisplayCreateImage(displayID) else {
            return nil
        }

        let localRect = CGRect(
            x: selection.rect.minX - screen.frame.minX,
            y: selection.rect.minY - screen.frame.minY,
            width: selection.rect.width,
            height: selection.rect.height
        )

        let scaleX = CGFloat(fullImage.width) / screen.frame.width
        let scaleY = CGFloat(fullImage.height) / screen.frame.height
        let imageRect = CGRect(
            x: localRect.minX * scaleX,
            y: (screen.frame.height - localRect.maxY) * scaleY,
            width: localRect.width * scaleX,
            height: localRect.height * scaleY
        ).integral

        let imageBounds = CGRect(x: 0, y: 0, width: fullImage.width, height: fullImage.height)
        let clippedRect = imageRect.intersection(imageBounds)
        guard !clippedRect.isNull, clippedRect.width >= 1, clippedRect.height >= 1 else {
            return nil
        }
        return fullImage.cropping(to: clippedRect)
    }

    private func displayID(for screen: NSScreen) -> CGDirectDisplayID? {
        guard let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber else {
            return nil
        }
        return CGDirectDisplayID(number.uint32Value)
    }
}
