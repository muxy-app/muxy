import Foundation

struct RichInputPresentationTarget: Equatable {
    let worktreeKey: WorktreeKey
    let paneID: UUID
}

enum RichInputTargetReconciliation: Equatable {
    case unchanged
    case rebound
    case transferredAcrossWorktrees(previousTarget: RichInputPresentationTarget)
    case closed(previousTarget: RichInputPresentationTarget?)
}

@MainActor
@Observable
final class RichInputPresentationController {
    private(set) var isFloatingVisible = false
    private(set) var target: RichInputPresentationTarget?
    private let panelHost: PanelHost
    private var panelWasVisible: Bool

    init(panelHost: PanelHost = .shared) {
        self.panelHost = panelHost
        panelWasVisible = panelHost.isOpen(BuiltinPanel.richInput)
    }

    var isPanelVisible: Bool {
        panelHost.isOpen(BuiltinPanel.richInput)
    }

    var isVisible: Bool {
        isFloatingVisible || isPanelVisible
    }

    func present(
        mode: RichInputPresentationMode,
        position: PanelPosition,
        target: RichInputPresentationTarget
    ) {
        self.target = target
        isFloatingVisible = false
        panelHost.close(BuiltinPanel.richInput)
        switch mode {
        case .panel:
            panelHost.open(BuiltinPanel.richInput, at: position, mode: .pinned)
        case .floating:
            isFloatingVisible = true
        }
        panelWasVisible = isPanelVisible
    }

    @discardableResult
    func close() -> Bool {
        let wasVisible = isVisible
        target = nil
        isFloatingVisible = false
        panelHost.close(BuiltinPanel.richInput)
        panelWasVisible = false
        return wasVisible
    }

    @discardableResult
    func synchronize(mode: RichInputPresentationMode, position: PanelPosition) -> Bool {
        guard isVisible, let target else { return false }
        if mode == .panel, isPanelVisible, !isFloatingVisible {
            return false
        }
        if mode == .floating, isFloatingVisible, !isPanelVisible {
            return false
        }
        present(mode: mode, position: position, target: target)
        return true
    }

    func movePanel(to position: PanelPosition) {
        guard isPanelVisible else { return }
        panelHost.move(BuiltinPanel.richInput, to: position)
        panelWasVisible = isPanelVisible
    }

    @discardableResult
    func reconcilePanelHostChange(voice: ComposerVoiceState) -> Bool {
        let panelIsVisible = isPanelVisible
        defer { panelWasVisible = panelIsVisible }
        guard panelWasVisible, !panelIsVisible, !isFloatingVisible else { return false }
        target = nil
        voice.cancel()
        return true
    }

    func reconcileTargetChange(
        _ currentTarget: RichInputPresentationTarget?,
        voice: ComposerVoiceState
    ) -> RichInputTargetReconciliation {
        guard isVisible else {
            target = nil
            return .unchanged
        }
        guard target != currentTarget else { return .unchanged }
        let previousTarget = target
        voice.cancel()
        guard isPanelVisible, let currentTarget else {
            close()
            return .closed(previousTarget: previousTarget)
        }
        target = currentTarget
        guard let previousTarget,
              previousTarget.worktreeKey != currentTarget.worktreeKey
        else { return .rebound }
        return .transferredAcrossWorktrees(previousTarget: previousTarget)
    }
}
