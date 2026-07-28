import CoreText
import Foundation
import GhosttyKit
import os

private let logger = Logger(subsystem: "app.muxy", category: "TerminalCJKFontConfig")

enum TerminalCJKFontConfig {
    private static let requiredChineseGlyphs = "中文简体繁體专业專業，。！？"
    private static let codepointRanges = [
        "U+3000-U+303F",
        "U+3400-U+4DBF",
        "U+4E00-U+9FFF",
        "U+F900-U+FAFF",
        "U+FF00-U+FFEF",
    ]

    static func load(into config: ghostty_config_t, userConfig: String) {
        guard let contents = configText(userConfig: userConfig) else { return }
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-cjk-font-\(UUID().uuidString).conf")
        defer { try? FileManager.default.removeItem(at: url) }

        do {
            try Data(contents.utf8).write(to: url, options: .atomic)
        } catch {
            logger.error("Failed to write CJK font config: \(error)")
            return
        }

        url.path.withCString { ghostty_config_load_file(config, $0) }
    }

    static func configText(userConfig: String) -> String? {
        guard let family = resolvedFontFamily(configuredFamilies: fontFamilies(in: userConfig)),
              !family.contains("\n"),
              !family.contains("\r")
        else { return nil }

        return "font-codepoint-map = \(codepointRanges.joined(separator: ","))=\(family)\n"
    }

    static func fontFamilies(in config: String) -> [String] {
        var families: [String] = []
        let content = config.first == "\u{FEFF}" ? String(config.dropFirst()) : config
        for line in content.components(separatedBy: .newlines) {
            guard let value = value(for: "font-family", in: line) else { continue }
            let family = unquoted(value)
            if family.isEmpty {
                families.removeAll(keepingCapacity: true)
            } else {
                families.append(family)
            }
        }
        return families
    }

    private static func resolvedFontFamily(configuredFamilies: [String]) -> String? {
        for family in configuredFamilies {
            let font = CTFontCreateWithName(family as CFString, 13, nil)
            guard supports(requiredChineseGlyphs, font: font) else { continue }
            return family
        }

        let baseFont = configuredFamilies.first.map { CTFontCreateWithName($0 as CFString, 13, nil) }
            ?? CTFontCreateWithName("Menlo" as CFString, 13, nil)
        let fallback = CTFontCreateForString(
            baseFont,
            requiredChineseGlyphs as CFString,
            CFRange(location: 0, length: requiredChineseGlyphs.utf16.count)
        )
        guard supports(requiredChineseGlyphs, font: fallback) else { return nil }
        return CTFontCopyFamilyName(fallback) as String
    }

    private static func supports(_ text: String, font: CTFont) -> Bool {
        let characterSet = CTFontCopyCharacterSet(font) as CharacterSet
        return text.unicodeScalars.allSatisfy(characterSet.contains)
    }

    private static func value(for key: String, in line: String) -> String? {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        guard trimmed.hasPrefix(key) else { return nil }
        let remainder = trimmed.dropFirst(key.count).trimmingCharacters(in: .whitespaces)
        guard remainder.first == "=" else { return nil }
        return remainder.dropFirst().trimmingCharacters(in: .whitespaces)
    }

    private static func unquoted(_ value: String) -> String {
        guard value.count >= 2, let first = value.first, let last = value.last,
              (first == "\"" && last == "\"") || (first == "'" && last == "'")
        else { return value }
        return String(value.dropFirst().dropLast())
    }
}
