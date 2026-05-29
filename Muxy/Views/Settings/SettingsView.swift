import AppKit
import SwiftUI

struct SettingsView: View {
    @State private var selectedRoute: SettingsRoute = .builtin(.general)
    @State private var searchText = ""
    @Environment(ExtensionStore.self) private var extensionStore

    private var visibleCategories: [SettingsCategory] {
        SettingsCatalog.categories.filter { SettingsCatalog.categoryMatches($0, query: searchText) }
    }

    private var visibleExtensionRoutes: [(extensionID: String, displayName: String)] {
        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return extensionStore.statuses
            .filter { $0.isEnabled && !$0.muxyExtension.manifest.settings.isEmpty }
            .filter { status in
                guard !query.isEmpty else { return true }
                let displayName = status.muxyExtension.displayName.lowercased()
                if displayName.contains(query) { return true }
                return status.muxyExtension.manifest.settings.contains { entry in
                    entry.key.lowercased().contains(query)
                        || entry.title.lowercased().contains(query)
                        || (entry.description?.lowercased().contains(query) ?? false)
                }
            }
            .map { ($0.id, $0.muxyExtension.displayName) }
    }

    var body: some View {
        VStack(spacing: 0) {
            SettingsHeader(searchText: $searchText)
            SettingsDivider()
            HStack(spacing: 0) {
                SettingsSidebar(
                    categories: visibleCategories,
                    extensionRoutes: visibleExtensionRoutes,
                    selectedRoute: $selectedRoute,
                    searchText: searchText
                )
                Rectangle()
                    .fill(SettingsStyle.border)
                    .frame(width: 1)
                settingsContent
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .environment(\.settingsSearchQuery, searchText)
                    .environment(\.settingsCategory, selectedBuiltinCategory)
            }
        }
        .frame(minWidth: 860, minHeight: 620)
        .background(SettingsStyle.background)
        .foregroundStyle(SettingsStyle.foreground)
        .tint(SettingsStyle.accent)
        .preferredColorScheme(MuxyTheme.colorScheme)
        .resetsSettingsFocusOnOutsideClick()
        .onChange(of: searchText) { _, _ in
            guard !isRouteVisible(selectedRoute) else { return }
            if let first = visibleCategories.first {
                selectedRoute = .builtin(first)
            } else if let ext = visibleExtensionRoutes.first {
                selectedRoute = .ext(ext.extensionID)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .focusProjectPickerDefaultLocation)) { _ in
            searchText = ""
            selectedRoute = .builtin(.projects)
        }
    }

    private var selectedBuiltinCategory: SettingsCategory? {
        if case let .builtin(category) = selectedRoute { return category }
        return nil
    }

    private func isRouteVisible(_ route: SettingsRoute) -> Bool {
        switch route {
        case let .builtin(category): visibleCategories.contains(category)
        case let .ext(extensionID): visibleExtensionRoutes.contains { $0.extensionID == extensionID }
        }
    }

    @ViewBuilder
    private var settingsContent: some View {
        switch selectedRoute {
        case let .builtin(category):
            builtinContent(for: category)
        case let .ext(extensionID):
            ExtensionCustomSettingsView(extensionID: extensionID)
        }
    }

    @ViewBuilder
    private func builtinContent(for category: SettingsCategory) -> some View {
        switch category {
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
    let extensionRoutes: [(extensionID: String, displayName: String)]
    @Binding var selectedRoute: SettingsRoute
    let searchText: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if categories.isEmpty, extensionRoutes.isEmpty {
                Text("No settings found")
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .padding(SettingsMetrics.horizontalPadding)
            } else {
                ForEach(categories) { category in
                    sidebarRow(
                        route: .builtin(category),
                        symbol: category.symbolName,
                        title: category.title,
                        matchCountText: searchText.isEmpty ? nil : matchCountText(for: category)
                    )
                }
                ForEach(extensionRoutes, id: \.extensionID) { route in
                    sidebarRow(
                        route: .ext(route.extensionID),
                        symbol: "puzzlepiece.extension",
                        title: route.displayName,
                        matchCountText: nil
                    )
                }
            }
            Spacer()
        }
        .padding(10)
        .frame(width: 210)
        .background(SettingsStyle.sidebarBackground)
    }

    @ViewBuilder
    private func sidebarRow(
        route: SettingsRoute,
        symbol: String,
        title: String,
        matchCountText: String?
    ) -> some View {
        let isSelected = selectedRoute == route
        Button {
            selectedRoute = route
        } label: {
            HStack(spacing: 8) {
                Image(systemName: symbol)
                    .font(.system(size: 12, weight: .medium))
                    .frame(width: 16)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.system(size: 12, weight: isSelected ? .semibold : .regular))
                        .foregroundStyle(SettingsStyle.foreground)
                        .lineLimit(1)
                        .truncationMode(.tail)
                    if let matchCountText {
                        Text(matchCountText)
                            .font(.system(size: 10))
                            .foregroundStyle(SettingsStyle.mutedForeground)
                    }
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(Rectangle())
            .background(
                isSelected ? SettingsStyle.accentSoft : .clear,
                in: RoundedRectangle(cornerRadius: 8)
            )
            .foregroundStyle(isSelected ? SettingsStyle.accent : SettingsStyle.mutedForeground)
        }
        .buttonStyle(.plain)
    }

    private func matchCountText(for category: SettingsCategory) -> String {
        let count = SettingsCatalog.matchingItems(query: searchText).count(where: { $0.category == category })
        guard count != 1 else { return "1 match" }
        return "\(count) matches"
    }
}
