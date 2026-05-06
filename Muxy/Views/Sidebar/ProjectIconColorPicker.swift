import MuxyShared
import SwiftUI

extension ProjectIconColor.Swatch {
    var color: Color { Color(hex: hex) ?? .gray }
    var foreground: Color { prefersDarkForeground ? .black : .white }
}

extension ProjectIconColor {
    static func color(for identifier: String?) -> Color? {
        swatch(for: identifier)?.color
    }

    static func foreground(for identifier: String?) -> Color? {
        swatch(for: identifier)?.foreground
    }
}

struct ProjectIconColorPicker: View {
    var title: String = "Icon Color"
    let selectedID: String?
    let onSelect: (String?) -> Void

    private let columns = Array(repeating: GridItem(.fixed(24), spacing: UIMetrics.spacing4), count: 6)

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
            Text(title)
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)

            LazyVGrid(columns: columns, spacing: UIMetrics.spacing4) {
                ForEach(ProjectIconColor.palette) { swatch in
                    swatchButton(swatch)
                }
            }

            Divider()

            Button {
                onSelect(nil)
            } label: {
                HStack(spacing: UIMetrics.spacing3) {
                    Image(systemName: "arrow.uturn.backward")
                        .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                    Text("Reset to Default")
                        .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                }
                .foregroundStyle(MuxyTheme.fgMuted)
            }
            .buttonStyle(.plain)
            .disabled(selectedID == nil)
            .opacity(selectedID == nil ? 0.4 : 1)
        }
        .padding(UIMetrics.spacing6)
        .frame(width: UIMetrics.scaled(216))
    }

    private func swatchButton(_ swatch: ProjectIconColor.Swatch) -> some View {
        let isSelected = ProjectIconColor.swatch(for: selectedID)?.id == swatch.id
        return Button {
            onSelect(swatch.id)
        } label: {
            ZStack {
                Circle()
                    .fill(swatch.color)
                    .frame(width: UIMetrics.scaled(22), height: UIMetrics.scaled(22))
                if isSelected {
                    Circle()
                        .strokeBorder(swatch.foreground, lineWidth: 2)
                        .frame(width: UIMetrics.scaled(18), height: UIMetrics.scaled(18))
                }
            }
            .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(swatch.name)
        .accessibilityLabel(swatch.name)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }
}

extension Color {
    init?(hex: String) {
        guard let rgb = ProjectIconColor.rgb(fromHex: hex) else { return nil }
        self = Color(.sRGB, red: rgb.0, green: rgb.1, blue: rgb.2, opacity: 1)
    }
}
