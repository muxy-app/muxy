import MuxyShared
import SwiftUI

struct RemoteWorkspaceView: View {
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        NavigationSplitView {
            ProjectListView()
        } detail: {
            if connection.workspace != nil {
                WorkspaceDetailView()
            } else {
                ContentUnavailableView(
                    "Select a Project",
                    systemImage: "folder",
                    description: Text("Choose a project from the sidebar")
                )
            }
        }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    connection.disconnect()
                } label: {
                    Image(systemName: "xmark.circle")
                }
            }
        }
    }
}

struct ProjectListView: View {
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        List(connection.projects, selection: Binding(
            get: { connection.activeProjectID },
            set: { id in
                guard let id else { return }
                Task { await connection.selectProject(id) }
            }
        )) { project in
            Label(project.name, systemImage: "folder")
                .tag(project.id)
        }
        .navigationTitle("Projects")
        .refreshable {
            await connection.refreshProjects()
        }
    }
}

struct WorkspaceDetailView: View {
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        if let workspace = connection.workspace {
            TabAreaListView(root: workspace.root, projectID: workspace.projectID)
                .navigationTitle("Workspace")
        }
    }
}

struct TabAreaListView: View {
    let root: SplitNodeDTO
    let projectID: UUID
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        List {
            ForEach(collectAreas(from: root)) { area in
                Section(area.projectPath.components(separatedBy: "/").last ?? "Area") {
                    ForEach(area.tabs) { tab in
                        HStack {
                            Image(systemName: iconForKind(tab.kind))
                            Text(tab.title)
                            Spacer()
                            if tab.id == area.activeTabID {
                                Image(systemName: "checkmark")
                                    .foregroundStyle(.tint)
                            }
                        }
                        .contentShape(Rectangle())
                        .onTapGesture {
                            Task {
                                await connection.selectTab(
                                    projectID: projectID,
                                    areaID: area.id,
                                    tabID: tab.id
                                )
                            }
                        }
                    }
                }
            }
        }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    Task { await connection.createTab(projectID: projectID) }
                } label: {
                    Image(systemName: "plus")
                }
            }
        }
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
