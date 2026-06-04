import SwiftUI

struct ExtensionPopoverAnchor: NSViewRepresentable {
    let anchorID: String
    let host: PopoverHost
    let state: ExtensionPopoverState?
    let width: Double?
    let height: Double?

    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore

    func makeCoordinator() -> ExtensionPopoverCoordinator {
        ExtensionPopoverCoordinator(
            anchorID: anchorID,
            host: host,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore
        )
    }

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        context.coordinator.anchorView = view
        return view
    }

    func updateNSView(_: NSView, context: Context) {
        context.coordinator.sync(state: state)
    }

    static func dismantleNSView(_: NSView, coordinator: ExtensionPopoverCoordinator) {
        coordinator.tearDown()
    }
}

@MainActor
final class ExtensionPopoverCoordinator: NSObject, NSPopoverDelegate {
    private let anchorID: String
    private let host: PopoverHost
    private let appState: AppState
    private let projectStore: ProjectStore?
    private let worktreeStore: WorktreeStore?

    weak var anchorView: NSView?
    private var popover: NSPopover?
    private var presentedStateID: UUID?

    init(
        anchorID: String,
        host: PopoverHost,
        appState: AppState,
        projectStore: ProjectStore?,
        worktreeStore: WorktreeStore?
    ) {
        self.anchorID = anchorID
        self.host = host
        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
    }

    func sync(state: ExtensionPopoverState?) {
        guard let state else {
            close()
            return
        }
        guard presentedStateID != state.id else {
            resizeIfNeeded(to: state)
            return
        }
        present(state)
    }

    func tearDown() {
        close()
    }

    private func present(_ state: ExtensionPopoverState) {
        guard let anchorView, anchorView.window != nil else { return }
        close()

        let popover = NSPopover()
        popover.behavior = .semitransient
        popover.delegate = self
        popover.contentViewController = makeContentController(for: state)
        popover.contentSize = contentSize(for: state)
        popover.show(relativeTo: anchorView.bounds, of: anchorView, preferredEdge: .maxY)

        self.popover = popover
        presentedStateID = state.id
    }

    private func close() {
        guard let popover else { return }
        popover.delegate = nil
        popover.performClose(nil)
        self.popover = nil
        presentedStateID = nil
    }

    private func resizeIfNeeded(to state: ExtensionPopoverState) {
        let size = contentSize(for: state)
        guard popover?.contentSize != size else { return }
        popover?.contentSize = size
    }

    private func makeContentController(for state: ExtensionPopoverState) -> NSViewController {
        let content = ExtensionPopoverView(state: state)
            .environment(appState)
            .environment(projectStore)
            .environment(worktreeStore)
        return NSHostingController(rootView: content)
    }

    private func contentSize(for state: ExtensionPopoverState) -> NSSize {
        NSSize(width: state.width, height: state.height)
    }

    func popoverDidClose(_: Notification) {
        popover = nil
        presentedStateID = nil
        host.close(anchorID: anchorID)
    }
}
