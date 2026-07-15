import Foundation
import MuxyShared

@MainActor
struct RemoteWorkspacePresentation {
    let projectID: UUID
    let worktreeID: UUID
    let focusedAreaID: UUID?
    let root: SplitNode

    var focusedArea: TabArea? {
        guard let focusedAreaID else { return root.allAreas().first }
        return root.findArea(id: focusedAreaID)
    }

    var activeTab: TerminalTab? {
        focusedArea?.activeTab
    }
}

@MainActor
final class RemoteWorkspaceIdentityMap {
    enum Entity: Hashable {
        case project
        case worktree
        case split
        case area
        case tab
        case pane
    }

    private struct Key: Hashable {
        let entity: Entity
        let remoteID: UUID
    }

    private var presentationIDs: [Key: UUID] = [:]
    private var remoteIDs: [UUID: UUID] = [:]

    func presentationID(for remoteID: UUID, entity: Entity) -> UUID {
        let key = Key(entity: entity, remoteID: remoteID)
        if let existing = presentationIDs[key] {
            return existing
        }
        let presentationID = UUID()
        presentationIDs[key] = presentationID
        remoteIDs[presentationID] = remoteID
        return presentationID
    }

    func remoteID(for presentationID: UUID) -> UUID? {
        remoteIDs[presentationID]
    }
}

@MainActor
enum RemoteWorkspacePresentationBuilder {
    static func projects(
        from projects: [ProjectDTO],
        deviceID: UUID,
        identities: RemoteWorkspaceIdentityMap
    ) -> [Project] {
        projects.map { dto in
            var project = Project(
                id: identities.presentationID(for: dto.id, entity: .project),
                name: dto.name,
                path: dto.path,
                sortOrder: dto.sortOrder,
                remoteMacDeviceID: deviceID
            )
            project.icon = dto.icon
            project.iconColor = dto.iconColor
            return project
        }
    }

    static func workspace(
        from workspace: WorkspaceDTO,
        identities: RemoteWorkspaceIdentityMap
    ) -> RemoteWorkspacePresentation {
        RemoteWorkspacePresentation(
            projectID: identities.presentationID(for: workspace.projectID, entity: .project),
            worktreeID: identities.presentationID(for: workspace.worktreeID, entity: .worktree),
            focusedAreaID: workspace.focusedAreaID.map {
                identities.presentationID(for: $0, entity: .area)
            },
            root: node(from: workspace.root, identities: identities)
        )
    }

    private static func node(
        from dtoNode: SplitNodeDTO,
        identities: RemoteWorkspaceIdentityMap
    ) -> SplitNode {
        switch dtoNode {
        case let .tabArea(area):
            let tabs = area.tabs.map {
                tab(from: $0, projectPath: area.projectPath, identities: identities)
            }
            let activeTabID = area.activeTabID.map {
                identities.presentationID(for: $0, entity: .tab)
            }
            return .tabArea(TabArea(
                id: identities.presentationID(for: area.id, entity: .area),
                projectPath: area.projectPath,
                tabs: tabs,
                activeTabID: activeTabID
            ))
        case let .split(branch):
            return .split(SplitBranch(
                id: identities.presentationID(for: branch.id, entity: .split),
                direction: branch.direction == .horizontal ? .horizontal : .vertical,
                ratio: branch.ratio,
                first: node(from: branch.first, identities: identities),
                second: node(from: branch.second, identities: identities)
            ))
        }
    }

    private static func tab(
        from tab: TabDTO,
        projectPath: String,
        identities: RemoteWorkspaceIdentityMap
    ) -> TerminalTab {
        let pane = tab.paneID.map { remotePaneID in
            TerminalPaneState(
                id: identities.presentationID(for: remotePaneID, entity: .pane),
                projectPath: projectPath,
                title: tab.title,
                backend: .remoteMac(paneID: remotePaneID)
            )
        }
        return TerminalTab(
            id: identities.presentationID(for: tab.id, entity: .tab),
            title: tab.title,
            isPinned: tab.isPinned,
            projectPath: projectPath,
            kind: kind(for: tab.kind),
            pane: pane
        )
    }

    private static func kind(for kind: TabKindDTO) -> TerminalTab.Kind {
        switch kind {
        case .terminal: .terminal
        case .browser: .browser
        case .vcs,
             .extensionWebView: .extensionWebView
        }
    }
}
