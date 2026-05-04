import Foundation

enum OpenerItem: Identifiable {
    case project(ProjectItem)
    case worktree(WorktreeItem)
    case layout(LayoutItem)
    case branch(BranchItem)
    case openTab(OpenTabItem)

    struct ProjectItem {
        let projectID: UUID
        let projectName: String
    }

    struct WorktreeItem {
        let projectID: UUID
        let projectName: String
        let worktreeID: UUID
        let worktreeName: String
        let branch: String?
        let isPrimary: Bool
    }

    struct LayoutItem {
        let projectID: UUID
        let projectName: String
        let layoutName: String
    }

    struct BranchItem {
        let projectID: UUID
        let projectName: String
        let branch: String
        let matchingWorktreeID: UUID?
    }

    struct OpenTabItem {
        let projectID: UUID
        let projectName: String
        let areaID: UUID
        let tabID: UUID
        let title: String
        let kind: String
    }

    var category: OpenerCategory {
        switch self {
        case .project: .projects
        case .worktree: .worktrees
        case .layout: .layouts
        case .branch: .branches
        case .openTab: .openTabs
        }
    }

    var id: String {
        switch self {
        case let .project(item):
            "project:\(item.projectID)"
        case let .worktree(item):
            "worktree:\(item.projectID):\(item.worktreeID)"
        case let .layout(item):
            "layout:\(item.projectID):\(item.layoutName)"
        case let .branch(item):
            "branch:\(item.projectID):\(item.branch)"
        case let .openTab(item):
            "tab:\(item.tabID)"
        }
    }

    var title: String {
        switch self {
        case let .project(item): item.projectName
        case let .worktree(item): item.worktreeName
        case let .layout(item): item.layoutName
        case let .branch(item): item.branch
        case let .openTab(item): item.title
        }
    }

    var subtitle: String? {
        switch self {
        case .project:
            return nil
        case let .worktree(item):
            if let branch = item.branch, branch.caseInsensitiveCompare(item.worktreeName) != .orderedSame {
                return "\(branch) · \(item.projectName)"
            }
            return item.projectName
        case let .layout(item):
            return item.projectName
        case let .branch(item):
            return item.matchingWorktreeID != nil ? "\(item.projectName) · has worktree" : item.projectName
        case let .openTab(item):
            return "\(item.kind) · \(item.projectName)"
        }
    }

    var searchKey: String {
        switch self {
        case let .project(item): "\(item.projectName) project"
        case let .worktree(item): "\(item.worktreeName) \(item.branch ?? "") \(item.projectName)"
        case let .layout(item): "\(item.layoutName) layout \(item.projectName)"
        case let .branch(item): "\(item.branch) branch \(item.projectName)"
        case let .openTab(item): "\(item.title) tab \(item.projectName)"
        }
    }
}
