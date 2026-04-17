import SwiftUI

enum ProjectIconColor {
    struct Swatch: Identifiable, Hashable {
        let id: String
        let name: String
        let hex: String

        var color: Color { Color(hex: hex) ?? .gray }
    }

    static let palette: [Swatch] = [
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

    static func color(for hex: String?) -> Color? {
        guard let hex else { return nil }
        return Color(hex: hex)
    }
}

struct ProjectIconColorPicker: View {
    let selectedHex: String?
    let onSelect: (String?) -> Void

    private let columns = Array(repeating: GridItem(.fixed(24), spacing: 8), count: 6)

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Icon Color")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)

            LazyVGrid(columns: columns, spacing: 8) {
                ForEach(ProjectIconColor.palette) { swatch in
                    swatchButton(swatch)
                }
            }

            Divider()

            Button {
                onSelect(nil)
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "arrow.uturn.backward")
                        .font(.system(size: 10, weight: .medium))
                    Text("Reset to Default")
                        .font(.system(size: 11, weight: .medium))
                }
                .foregroundStyle(MuxyTheme.fgMuted)
            }
            .buttonStyle(.plain)
            .disabled(selectedHex == nil)
            .opacity(selectedHex == nil ? 0.4 : 1)
        }
        .padding(12)
        .frame(width: 216)
    }

    private func swatchButton(_ swatch: ProjectIconColor.Swatch) -> some View {
        let isSelected = matches(swatch: swatch)
        return Button {
            onSelect(swatch.hex)
        } label: {
            ZStack {
                Circle()
                    .fill(swatch.color)
                    .frame(width: 22, height: 22)
                if isSelected {
                    Circle()
                        .strokeBorder(MuxyTheme.fg, lineWidth: 2)
                        .frame(width: 24, height: 24)
                }
            }
            .frame(width: 24, height: 24)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(swatch.name)
        .accessibilityLabel(swatch.name)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }

    private func matches(swatch: ProjectIconColor.Swatch) -> Bool {
        guard let selectedHex else { return false }
        return swatch.hex.caseInsensitiveCompare(selectedHex) == .orderedSame
    }
}

extension Color {
    init?(hex: String) {
        var normalized = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if normalized.hasPrefix("#") {
            normalized.removeFirst()
        }
        guard normalized.count == 6,
              let value = UInt32(normalized, radix: 16)
        else { return nil }

        let red = Double((value >> 16) & 0xFF) / 255.0
        let green = Double((value >> 8) & 0xFF) / 255.0
        let blue = Double(value & 0xFF) / 255.0
        self = Color(.sRGB, red: red, green: green, blue: blue, opacity: 1)
    }
}
