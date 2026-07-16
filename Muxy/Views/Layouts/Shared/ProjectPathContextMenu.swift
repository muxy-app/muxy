import SwiftUI

struct ProjectPathContextMenu: View {
    let path: String
    let workspaceContext: WorkspaceContext

    var body: some View {
        Button("Copy Path") {
            Task {
                await ProjectPathCopyService.copy(path: path, workspaceContext: workspaceContext)
            }
        }
    }
}

struct ProjectContextMenuFooter<FinalAction: View>: View {
    let path: String
    let workspaceContext: WorkspaceContext
    let separatesFromPreviousActions: Bool
    private let finalAction: FinalAction

    init(
        path: String,
        workspaceContext: WorkspaceContext,
        separatesFromPreviousActions: Bool = true,
        @ViewBuilder finalAction: () -> FinalAction
    ) {
        self.path = path
        self.workspaceContext = workspaceContext
        self.separatesFromPreviousActions = separatesFromPreviousActions
        self.finalAction = finalAction()
    }

    var body: some View {
        if separatesFromPreviousActions {
            Divider()
        }
        ProjectPathContextMenu(path: path, workspaceContext: workspaceContext)
        Divider()
        finalAction
    }
}
