import Foundation
import MuxyShared

extension Project {
    func toDTO() -> ProjectDTO {
        ProjectDTO(
            id: id,
            name: name,
            path: path,
            sortOrder: sortOrder,
            createdAt: createdAt,
            icon: icon,
            logo: logo,
            iconColor: iconColor
        )
    }
}

extension Worktree {
    func toDTO() -> WorktreeDTO {
        WorktreeDTO(
            id: id,
            name: name,
            path: path,
            branch: branch,
            isPrimary: isPrimary,
            createdAt: createdAt
        )
    }
}

@MainActor
extension SplitNode {
    func toDTO() -> SplitNodeDTO {
        switch self {
        case let .tabArea(area):
            .tabArea(area.toDTO())
        case let .split(branch):
            .split(branch.toDTO())
        }
    }
}

@MainActor
extension SplitBranch {
    func toDTO() -> SplitBranchDTO {
        SplitBranchDTO(
            id: id,
            direction: direction == .horizontal ? .horizontal : .vertical,
            ratio: Double(ratio),
            first: first.toDTO(),
            second: second.toDTO()
        )
    }
}

@MainActor
extension TabArea {
    func toDTO() -> TabAreaDTO {
        TabAreaDTO(
            id: id,
            projectPath: projectPath,
            tabs: tabs.map { $0.toDTO() },
            activeTabID: activeTabID
        )
    }
}

@MainActor
extension TerminalTab {
    func toDTO() -> TabDTO {
        TabDTO(
            id: id,
            kind: kind.toDTO(),
            title: title,
            isPinned: isPinned,
            paneID: content.pane?.id
        )
    }
}

extension TerminalTab.Kind {
    func toDTO() -> TabKindDTO {
        switch self {
        case .terminal: .terminal
        case .vcs: .vcs
        case .editor: .editor
        }
    }
}

extension MuxyNotification {
    func toDTO() -> NotificationDTO {
        NotificationDTO(
            id: id,
            paneID: paneID,
            projectID: projectID,
            worktreeID: worktreeID,
            areaID: areaID,
            tabID: tabID,
            source: source.toDTO(),
            title: title,
            body: body,
            timestamp: timestamp,
            isRead: isRead
        )
    }
}

extension MuxyNotification.Source {
    func toDTO() -> NotificationDTO.SourceDTO {
        switch self {
        case .osc: .osc
        case let .aiProvider(name): .aiProvider(name)
        case .socket: .socket
        }
    }
}
