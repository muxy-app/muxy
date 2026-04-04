import AppKit
import SwiftUI

struct DiffLineRow<Content: View>: View {
    let filePath: String
    let lineNumber: Int?
    @ViewBuilder let content: Content
    @State private var hovered = false

    var body: some View {
        content
            .overlay(alignment: .leading) {
                if hovered, let lineNumber {
                    Menu {
                        Button("Copy Reference") {
                            NSPasteboard.general.clearContents()
                            NSPasteboard.general.setString("\(filePath):\(lineNumber)", forType: .string)
                        }
                    } label: {
                        Image(systemName: "ellipsis")
                            .font(.system(size: 9, weight: .bold))
                            .foregroundStyle(MuxyTheme.fgMuted)
                            .frame(width: 20, height: 20)
                            .background(
                                RoundedRectangle(cornerRadius: 4)
                                    .fill(MuxyTheme.surface)
                            )
                    }
                    .menuStyle(.borderlessButton)
                    .menuIndicator(.hidden)
                    .frame(width: 20)
                    .padding(.leading, 2)
                }
            }
            .onHover { hovered = $0 }
    }
}

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

enum DiffBackgroundSide {
    case left
    case right
    case both
}

@MainActor
func rowBackground(_ kind: DiffDisplayRow.Kind, side: DiffBackgroundSide) -> Color {
    switch kind {
    case .addition:
        switch side {
        case .left:
            .clear
        case .right,
             .both:
            MuxyTheme.diffAddBg
        }
    case .deletion:
        switch side {
        case .left,
             .both:
            MuxyTheme.diffRemoveBg
        case .right:
            .clear
        }
    case .hunk:
        MuxyTheme.diffHunkBg
    case .collapsed:
        MuxyTheme.bg
    case .context:
        .clear
    }
}

struct CodeHighlightedText: View {
    enum ChangeKind {
        case context
        case addition
        case deletion
    }

    let text: String
    let kind: ChangeKind

    var body: some View {
        Text(DiffHighlightCache.shared.highlighted(text, kind: kind))
            .font(.system(size: 12, design: .monospaced))
            .lineLimit(1)
            .truncationMode(.tail)
            .textSelection(.enabled)
    }
}

@MainActor
final class DiffHighlightCache {
    static let shared = DiffHighlightCache()

    private struct CacheKey: Hashable {
        let text: String
        let kind: CodeHighlightedText.ChangeKind
    }

    private var cache: [CacheKey: AttributedString] = [:]
    private var insertionOrder: [CacheKey] = []
    private let maxEntries = 2000

    struct Rule {
        let regex: NSRegularExpression
        let color: @MainActor () -> NSColor
    }

    let rules: [Rule]

    private init() {
        rules = Self.buildRules()
    }

    func highlighted(_ source: String, kind: CodeHighlightedText.ChangeKind) -> AttributedString {
        let key = CacheKey(text: source, kind: kind)
        if let cached = cache[key] {
            return cached
        }
        let result = computeHighlighted(source, kind: kind)
        if insertionOrder.count >= maxEntries {
            let evicted = insertionOrder.removeFirst()
            cache.removeValue(forKey: evicted)
        }
        cache[key] = result
        insertionOrder.append(key)
        return result
    }

    func invalidate() {
        cache.removeAll()
        insertionOrder.removeAll()
    }

    private func computeHighlighted(_ source: String, kind: CodeHighlightedText.ChangeKind) -> AttributedString {
        let fullRange = NSRange(location: 0, length: (source as NSString).length)

        let baseColor: NSColor = switch kind {
        case .addition: MuxyTheme.nsDiffAdd
        case .deletion: MuxyTheme.nsDiffRemove
        case .context: GhosttyService.shared.foregroundColor
        }

        let attributed = NSMutableAttributedString(
            string: source,
            attributes: [
                .foregroundColor: baseColor,
                .font: NSFont.monospacedSystemFont(ofSize: 12, weight: .regular),
            ]
        )

        for rule in rules {
            let matches = rule.regex.matches(in: source, range: fullRange)
            let color = rule.color()
            for match in matches {
                attributed.addAttribute(.foregroundColor, value: color, range: match.range)
            }
        }

        return AttributedString(attributed)
    }

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
