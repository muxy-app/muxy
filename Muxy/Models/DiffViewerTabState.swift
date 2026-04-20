import Foundation

@MainActor
@Observable
final class DiffViewerTabState: Identifiable {
    let id = UUID()
    let vcs: VCSTabState
    let filePath: String
    let isStaged: Bool
    let projectPath: String
    var mode: VCSTabState.ViewMode

    var displayTitle: String {
        (filePath as NSString).lastPathComponent
    }

    init(vcs: VCSTabState, filePath: String, isStaged: Bool) {
        self.vcs = vcs
        self.filePath = filePath
        self.isStaged = isStaged
        projectPath = vcs.projectPath
        mode = vcs.mode
        loadIfNeeded(forceFull: false)
    }

    func refresh(forceFull: Bool) {
        loadIfNeeded(forceFull: forceFull)
    }

    private func loadIfNeeded(forceFull: Bool) {
        if vcs.files.contains(where: { $0.path == filePath }) {
            vcs.ensureDiffLoaded(filePath: filePath, forceFull: forceFull)
            return
        }
        vcs.loadDiffWithHints(
            filePath: filePath,
            hints: GitRepositoryService.DiffHints(
                hasStaged: isStaged,
                hasUnstaged: !isStaged,
                isUntrackedOrNew: false
            ),
            forceFull: forceFull
        )
    }
}
