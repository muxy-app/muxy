import Foundation

@MainActor
@Observable
final class PopoverHost {
    static let shared = PopoverHost()

    static let minSize: Double = 80
    static let maxWidth: Double = 600
    static let maxHeight: Double = 720

    struct Open: Identifiable {
        let anchorID: String
        let state: ExtensionPopoverState
        var id: String { anchorID }
    }

    private(set) var open: Open?

    func isOpen(anchorID: String) -> Bool {
        open?.anchorID == anchorID
    }

    func state(for extensionID: String) -> ExtensionPopoverState? {
        guard let open, open.state.extensionID == extensionID else { return nil }
        return open.state
    }

    func toggle(anchorID: String, extensionID: String, popover: ExtensionPopover, data: ExtensionJSON?) {
        if isOpen(anchorID: anchorID) {
            close()
            return
        }
        present(anchorID: anchorID, extensionID: extensionID, popover: popover, data: data)
    }

    func present(anchorID: String, extensionID: String, popover: ExtensionPopover, data: ExtensionJSON?) {
        let state = ExtensionPopoverState(
            extensionID: extensionID,
            popoverID: popover.id,
            width: popover.width,
            height: popover.height,
            initialData: data ?? popover.defaultData
        )
        open = Open(anchorID: anchorID, state: state)
    }

    func close() {
        open = nil
    }

    func close(anchorID: String) {
        guard isOpen(anchorID: anchorID) else { return }
        close()
    }

    func close(extensionID: String) {
        guard open?.state.extensionID == extensionID else { return }
        close()
    }

    func resize(extensionID: String, width: Double, height: Double) {
        guard let state = state(for: extensionID) else { return }
        state.width = min(max(width, PopoverHost.minSize), PopoverHost.maxWidth)
        state.height = min(max(height, PopoverHost.minSize), PopoverHost.maxHeight)
    }
}
