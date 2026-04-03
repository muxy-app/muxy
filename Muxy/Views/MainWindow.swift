import SwiftUI

struct MainWindow: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(GhosttyService.self) private var ghostty
    private let sidebarWidth: CGFloat = 160

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 0) {
                SidebarToolbar()
                    .frame(width: sidebarWidth)
                Rectangle().fill(MuxyTheme.border).frame(width: 1)
                topBarContent
            }
            .frame(height: 32)
            .background(MuxyTheme.bg)

            Rectangle().fill(MuxyTheme.border).frame(height: 1)
                .background(MuxyTheme.bg)

            HStack(spacing: 0) {
                if appState.sidebarVisible {
                    HStack(spacing: 0) {
                        Sidebar()
                            .frame(width: sidebarWidth)
                        Rectangle().fill(MuxyTheme.border).frame(width: 1)
                    }
                    .background(MuxyTheme.bg)
                }

                ZStack {
                    MuxyTheme.terminalBg
                    if let project = activeProject {
                        TerminalArea(project: project, isActiveProject: true)
                            .id(project.id)
                    } else {
                        WelcomeView()
                    }
                }
            }
        }
        .id(ghostty.configVersion)
        .background(WindowConfigurator(configVersion: ghostty.configVersion))
        .edgesIgnoringSafeArea(.top)
        .onAppear {
            appState.restoreSelection(projects: projectStore.projects)
        }
    }

    @ViewBuilder
    private var topBarContent: some View {
        if let project = activeProject,
           let root = appState.workspaceRoot(for: project.id),
           case let .tabArea(area) = root
        {
            PaneTabStrip(
                area: area,
                isFocused: true,
                isWindowTitleBar: true,
                onFocus: {},
                onSelectTab: { tabID in
                    appState.dispatch(.selectTab(projectID: project.id, areaID: area.id, tabID: tabID))
                },
                onCreateTab: {
                    appState.dispatch(.createTab(projectID: project.id, areaID: area.id))
                },
                onCloseTab: { tabID in
                    appState.dispatch(.closeTab(projectID: project.id, areaID: area.id, tabID: tabID))
                },
                onSplit: { dir in
                    appState.dispatch(.splitArea(projectID: project.id, areaID: area.id, direction: dir, projectPath: project.path))
                },
                onClose: {
                    appState.dispatch(.closeArea(projectID: project.id, areaID: area.id))
                }
            )
        } else {
            HStack {
                if let project = activeProject {
                    Text(project.name)
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .padding(.leading, 12)
                }
                Spacer(minLength: 0)
            }
            .background(WindowDragRepresentable())
        }
    }

    private var activeProject: Project? {
        guard let pid = appState.activeProjectID else { return nil }
        return projectStore.projects.first { $0.id == pid }
    }
}
