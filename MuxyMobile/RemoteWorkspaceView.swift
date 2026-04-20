import MuxyShared
import SwiftUI

struct WorkspaceContentWrapper: View {
    @Environment(ConnectionManager.self) private var connection
    @State private var showingVCS = false

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
                    HStack(spacing: 16) {
                        vcsButton
                        tabPicker
                    }
                }
            }
            .toolbarColorScheme(preferredScheme, for: .navigationBar)
            .tint(themeFg)
            .background(themeBg.ignoresSafeArea())
            .sheet(isPresented: $showingVCS) {
                if let id = connection.activeProjectID {
                    VCSView(projectID: id)
                        .environment(connection)
                }
            }
    }

    private var vcsButton: some View {
        Button {
            showingVCS = true
        } label: {
            Label("Source Control", systemImage: "arrow.triangle.branch")
                .labelStyle(.iconOnly)
        }
        .disabled(connection.activeProjectID == nil)
    }

    private var themeFg: Color {
        connection.deviceTheme?.fgColor ?? .primary
    }

    private var themeBg: Color {
        connection.deviceTheme?.bgColor ?? Color(.systemBackground)
    }

    private var preferredScheme: ColorScheme {
        (connection.deviceTheme?.isDark ?? true) ? .dark : .light
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
            Label("Tabs", systemImage: "rectangle.stack")
                .labelStyle(.iconOnly)
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
        case .diffViewer: "rectangle.split.2x1"
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
            case .diffViewer:
                diffViewerPlaceholder
            }
        }
    }

    @ViewBuilder
    private var terminalPlaceholder: some View {
        if let paneID = tab.paneID {
            TerminalView(paneID: paneID)
        } else {
            placeholder(icon: "terminal", title: "No pane available")
        }
    }

    private var vcsPlaceholder: some View {
        placeholder(icon: "arrow.triangle.branch", title: "Source Control")
    }

    private var editorPlaceholder: some View {
        placeholder(icon: "doc.text", title: tab.title)
    }

    private var diffViewerPlaceholder: some View {
        placeholder(icon: "rectangle.split.2x1", title: tab.title)
    }

    private func placeholder(icon: String, title: String) -> some View {
        VStack(spacing: 12) {
            Spacer()
            Image(systemName: icon)
                .font(.system(size: 40))
                .foregroundStyle(themeFg.opacity(0.4))
            Text(title)
                .font(.headline)
                .foregroundStyle(themeFg.opacity(0.7))
            Spacer()
        }
        .frame(maxWidth: .infinity)
        .background(themeBg)
    }

    private var themeFg: Color {
        connection.deviceTheme?.fgColor ?? .primary
    }

    private var themeBg: Color {
        connection.deviceTheme?.bgColor ?? Color(.systemBackground)
    }
}
