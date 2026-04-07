import AppKit
import SwiftUI

struct DiffSectionDivider: View {
    let text: String

    var body: some View {
        HStack(spacing: 0) {
            Text(text)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgDim)
                .lineLimit(1)
                .truncationMode(.tail)
                .padding(.leading, 10)
            Spacer(minLength: 8)
        }
        .frame(height: 28)
        .frame(maxWidth: .infinity)
        .background(MuxyTheme.bg)
        .overlay(alignment: .top) {
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
        }
        .overlay(alignment: .bottom) {
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
        }
    }
}

func hunkLabel(_ raw: String) -> String {
    guard raw.count > 2,
          let closingRange = raw.range(of: "@@", range: raw.index(raw.startIndex, offsetBy: 2) ..< raw.endIndex)
    else { return raw }
    let after = raw[closingRange.upperBound...].trimmingCharacters(in: .whitespaces)
    return after.isEmpty ? raw : after
}

func lineNumberWidth(for maxLineNumber: Int) -> CGFloat {
    let digitCount = max(String(maxLineNumber).count, 1)
    return CGFloat(digitCount) * 8 + 12
}

func maxLineNumber(in rows: [DiffDisplayRow]) -> Int {
    rows.reduce(0) { result, row in
        max(result, row.oldLineNumber ?? 0, row.newLineNumber ?? 0)
    }
}

enum DiffBackgroundSide {
    case left
    case right
    case both
}

@MainActor
final class DiffHighlightCache {
    static let shared = DiffHighlightCache()

    struct Rule {
        let regex: NSRegularExpression
        let color: @MainActor () -> NSColor
    }

    let rules: [Rule]

    private init() {
        rules = Self.buildRules()
    }

    func invalidate() {}

    private struct RuleDefinition {
        let pattern: String
        let color: @MainActor () -> NSColor
        let options: NSRegularExpression.Options
    }

    private static func buildRules() -> [Rule] {
        var result: [Rule] = []

        let definitions: [RuleDefinition] = [
            RuleDefinition(pattern: #"'(?:\\.|[^'\\])*'"#, color: { MuxyTheme.nsDiffString }, options: []),
            RuleDefinition(pattern: #""(?:\\.|[^"\\])*""#, color: { MuxyTheme.nsDiffString }, options: []),
            RuleDefinition(pattern: #"`(?:\\.|[^`\\])*`"#, color: { MuxyTheme.nsDiffString }, options: []),
            RuleDefinition(pattern: #"\b\d+(?:\.\d+)?\b"#, color: { MuxyTheme.nsDiffNumber }, options: []),
            RuleDefinition(pattern: #"//.*$"#, color: { MuxyTheme.nsDiffComment }, options: [.anchorsMatchLines]),
        ]

        for definition in definitions {
            guard let regex = try? NSRegularExpression(pattern: definition.pattern, options: definition.options)
            else { continue }
            result.append(Rule(regex: regex, color: definition.color))
        }

        return result
    }
}
