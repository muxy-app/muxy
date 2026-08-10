import Foundation
import Testing

@testable import Muxy

@Suite("Rich input presentation controller")
@MainActor
struct RichInputPresentationControllerTests {
    private let firstTarget = RichInputPresentationTarget(
        worktreeKey: WorktreeKey(projectID: UUID(), worktreeID: UUID()),
        paneID: UUID()
    )

    @Test("presents and closes the panel")
    func presentsAndClosesPanel() {
        let host = PanelHost()
        let controller = RichInputPresentationController(panelHost: host)

        controller.present(mode: .panel, position: .right, target: firstTarget)

        #expect(controller.isVisible)
        #expect(controller.isPanelVisible)
        #expect(!controller.isFloatingVisible)
        #expect(host.placement(for: BuiltinPanel.richInput)?.position == .right)
        #expect(controller.close())
        #expect(!controller.isVisible)
        #expect(controller.target == nil)
    }

    @Test("switches between panel and floating presentations")
    func switchesPresentations() {
        let host = PanelHost()
        let controller = RichInputPresentationController(panelHost: host)

        controller.present(mode: .panel, position: .right, target: firstTarget)
        controller.synchronize(mode: .floating, position: .right)

        #expect(controller.isFloatingVisible)
        #expect(!controller.isPanelVisible)
        #expect(controller.target == firstTarget)

        controller.synchronize(mode: .panel, position: .bottom)

        #expect(!controller.isFloatingVisible)
        #expect(controller.isPanelVisible)
        #expect(host.placement(for: BuiltinPanel.richInput)?.position == .bottom)
    }

    @Test("moves an open panel without closing it")
    func movesPanel() {
        let host = PanelHost()
        let controller = RichInputPresentationController(panelHost: host)
        controller.present(mode: .panel, position: .right, target: firstTarget)

        controller.movePanel(to: .bottom)

        #expect(controller.isPanelVisible)
        #expect(host.placement(for: BuiltinPanel.richInput)?.position == .bottom)
    }

    @Test("reports unexpected panel displacement")
    func reportsPanelDisplacement() {
        let host = PanelHost()
        let controller = RichInputPresentationController(panelHost: host)
        let recorder = RichInputPresentationVoiceRecorderStub()
        recorder.isRecording = true
        let voice = ComposerVoiceState(recorder: recorder)
        controller.present(mode: .panel, position: .right, target: firstTarget)

        host.open("extension:files", at: .right, mode: .pinned)
        let didClose = controller.reconcilePanelHostChange(voice: voice)

        #expect(didClose)
        #expect(!controller.isVisible)
        #expect(controller.target == nil)
        #expect(recorder.cancelCount == 1)
    }

    @Test("presentation changes preserve rich input state")
    func presentationChangesPreserveRichInputState() {
        let host = PanelHost()
        let controller = RichInputPresentationController(panelHost: host)
        let state = RichInputState()
        state.text = "draft"
        state.fileAttachments = [URL(fileURLWithPath: "/tmp/file.txt")]
        state.imageAttachments = [URL(fileURLWithPath: "/tmp/image.png")]

        controller.present(mode: .panel, position: .right, target: firstTarget)
        controller.synchronize(mode: .floating, position: .right)
        controller.synchronize(mode: .panel, position: .bottom)

        #expect(state.text == "draft")
        #expect(state.fileAttachments.map(\.path) == ["/tmp/file.txt"])
        #expect(state.imageAttachments.map(\.path) == ["/tmp/image.png"])
    }

    @Test("cross-worktree target changes report the previous draft and rebind an open panel")
    func crossWorktreeTargetChangesReportPreviousDraftAndRebindPanel() {
        let host = PanelHost()
        let controller = RichInputPresentationController(panelHost: host)
        let recorder = RichInputPresentationVoiceRecorderStub()
        recorder.isRecording = true
        let voice = ComposerVoiceState(recorder: recorder)
        let nextTarget = RichInputPresentationTarget(
            worktreeKey: WorktreeKey(projectID: UUID(), worktreeID: UUID()),
            paneID: UUID()
        )
        controller.present(mode: .panel, position: .right, target: firstTarget)

        let reconciliation = controller.reconcileTargetChange(nextTarget, voice: voice)

        #expect(reconciliation == .transferredAcrossWorktrees(previousTarget: firstTarget))
        #expect(controller.isPanelVisible)
        #expect(controller.target == nextTarget)
        #expect(recorder.cancelCount == 1)
    }

    @Test("same-worktree pane changes rebind without reporting a worktree transfer")
    func sameWorktreePaneChangesRebindWithoutWorktreeTransfer() {
        let host = PanelHost()
        let controller = RichInputPresentationController(panelHost: host)
        let voice = ComposerVoiceState(recorder: RichInputPresentationVoiceRecorderStub())
        let nextTarget = RichInputPresentationTarget(
            worktreeKey: firstTarget.worktreeKey,
            paneID: UUID()
        )
        controller.present(mode: .panel, position: .right, target: firstTarget)

        let reconciliation = controller.reconcileTargetChange(nextTarget, voice: voice)

        #expect(reconciliation == .rebound)
        #expect(controller.isPanelVisible)
        #expect(controller.target == nextTarget)
    }

    @Test("losing the target closes the panel and cancels dictation")
    func losingTargetClosesPanelAndCancelsDictation() {
        let host = PanelHost()
        let controller = RichInputPresentationController(panelHost: host)
        let recorder = RichInputPresentationVoiceRecorderStub()
        recorder.isRecording = true
        let voice = ComposerVoiceState(recorder: recorder)
        controller.present(mode: .panel, position: .right, target: firstTarget)

        let reconciliation = controller.reconcileTargetChange(nil, voice: voice)

        #expect(reconciliation == .closed(previousTarget: firstTarget))
        #expect(!controller.isVisible)
        #expect(controller.target == nil)
        #expect(recorder.cancelCount == 1)
    }

    @Test("changing a floating target closes the composer and cancels dictation")
    func changingFloatingTargetClosesComposerAndCancelsDictation() {
        let host = PanelHost()
        let controller = RichInputPresentationController(panelHost: host)
        let recorder = RichInputPresentationVoiceRecorderStub()
        recorder.isRecording = true
        let voice = ComposerVoiceState(recorder: recorder)
        let nextTarget = RichInputPresentationTarget(
            worktreeKey: WorktreeKey(projectID: UUID(), worktreeID: UUID()),
            paneID: UUID()
        )
        controller.present(mode: .floating, position: .right, target: firstTarget)

        let reconciliation = controller.reconcileTargetChange(nextTarget, voice: voice)

        #expect(reconciliation == .closed(previousTarget: firstTarget))
        #expect(!controller.isVisible)
        #expect(controller.target == nil)
        #expect(recorder.cancelCount == 1)
    }
}

@MainActor
private final class RichInputPresentationVoiceRecorderStub: VoiceRecording {
    var isRecording = false
    var isPaused = false
    var elapsed: TimeInterval = 0
    var level: Float = 0
    var transcript = ""
    var onFailure: (@MainActor (String) -> Void)?
    private(set) var cancelCount = 0

    func start(locale _: Locale) throws {
        isRecording = true
    }

    func pause() {
        isPaused = true
    }

    func resume() {
        isPaused = false
    }

    func finish() -> String {
        isRecording = false
        return transcript
    }

    func cancel() {
        cancelCount += 1
        isRecording = false
        isPaused = false
    }
}
