import SwiftUI

struct ThemePicker: View {
    @Environment(ThemeService.self) private var themeService
    @State private var themes: [ThemePreview] = []
    @State private var searchText = ""
    @State private var currentTheme: String?
    @State private var highlightedIndex: Int?

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .font(.system(size: 12))
                TextField("Search themes", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fg)
                    .onSubmit { confirmSelection() }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)

            Divider().overlay(MuxyTheme.border)

            ScrollViewReader { proxy in
                ScrollView(.vertical, showsIndicators: false) {
                    LazyVStack(spacing: 0) {
                        ForEach(Array(filteredThemes.enumerated()), id: \.element.id) { index, theme in
                            ThemeRow(
                                theme: theme,
                                isActive: theme.name == currentTheme,
                                isHighlighted: index == highlightedIndex,
                                onSelect: { selectTheme(theme) }
                            )
                            .id(theme.id)
                        }
                    }
                }
                .onChange(of: highlightedIndex) { _, newIndex in
                    guard let newIndex, newIndex < filteredThemes.count else { return }
                    proxy.scrollTo(filteredThemes[newIndex].id, anchor: nil)
                }
            }
        }
        .frame(width: 280, height: 400)
        .background(MuxyTheme.bg)
        .onKeyPress(.upArrow) { moveHighlight(-1); return .handled }
        .onKeyPress(.downArrow) { moveHighlight(1); return .handled }
        .onKeyPress(.return) { confirmSelection(); return .handled }
        .onChange(of: searchText) { highlightedIndex = filteredThemes.isEmpty ? nil : 0 }
        .task {
            themes = await themeService.loadThemes()
            currentTheme = themeService.currentThemeName()
        }
    }

    private var filteredThemes: [ThemePreview] {
        guard !searchText.isEmpty else { return themes }
        return themes.filter { $0.name.localizedCaseInsensitiveContains(searchText) }
    }

    private func moveHighlight(_ delta: Int) {
        let list = filteredThemes
        guard !list.isEmpty else { return }
        guard let current = highlightedIndex else {
            highlightedIndex = delta > 0 ? 0 : list.count - 1
            return
        }
        highlightedIndex = max(0, min(list.count - 1, current + delta))
    }

    private func confirmSelection() {
        let list = filteredThemes
        guard let index = highlightedIndex, index < list.count else { return }
        selectTheme(list[index])
    }

    private func selectTheme(_ theme: ThemePreview) {
        currentTheme = theme.name
        themeService.applyTheme(theme.name)
    }
}

private struct ThemeRow: View {
    let theme: ThemePreview
    let isActive: Bool
    let isHighlighted: Bool
    let onSelect: () -> Void
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
        .contentShape(Rectangle())
        .onTapGesture(perform: onSelect)
        .onHover { hovered = $0 }
    }
}
