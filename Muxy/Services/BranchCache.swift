import Foundation

@MainActor
@Observable
final class BranchCache {
    static let shared = BranchCache()

    private var branchesByPath: [String: [String]] = [:]
    private var currentBranchByPath: [String: String] = [:]

    func update(projectPath: String, branches: [String], current: String?) {
        branchesByPath[projectPath] = branches
        if let current {
            currentBranchByPath[projectPath] = current
        }
    }

    func branches(for projectPath: String) -> [String] {
        branchesByPath[projectPath] ?? []
    }

    func currentBranch(for projectPath: String) -> String? {
        currentBranchByPath[projectPath]
    }
}
