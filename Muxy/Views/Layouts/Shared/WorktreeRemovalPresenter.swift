import SwiftUI

struct WorktreeRemovalRequest: Identifiable {
    let id = UUID()
    let worktree: Worktree
    let repoPath: String
    let context: WorkspaceContext
    let onSuccess: @MainActor () -> Void
}

private struct WorktreeRemovalPresenterModifier: ViewModifier {
    @Binding var request: WorktreeRemovalRequest?
    @State private var controller: WorktreeRemovalController?

    func body(content: Content) -> some View {
        content
            .sheet(item: $request) { current in
                let controller = controller(for: current)
                WorktreeRemovalSheet(controller: controller, onDismiss: cleanup)
                    .task(id: current.id) { await run(current, controller: controller) }
            }
    }

    private func controller(for current: WorktreeRemovalRequest) -> WorktreeRemovalController {
        if let controller, controller.worktree.id == current.worktree.id { return controller }
        let new = WorktreeRemovalController(worktree: current.worktree)
        controller = new
        return new
    }

    private func cleanup() {
        request = nil
        controller = nil
    }

    private func run(_ current: WorktreeRemovalRequest, controller: WorktreeRemovalController) async {
        do {
            try await WorktreeStore.cleanupOnDisk(
                worktree: current.worktree,
                repoPath: current.repoPath,
                context: current.context,
                teardownEmit: { line in
                    Task { @MainActor in controller.append(line) }
                }
            )
            cleanup()
            current.onSuccess()
        } catch {
            controller.markFailed(error)
        }
    }
}

extension View {
    func worktreeRemovalSheet(_ request: Binding<WorktreeRemovalRequest?>) -> some View {
        modifier(WorktreeRemovalPresenterModifier(request: request))
    }
}
