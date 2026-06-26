import SwiftUI

struct OverviewTabsSection: View {
    let project: Project

    @Environment(AppState.self) private var appState
    @Environment(BrowserProfileStore.self) private var browserProfileStore
    @AppStorage(BrowserPreferences.enabledKey) private var browserEnabled = true

    private struct AreaTab: Identifiable {
        let areaID: UUID
        let tab: TerminalTab
        var id: UUID { tab.id }
    }

    private var areas: [TabArea] {
        appState.allAreas(for: project.id)
    }

    private var areaTabs: [AreaTab] {
        areas.flatMap { area in
            area.tabs.map { AreaTab(areaID: area.id, tab: $0) }
        }
    }

    private var activeTabID: UUID? {
        appState.activeTab(for: project.id)?.id
    }

    var body: some View {
        OverviewSection(
            title: "Tabs",
            storageKey: OverviewSidebarPreferences.tabsSectionExpandedKey,
            accessory: {
                HStack(spacing: UIMetrics.spacing1) {
                    OverviewActionButton(symbol: "plus", label: "New Terminal Tab") {
                        appState.createTab(projectID: project.id)
                    }
                    if browserEnabled {
                        OverviewActionButton(symbol: "globe", label: "New Browser Tab") {
                            appState.dispatch(.createBrowserTab(
                                projectID: project.id,
                                areaID: appState.focusedArea(for: project.id)?.id,
                                url: BrowserURL.homeURL,
                                profileID: browserProfileStore.defaultProfileID
                            ))
                        }
                    }
                }
            },
            content: { content }
        )
    }

    @ViewBuilder
    private var content: some View {
        if areaTabs.isEmpty {
            Text("No open tabs")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
        } else {
            VStack(spacing: UIMetrics.scaled(1)) {
                ForEach(areaTabs) { item in
                    OverviewRow(
                        title: item.tab.title,
                        isSelected: item.tab.id == activeTabID,
                        onTap: {
                            appState.dispatch(.selectTab(
                                projectID: project.id,
                                areaID: item.areaID,
                                tabID: item.tab.id
                            ))
                        },
                        leading: {
                            Image(systemName: icon(for: item.tab.kind))
                                .font(.system(size: UIMetrics.fontXS, weight: .medium))
                                .foregroundStyle(MuxyTheme.fgMuted)
                                .frame(width: UIMetrics.scaled(14))
                        }
                    )
                }
            }
        }
    }

    private func icon(for kind: TerminalTab.Kind) -> String {
        switch kind {
        case .terminal: "terminal"
        case .browser: "globe"
        case .extensionWebView: "puzzlepiece.extension"
        }
    }
}
