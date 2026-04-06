import SwiftUI

struct MainWindow: View {
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(GhosttyService.self) private var ghostty
    @Environment(\.openWindow) private var openWindow
    @State private var dragCoordinator = TabDragCoordinator()
    private enum AttachedVCSLayout {
        static let minWidth: CGFloat = 200
        static let defaultWidth: CGFloat = 400
        static let maxWidth: CGFloat = 800
    }

    @State private var vcsPanelVisible = false
    @State private var vcsPanelWidth: CGFloat = AttachedVCSLayout.defaultWidth
    @State private var vcsStates: [UUID: VCSTabState] = [:]
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
                    if projectsWithWorkspaces.isEmpty {
                        WelcomeView()
                    } else if let project = activeProjectWithWorkspace {
                        TerminalArea(project: project, isActiveProject: true)
                            .id(project.id)
                    }
                }

                if vcsPanelVisible, VCSDisplayMode.current == .attached, let state = activeVCSState {
                    HStack(spacing: 0) {
                        Rectangle().fill(MuxyTheme.border).frame(width: 1)
                            .overlay {
                                Color.clear
                                    .frame(width: 5)
                                    .contentShape(Rectangle())
                                    .gesture(
                                        DragGesture(minimumDistance: 1)
                                            .onChanged { v in
                                                let delta = v.translation.width
                                                vcsPanelWidth = max(
                                                    AttachedVCSLayout.minWidth,
                                                    min(AttachedVCSLayout.maxWidth, vcsPanelWidth - delta)
                                                )
                                            }
                                    )
                                    .onHover { on in
                                        if on { NSCursor.resizeLeftRight.push() } else { NSCursor.pop() }
                                    }
                            }
                        VCSTabView(state: state, focused: false, onFocus: {})
                            .frame(width: vcsPanelWidth)
                    }
                }
            }
        }
        .overlay(alignment: .bottom) {
            if let toast = ToastState.shared.message {
                HStack(spacing: 6) {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(MuxyTheme.diffAddFg)
                    Text(toast)
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(MuxyTheme.fg)
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 8)
                .background(MuxyTheme.surface, in: Capsule())
                .overlay(Capsule().stroke(MuxyTheme.border, lineWidth: 1))
                .padding(.bottom, 16)
                .transition(.move(edge: .bottom).combined(with: .opacity))
                .allowsHitTesting(false)
            }
        }
        .animation(.easeInOut(duration: 0.2), value: ToastState.shared.message != nil)
        .coordinateSpace(name: DragCoordinateSpace.mainWindow)
        .environment(dragCoordinator)
        .background(WindowConfigurator(configVersion: ghostty.configVersion))
        .edgesIgnoringSafeArea(.top)
        .onAppear {
            appState.restoreSelection(projects: projectStore.projects)
        }
        .onReceive(NotificationCenter.default.publisher(for: .openVCSWindow)) { _ in
            openWindow(id: "vcs")
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleAttachedVCS)) { _ in
            if let project = activeProject {
                ensureVCSState(for: project)
            }
            vcsPanelVisible.toggle()
        }
        .onChange(of: projectStore.projects.map(\.id)) {
            pruneVCSStates(validProjectIDs: Set(projectStore.projects.map(\.id)))
        }
        .onChange(of: appState.activeProjectID) {
            guard vcsPanelVisible, VCSDisplayMode.current == .attached,
                  let project = activeProject
            else { return }
            ensureVCSState(for: project)
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
                showVCSButton: true,
                projectID: project.id,
                onFocus: {},
                onSelectTab: { tabID in
                    appState.dispatch(.selectTab(projectID: project.id, areaID: area.id, tabID: tabID))
                },
                onCreateTab: {
                    appState.dispatch(.createTab(projectID: project.id, areaID: area.id))
                },
                onCreateVCSTab: {
                    openVCS(for: project, preferredAreaID: area.id)
                },
                onCloseTab: { tabID in
                    appState.dispatch(.closeTab(projectID: project.id, areaID: area.id, tabID: tabID))
                },
                onSplit: { dir in
                    appState.dispatch(.splitArea(
                        projectID: project.id, areaID: area.id, direction: dir, projectPath: project.path
                    ))
                },
                onClose: {
                    appState.dispatch(.closeArea(projectID: project.id, areaID: area.id))
                },
                onDropAction: { result in
                    appState.dispatch(result.action(projectID: project.id))
                }
            )
        } else {
            WindowDragRepresentable(alwaysEnabled: true)
                .overlay {
                    HStack {
                        if let project = activeProject {
                            Text(project.name)
                                .font(.system(size: 12, weight: .semibold))
                                .foregroundStyle(MuxyTheme.fgMuted)
                                .padding(.leading, 12)
                        }
                        Spacer(minLength: 0)
                    }
                    .allowsHitTesting(false)
                }
                .overlay(alignment: .trailing) {
                    if let project = activeProject, activeProjectHasSplitWorkspace {
                        FileDiffIconButton {
                            openVCS(for: project)
                        }
                        .padding(.trailing, 4)
                    }
                }
        }
    }

    private var activeProject: Project? {
        guard let pid = appState.activeProjectID else { return nil }
        return projectStore.projects.first { $0.id == pid }
    }

    private var activeProjectWithWorkspace: Project? {
        guard let project = activeProject,
              appState.workspaceRoot(for: project.id) != nil
        else { return nil }
        return project
    }

    private var activeProjectHasSplitWorkspace: Bool {
        guard let project = activeProject,
              let root = appState.workspaceRoot(for: project.id)
        else { return false }
        if case .split = root { return true }
        return false
    }

    private var projectsWithWorkspaces: [Project] {
        projectStore.projects.filter { appState.workspaceRoots[$0.id] != nil }
    }

    private var activeVCSState: VCSTabState? {
        guard let project = activeProject else { return nil }
        return vcsStates[project.id]
    }

    private func ensureVCSState(for project: Project) {
        guard vcsStates[project.id] == nil else { return }
        vcsStates[project.id] = VCSTabState(projectPath: project.path)
    }

    private func openVCS(for project: Project, preferredAreaID: UUID? = nil) {
        VCSDisplayMode.current.route(
            tab: {
                let areaID = preferredAreaID
                    ?? appState.focusedAreaID[project.id]
                    ?? appState.workspaceRoot(for: project.id)?.allAreas().first?.id
                guard let areaID else { return }
                appState.dispatch(.createVCSTab(projectID: project.id, areaID: areaID))
            },
            window: { openWindow(id: "vcs") },
            attached: {
                ensureVCSState(for: project)
                vcsPanelVisible.toggle()
            }
        )
    }

    private func pruneVCSStates(validProjectIDs: Set<UUID>) {
        vcsStates = vcsStates.filter { validProjectIDs.contains($0.key) }
    }
}
