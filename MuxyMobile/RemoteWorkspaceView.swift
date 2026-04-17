import MuxyShared
import SwiftUI

struct WorkspaceContentWrapper: View {
    @Environment(ConnectionManager.self) private var connection

    private var activeProject: ProjectDTO? {
        guard let id = connection.activeProjectID else { return nil }
        return connection.projects.first { $0.id == id }
    }

    private var allTabs: [(area: TabAreaDTO, tab: TabDTO)] {
        guard let workspace = connection.workspace else { return [] }
        return collectAreas(from: workspace.root).flatMap { area in
            area.tabs.map { (area: area, tab: $0) }
        }
    }

    private var activeTab: (area: TabAreaDTO, tab: TabDTO)? {
        guard let workspace = connection.workspace else { return nil }
        let areas = collectAreas(from: workspace.root)
        let focusedArea = areas.first { $0.id == workspace.focusedAreaID } ?? areas.first
        guard let area = focusedArea,
              let tabID = area.activeTabID,
              let tab = area.tabs.first(where: { $0.id == tabID })
        else { return nil }
        return (area: area, tab: tab)
    }

    var body: some View {
        tabContentView
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .principal) {
                    Text(activeProject?.name ?? "")
                        .font(.headline)
                        .foregroundStyle(themeFg)
                }
                ToolbarItem(placement: .topBarTrailing) {
                    tabPicker
                }
            }
            .toolbarBackground(themeBg, for: .navigationBar)
            .toolbarBackground(.visible, for: .navigationBar)
            .toolbarColorScheme(preferredScheme, for: .navigationBar)
            .tint(themeFg)
            .preferredColorScheme(preferredScheme)
            .background(themeBg.ignoresSafeArea())
    }

    private var themeFg: Color {
        connection.terminalTheme?.fgColor ?? .primary
    }

    private var themeBg: Color {
        connection.terminalTheme?.bgColor ?? Color(.systemBackground)
    }

    private var preferredScheme: ColorScheme {
        (connection.terminalTheme?.isDark ?? true) ? .dark : .light
    }

    @ViewBuilder
    private var tabContentView: some View {
        if let active = activeTab {
            TabDetailView(area: active.area, tab: active.tab)
        } else {
            ContentUnavailableView(
                "No Tabs",
                systemImage: "rectangle.on.rectangle.slash",
                description: Text("Create a new tab to get started")
            )
        }
    }

    private var tabPicker: some View {
        Menu {
            ForEach(allTabs, id: \.tab.id) { entry in
                Button {
                    Task {
                        await connection.selectTab(
                            projectID: connection.activeProjectID!,
                            areaID: entry.area.id,
                            tabID: entry.tab.id
                        )
                    }
                } label: {
                    if entry.tab.id == activeTab?.tab.id {
                        Label(shortTitle(entry.tab.title), systemImage: "checkmark")
                    } else {
                        Text(shortTitle(entry.tab.title))
                    }
                }
            }

            Divider()

            Button {
                guard let projectID = connection.activeProjectID else { return }
                Task { await connection.createTab(projectID: projectID) }
            } label: {
                Label("New Terminal", systemImage: "plus")
            }
        } label: {
            Image(systemName: "rectangle.stack")
        }
    }

    private func shortTitle(_ title: String) -> String {
        if let lastComponent = title.components(separatedBy: "/").last(where: { !$0.isEmpty }) {
            return lastComponent
        }
        return title
    }

    private func collectAreas(from node: SplitNodeDTO) -> [TabAreaDTO] {
        switch node {
        case let .tabArea(area):
            [area]
        case let .split(branch):
            collectAreas(from: branch.first) + collectAreas(from: branch.second)
        }
    }

    private func iconForKind(_ kind: TabKindDTO) -> String {
        switch kind {
        case .terminal: "terminal"
        case .vcs: "arrow.triangle.branch"
        case .editor: "doc.text"
        }
    }
}

struct TabDetailView: View {
    let area: TabAreaDTO
    let tab: TabDTO
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        VStack(spacing: 0) {
            switch tab.kind {
            case .terminal:
                terminalPlaceholder
            case .vcs:
                vcsPlaceholder
            case .editor:
                editorPlaceholder
            }
        }
    }

    @ViewBuilder
    private var terminalPlaceholder: some View {
        if let paneID = tab.paneID {
            TerminalView(paneID: paneID)
        } else {
            VStack(spacing: 12) {
                Spacer()
                Image(systemName: "terminal")
                    .font(.system(size: 40))
                    .foregroundStyle(.tertiary)
                Text("No pane available")
                    .font(.headline)
                    .foregroundStyle(.secondary)
                Spacer()
            }
            .frame(maxWidth: .infinity)
            .background(Color.black)
        }
    }

    private var vcsPlaceholder: some View {
        VStack(spacing: 12) {
            Spacer()
            Image(systemName: "arrow.triangle.branch")
                .font(.system(size: 40))
                .foregroundStyle(.tertiary)
            Text("Source Control")
                .font(.headline)
                .foregroundStyle(.secondary)
            Spacer()
        }
        .frame(maxWidth: .infinity)
        .background(Color(.systemBackground))
    }

    private var editorPlaceholder: some View {
        VStack(spacing: 12) {
            Spacer()
            Image(systemName: "doc.text")
                .font(.system(size: 40))
                .foregroundStyle(.tertiary)
            Text(tab.title)
                .font(.headline)
                .foregroundStyle(.secondary)
            Spacer()
        }
        .frame(maxWidth: .infinity)
        .background(Color(.systemBackground))
    }
}
