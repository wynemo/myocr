import CoreGraphics
import Vision

struct OCRLanguage: Equatable {
    let id: String
    let title: String
    let recognitionCodes: [String]
}

enum LanguageCatalog {
    static let all: [OCRLanguage] = [
        OCRLanguage(id: "auto", title: "自动检测（推荐）", recognitionCodes: [
            "zh-Hans", "zh-Hant", "en-US", "ja-JP", "ko-KR",
            "fr-FR", "de-DE", "es-ES", "it-IT", "pt-BR",
            "ru-RU", "uk-UA", "ar-SA", "th-TH", "vi-VT", "id-ID",
            "nl-NL", "tr-TR", "cs-CZ", "da-DK", "sv-SE", "ro-RO",
            "ms-MY", "nb-NO", "pl-PL"
        ]),
        OCRLanguage(id: "zh-Hans", title: "中文（简体）", recognitionCodes: ["zh-Hans"]),
        OCRLanguage(id: "zh-Hant", title: "中文（繁体）", recognitionCodes: ["zh-Hant"]),
        OCRLanguage(id: "en-US", title: "English", recognitionCodes: ["en-US"]),
        OCRLanguage(id: "ja-JP", title: "日本語", recognitionCodes: ["ja-JP"]),
        OCRLanguage(id: "ko-KR", title: "한국어", recognitionCodes: ["ko-KR"]),
        OCRLanguage(id: "fr-FR", title: "Français", recognitionCodes: ["fr-FR"]),
        OCRLanguage(id: "de-DE", title: "Deutsch", recognitionCodes: ["de-DE"]),
        OCRLanguage(id: "es-ES", title: "Español", recognitionCodes: ["es-ES"]),
        OCRLanguage(id: "it-IT", title: "Italiano", recognitionCodes: ["it-IT"]),
        OCRLanguage(id: "pt-BR", title: "Português", recognitionCodes: ["pt-BR"]),
        OCRLanguage(id: "ru-RU", title: "Русский", recognitionCodes: ["ru-RU"]),
        OCRLanguage(id: "uk-UA", title: "Українська", recognitionCodes: ["uk-UA"]),
        OCRLanguage(id: "ar-SA", title: "العربية", recognitionCodes: ["ar-SA"]),
        OCRLanguage(id: "th-TH", title: "ไทย", recognitionCodes: ["th-TH"]),
        OCRLanguage(id: "vi-VT", title: "Tiếng Việt", recognitionCodes: ["vi-VT", "vi-VN"]),
        OCRLanguage(id: "id-ID", title: "Bahasa Indonesia", recognitionCodes: ["id-ID"]),
        OCRLanguage(id: "nl-NL", title: "Nederlands", recognitionCodes: ["nl-NL"]),
        OCRLanguage(id: "tr-TR", title: "Türkçe", recognitionCodes: ["tr-TR"]),
        OCRLanguage(id: "cs-CZ", title: "Čeština", recognitionCodes: ["cs-CZ"]),
        OCRLanguage(id: "da-DK", title: "Dansk", recognitionCodes: ["da-DK"]),
        OCRLanguage(id: "sv-SE", title: "Svenska", recognitionCodes: ["sv-SE"]),
        OCRLanguage(id: "ro-RO", title: "Română", recognitionCodes: ["ro-RO"]),
        OCRLanguage(id: "ms-MY", title: "Bahasa Melayu", recognitionCodes: ["ms-MY"]),
        OCRLanguage(id: "nb-NO", title: "Norsk", recognitionCodes: ["nb-NO"]),
        OCRLanguage(id: "pl-PL", title: "Polski", recognitionCodes: ["pl-PL"])
    ]

    static func language(withID id: String) -> OCRLanguage? {
        all.first(where: { $0.id == id })
    }
}

final class SettingsStore {
    static let shared = SettingsStore()

    private let defaults = UserDefaults.standard
    private let languageKey = "ocr.language"

    var languageIdentifier: String {
        get {
            let value = defaults.string(forKey: languageKey) ?? "auto"
            return LanguageCatalog.language(withID: value) == nil ? "auto" : value
        }
        set {
            defaults.set(newValue, forKey: languageKey)
        }
    }
}

