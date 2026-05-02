import AppKit

enum SyntaxScope: Hashable {
    case keyword
    case storage
    case type
    case builtin
    case constant
    case string
    case stringEscape
    case number
    case comment
    case docComment
    case function
    case variable
    case attribute
    case preprocessor
    case op
    case punctuation
    case tag
    case attributeName
    case attributeValue
    case regex
    case heading
    case link
    case emphasis
}

@MainActor
enum SyntaxTheme {
    private static var cachedColors: [SyntaxScope: NSColor] = [:]
    private static var cachedDefaultForeground: NSColor?
    private static var cachedVersion = -1

    static func color(for scope: SyntaxScope) -> NSColor {
        ensureFresh()
        if let cached = cachedColors[scope] {
            return cached
        }
        let color = resolve(scope: scope)
        cachedColors[scope] = color
        return color
    }

    static var defaultForeground: NSColor {
        ensureFresh()
        if let cached = cachedDefaultForeground {
            return cached
        }
        let color = EditorThemePalette.active.foreground
        cachedDefaultForeground = color
        return color
    }

    private static func ensureFresh() {
        let version = GhosttyService.shared.configVersion
        guard version != cachedVersion else { return }
        cachedColors.removeAll(keepingCapacity: true)
        cachedDefaultForeground = nil
        cachedVersion = version
    }

    private static func resolve(scope: SyntaxScope) -> NSColor {
        let palette = EditorThemePalette.active
        let fg = palette.foreground

        switch scope {
        case .keyword,
             .storage:
            return palette.paletteColor(at: 5) ?? fg
        case .type:
            return palette.paletteColor(at: 6) ?? fg
        case .builtin:
            return palette.paletteColor(at: 14) ?? palette.paletteColor(at: 6) ?? fg
        case .constant:
            return palette.paletteColor(at: 3) ?? fg
        case .string:
            return palette.paletteColor(at: 2) ?? fg
        case .stringEscape:
            return palette.paletteColor(at: 13) ?? palette.paletteColor(at: 5) ?? fg
        case .number:
            return palette.paletteColor(at: 3) ?? fg
        case .comment,
             .docComment:
            return palette.paletteColor(at: 8) ?? fg.withAlphaComponent(0.55)
        case .function:
            return palette.paletteColor(at: 4) ?? fg
        case .variable:
            return palette.paletteColor(at: 6) ?? fg
        case .attribute:
            return palette.paletteColor(at: 11) ?? palette.paletteColor(at: 3) ?? fg
        case .preprocessor:
            return palette.paletteColor(at: 13) ?? palette.paletteColor(at: 5) ?? fg
        case .op:
            return fg
        case .punctuation:
            return fg.withAlphaComponent(0.75)
        case .tag:
            return palette.paletteColor(at: 1) ?? fg
        case .attributeName:
            return palette.paletteColor(at: 3) ?? fg
        case .attributeValue:
            return palette.paletteColor(at: 2) ?? fg
        case .regex:
            return palette.paletteColor(at: 1) ?? fg
        case .heading:
            return palette.paletteColor(at: 4) ?? fg
        case .link:
            return palette.paletteColor(at: 6) ?? fg
        case .emphasis:
            return palette.paletteColor(at: 3) ?? fg
        }
    }
}
