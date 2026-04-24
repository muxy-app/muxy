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
        let color = GhosttyService.shared.foregroundColor
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
        let service = GhosttyService.shared
        let fg = service.foregroundColor

        switch scope {
        case .keyword,
             .storage:
            return service.paletteColor(at: 5) ?? fg
        case .type:
            return service.paletteColor(at: 6) ?? fg
        case .builtin:
            return service.paletteColor(at: 14) ?? service.paletteColor(at: 6) ?? fg
        case .constant:
            return service.paletteColor(at: 3) ?? fg
        case .string:
            return service.paletteColor(at: 2) ?? fg
        case .stringEscape:
            return service.paletteColor(at: 13) ?? service.paletteColor(at: 5) ?? fg
        case .number:
            return service.paletteColor(at: 3) ?? fg
        case .comment,
             .docComment:
            return service.paletteColor(at: 8) ?? fg.withAlphaComponent(0.55)
        case .function:
            return service.paletteColor(at: 4) ?? fg
        case .variable:
            return service.paletteColor(at: 6) ?? fg
        case .attribute:
            return service.paletteColor(at: 11) ?? service.paletteColor(at: 3) ?? fg
        case .preprocessor:
            return service.paletteColor(at: 13) ?? service.paletteColor(at: 5) ?? fg
        case .op:
            return fg
        case .punctuation:
            return fg.withAlphaComponent(0.75)
        case .tag:
            return service.paletteColor(at: 1) ?? fg
        case .attributeName:
            return service.paletteColor(at: 3) ?? fg
        case .attributeValue:
            return service.paletteColor(at: 2) ?? fg
        case .regex:
            return service.paletteColor(at: 1) ?? fg
        case .heading:
            return service.paletteColor(at: 4) ?? fg
        case .link:
            return service.paletteColor(at: 6) ?? fg
        case .emphasis:
            return service.paletteColor(at: 3) ?? fg
        }
    }
}
