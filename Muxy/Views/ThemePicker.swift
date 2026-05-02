import SwiftUI

enum ThemePickerMode {
    case light
    case dark
    case currentAppearance

    static var sidebar: ThemePickerMode { .currentAppearance }
}

struct ThemePicker: View {
    var mode: ThemePickerMode = .currentAppearance
    @Environment(ThemeService.self) private var themeService
    @State private var themes: [ThemePreview] = []
    @State private var currentTheme: String?

    var body: some View {
        SearchableListPicker(
            items: themes,
            filterKey: \.name,
            placeholder: "Search themes",
            emptyLabel: "No themes found",
            onSelect: { selectTheme($0) },
            row: { theme, isHighlighted in
                ThemeRow(
                    theme: theme,
                    isActive: theme.name == currentTheme,
                    isHighlighted: isHighlighted
                )
            }
        )
        .frame(width: 280, height: 400)
        .task {
            themes = await themeService.loadThemes()
            currentTheme = currentName()
        }
    }

    private func currentName() -> String? {
        isDarkMode() ? themeService.currentDarkThemeName() : themeService.currentLightThemeName()
    }

    private func isDarkMode() -> Bool {
        switch mode {
        case .light: false
        case .dark: true
        case .currentAppearance: themeService.activeAppearance() == .dark
        }
    }

    private func selectTheme(_ theme: ThemePreview) {
        currentTheme = theme.name
        if isDarkMode() {
            themeService.applyDarkTheme(theme.name)
        } else {
            themeService.applyLightTheme(theme.name)
        }
    }
}

private struct ThemeRow: View {
    let theme: ThemePreview
    let isActive: Bool
    let isHighlighted: Bool
    @State private var hovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 4) {
                Text(theme.name)
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)

                Spacer(minLength: 0)

                if isActive {
                    Image(systemName: "checkmark")
                        .font(.system(size: 9, weight: .bold))
                        .foregroundStyle(Color.accentColor)
                }
            }

            HStack(spacing: 0) {
                Rectangle()
                    .fill(Color(nsColor: theme.background))
                    .overlay(
                        Text("Ab")
                            .font(.system(size: 9, weight: .medium, design: .monospaced))
                            .foregroundStyle(Color(nsColor: theme.foreground))
                    )
                    .frame(width: 24)

                ForEach(Array(theme.palette.enumerated()), id: \.offset) { _, color in
                    Rectangle().fill(Color(nsColor: color))
                }
            }
            .frame(height: 14)
            .clipShape(RoundedRectangle(cornerRadius: 3))
            .overlay(
                RoundedRectangle(cornerRadius: 3)
                    .strokeBorder(MuxyTheme.border, lineWidth: 0.5)
            )
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(isHighlighted ? MuxyTheme.surface : (hovered ? MuxyTheme.hover : .clear))
        .onHover { hovered = $0 }
    }
}