enum OCRServiceError: LocalizedError {
    case noText

    var errorDescription: String? {
        switch self {
        case .noText:
            return "没有识别到文字。"
        }
    }
}

final class OCRService {
    func recognize(_ image: CGImage, languageID: String) async throws -> String {
        try await Task.detached(priority: .userInitiated) {
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            request.usesLanguageCorrection = true
            request.automaticallyDetectsLanguage = languageID == "auto"

            if let language = LanguageCatalog.language(withID: languageID) {
                let supportedLanguages = (try? request.supportedRecognitionLanguages()) ?? []
                let resolvedCodes = Self.resolve(
                    requested: language.recognitionCodes,
                    supported: supportedLanguages
                )
                if !resolvedCodes.isEmpty {
                    request.recognitionLanguages = resolvedCodes
                }
            }

            let handler = VNImageRequestHandler(cgImage: image, options: [:])
            try handler.perform([request])

            let observations = request.results ?? []
            guard !observations.isEmpty else { throw OCRServiceError.noText }
            return Self.format(observations)
        }.value
    }

    private static func resolve(requested: [String], supported: [String]) -> [String] {
        guard !supported.isEmpty else { return requested }

        return requested.compactMap { requestedCode in
            supported.first { supportedCode in
                supportedCode.caseInsensitiveCompare(requestedCode) == .orderedSame
                    || supportedCode.lowercased().hasPrefix(requestedCode.lowercased() + "-")
                    || requestedCode.lowercased().hasPrefix(supportedCode.lowercased() + "-")
            }
        }
    }

    private static func format(_ observations: [VNRecognizedTextObservation]) -> String {
        struct Line {
            let text: String
            let box: CGRect
        }

        let lines = observations.compactMap { observation -> Line? in
            guard let text = observation.topCandidates(1).first?.string else { return nil }
            let cleaned = text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !cleaned.isEmpty else { return nil }
            return Line(text: cleaned, box: observation.boundingBox)
        }

        guard !lines.isEmpty else { return "" }

        // Vision usually returns reading order, but grouping by vertical position keeps
        // columns and multi-fragment lines stable for screenshots containing tables.
        let sorted = lines.sorted {
            if abs($0.box.midY - $1.box.midY) > max($0.box.height, $1.box.height) * 0.45 {
                return $0.box.midY > $1.box.midY
            }
            return $0.box.minX < $1.box.minX
        }

        var rows: [[Line]] = []
        for line in sorted {
            if let rowIndex = rows.firstIndex(where: { row in
                guard let first = row.first else { return false }
                let threshold = max(first.box.height, line.box.height) * 0.55
                return abs(first.box.midY - line.box.midY) <= threshold
            }) {
                rows[rowIndex].append(line)
            } else {
                rows.append([line])
            }
        }

        return rows
            .sorted { ($0.first?.box.midY ?? 0) > ($1.first?.box.midY ?? 0) }
            .map { row in
                row.sorted { $0.box.minX < $1.box.minX }
                    .map(\.text)
                    .reduce(into: "") { result, fragment in
                        if result.isEmpty || Self.shouldJoinWithoutSpace(result, fragment) {
                            result += fragment
                        } else {
                            result += " " + fragment
                        }
                    }
            }
            .joined(separator: "\n")
    }

    private static func shouldJoinWithoutSpace(_ left: String, _ right: String) -> Bool {
        guard let leftCharacter = left.last, let rightCharacter = right.first else { return true }
        return isCJK(leftCharacter) || isCJK(rightCharacter) || rightCharacter.isPunctuation
    }

    private static func isCJK(_ character: Character) -> Bool {
        character.unicodeScalars.contains { scalar in
            switch scalar.value {
            case 0x2E80...0x2FFF, 0x3040...0x30FF, 0x3400...0x4DBF,
                 0x4E00...0x9FFF, 0xAC00...0xD7AF, 0xF900...0xFAFF:
                return true
            default:
                return false
            }
        }
    }
}

enum TextPostProcessor {
    static func clean(_ text: String) -> String {
        text
            .replacingOccurrences(of: "\r\n", with: "\n")
            .replacingOccurrences(of: "\r", with: "\n")
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .joined(separator: "\n")
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
