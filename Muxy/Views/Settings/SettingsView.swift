import AppKit
import SwiftUI

struct SettingsView: View {
    @State private var selectedCategory: SettingsCategory = .general
    @State private var searchText = ""
    @State private var themeRefreshID = 0

    private var visibleCategories: [SettingsCategory] {
        SettingsCatalog.categories.filter { SettingsCatalog.categoryMatches($0, query: searchText) }
    }

    var body: some View {
        VStack(spacing: 0) {
            SettingsHeader(searchText: $searchText)
            SettingsDivider()
            HStack(spacing: 0) {
                SettingsSidebar(
                    categories: visibleCategories,
                    selectedCategory: $selectedCategory,
                    searchText: searchText
                )
                Rectangle()
                    .fill(SettingsStyle.border)
                    .frame(width: 1)
                settingsContent
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .environment(\.settingsSearchQuery, searchText)
                    .environment(\.settingsCategory, selectedCategory)
            }
        }
        .frame(minWidth: 860, minHeight: 620)
        .background(SettingsStyle.background)
        .foregroundStyle(SettingsStyle.foreground)
        .tint(SettingsStyle.accent)
        .preferredColorScheme(MuxyTheme.colorScheme)
        .resetsSettingsFocusOnOutsideClick()
        .onChange(of: searchText) { _, _ in
            guard !visibleCategories.contains(selectedCategory), let first = visibleCategories.first else { return }
            selectedCategory = first
        }
        .onReceive(NotificationCenter.default.publisher(for: .focusProjectPickerDefaultLocation)) { _ in
            searchText = ""
            selectedCategory = .projects
        }
        .onReceive(NotificationCenter.default.publisher(for: .themeDidChange)) { _ in
            themeRefreshID += 1
        }
    }

    @ViewBuilder
    private var settingsContent: some View {
        switch selectedCategory {
        case .general:
            GeneralSettingsView()
        case .projects:
            ProjectsSettingsView()
        case .appearance:
            InterfaceSettingsView()
        case .terminal:
            TerminalSettingsView()
        case .editor:
            EditorSettingsView()
        case .shortcuts:
            KeyboardShortcutsSettingsView()
        case .voice:
            RecordingSettingsView()
        case .notifications:
            NotificationSettingsView()
        case .mobile:
            MobileSettingsView()
        case .ai:
            AIAssistantSettingsView()
        case .extensions:
            ExtensionsSettingsView()
        case .json:
            SettingsJSONEditorView()
        }
    }
}

private struct SettingsHeader: View {
    @Binding var searchText: String

    var body: some View {
        HStack(spacing: 0) {
            HStack(spacing: 12) {
                Image(systemName: "slider.horizontal.3")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(SettingsStyle.mutedForeground)

                Text("Settings")
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(SettingsStyle.foreground)
            }
            .padding(.horizontal, 16)
            .frame(width: 210, alignment: .leading)

            Rectangle()
                .fill(SettingsStyle.border)
                .frame(width: 1, height: 56)

            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 12))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                TextField("Search settings", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(.system(size: 13))
                    .foregroundStyle(SettingsStyle.foreground)
                if !searchText.isEmpty {
                    Button {
                        searchText = ""
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .font(.system(size: 12))
                            .foregroundStyle(SettingsStyle.mutedForeground)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 9)
            .padding(.vertical, 7)
            .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 8))
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(SettingsStyle.accent.opacity(searchText.isEmpty ? 0 : 0.55), lineWidth: 1)
            )
            .frame(maxWidth: .infinity)
            .padding(.leading, 8)
            .padding(.trailing, 10)

            Button {
                NSApp.keyWindow?.close()
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .frame(width: 28, height: 28)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Close Settings")
            .padding(.trailing, 12)
        }
        .padding(.vertical, 12)
        .frame(height: 56)
        .background(SettingsStyle.background)
    }
}

private struct SettingsSidebar: View {
    let categories: [SettingsCategory]
    @Binding var selectedCategory: SettingsCategory
    let searchText: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if categories.isEmpty {
                Text("No settings found")
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .padding(SettingsMetrics.horizontalPadding)
            } else {
                ForEach(categories) { category in
                    Button {
                        selectedCategory = category
                    } label: {
                        HStack(spacing: 8) {
                            Image(systemName: category.symbolName)
                                .font(.system(size: 12, weight: .medium))
                                .frame(width: 16)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(category.title)
                                    .font(.system(size: 12, weight: selectedCategory == category ? .semibold : .regular))
                                    .foregroundStyle(SettingsStyle.foreground)
                                if !searchText.isEmpty {
                                    Text(matchCountText(for: category))
                                        .font(.system(size: 10))
                                        .foregroundStyle(SettingsStyle.mutedForeground)
                                }
                            }
                            Spacer(minLength: 0)
                            if let badge = category.developmentBadge {
                                SettingsDevelopmentBadge(text: badge)
                            }
                        }
                        .padding(.horizontal, 10)
                        .padding(.vertical, 7)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .contentShape(Rectangle())
                        .background(
                            selectedCategory == category ? SettingsStyle.accentSoft : .clear,
                            in: RoundedRectangle(cornerRadius: 8)
                        )
                        .foregroundStyle(selectedCategory == category ? SettingsStyle.accent : SettingsStyle.mutedForeground)
                    }
                    .buttonStyle(.plain)
                }
            }
            Spacer()
        }
        .padding(10)
        .frame(width: 210)
        .background(SettingsStyle.sidebarBackground)
    }

    private func matchCountText(for category: SettingsCategory) -> String {
        let count = SettingsCatalog.matchingItems(query: searchText).count(where: { $0.category == category })
        guard count != 1 else { return "1 match" }
        return "\(count) matches"
    }
}
