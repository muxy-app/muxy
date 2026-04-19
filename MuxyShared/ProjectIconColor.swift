import Foundation

public enum ProjectIconColor {
    public struct Swatch: Identifiable, Hashable, Sendable {
        public let id: String
        public let name: String
        public let hex: String

        public init(id: String, name: String, hex: String) {
            self.id = id
            self.name = name
            self.hex = hex
        }

        public var prefersDarkForeground: Bool {
            guard let rgb = ProjectIconColor.rgb(fromHex: hex) else { return false }
            let luminance = 0.2126 * rgb.0 + 0.7152 * rgb.1 + 0.0722 * rgb.2
            return luminance > 0.6
        }
    }

    public static let palette: [Swatch] = [
        Swatch(id: "red", name: "Red", hex: "#E5484D"),
        Swatch(id: "orange", name: "Orange", hex: "#F76B15"),
        Swatch(id: "amber", name: "Amber", hex: "#F5A623"),
        Swatch(id: "yellow", name: "Yellow", hex: "#EBCB00"),
        Swatch(id: "lime", name: "Lime", hex: "#9BCD1E"),
        Swatch(id: "green", name: "Green", hex: "#30A46C"),
        Swatch(id: "teal", name: "Teal", hex: "#12A594"),
        Swatch(id: "cyan", name: "Cyan", hex: "#05A2C2"),
        Swatch(id: "blue", name: "Blue", hex: "#3E63DD"),
        Swatch(id: "indigo", name: "Indigo", hex: "#5B5BD6"),
        Swatch(id: "violet", name: "Violet", hex: "#8E4EC6"),
        Swatch(id: "pink", name: "Pink", hex: "#D6409F"),
    ]

    private static let byID: [String: Swatch] = Dictionary(
        uniqueKeysWithValues: palette.map { ($0.id, $0) }
    )

    public static func swatch(for identifier: String?) -> Swatch? {
        guard let identifier else { return nil }
        if let direct = byID[identifier] { return direct }
        return palette.first { $0.hex.caseInsensitiveCompare(identifier) == .orderedSame }
    }

    public static func rgb(fromHex hex: String) -> (Double, Double, Double)? {
        var normalized = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if normalized.hasPrefix("#") { normalized.removeFirst() }
        guard normalized.count == 6,
              let value = UInt32(normalized, radix: 16)
        else { return nil }
        let red = Double((value >> 16) & 0xFF) / 255.0
        let green = Double((value >> 8) & 0xFF) / 255.0
        let blue = Double(value & 0xFF) / 255.0
        return (red, green, blue)
    }
}
