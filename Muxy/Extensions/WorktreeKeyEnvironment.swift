import SwiftUI

extension EnvironmentValues {
    @Entry var activeWorktreeKey: WorktreeKey?
    @Entry var paneWorkspaceContext: WorkspaceContext = .local
}
