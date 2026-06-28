import Foundation

@MainActor
@Observable
final class BranchCache {
    static let shared = BranchCache()

    private var branchesByPath: [String: [String]] = [:]

    func update(projectPath: String, branches: [String]) {
        branchesByPath[projectPath] = branches
    }

    func branches(for projectPath: String) -> [String] {
        branchesByPath[projectPath] ?? []
    }
}
