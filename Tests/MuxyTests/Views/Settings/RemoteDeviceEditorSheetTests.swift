import Foundation
import Testing

@testable import Muxy

@Suite("Remote device editor")
struct RemoteDeviceEditorSheetTests {
    @MainActor
    @Test("tmux mode changes invalidate a direct SSH probe")
    func tmuxModeInvalidatesDirectProbe() async {
        let direct = SSHDestination(host: "example.com", remoteSessionMode: .direct)
        let tmux = SSHDestination(host: "example.com", remoteSessionMode: .tmux)
        let controller = RemoteDeviceProbeController()
        var probes: [CheckedContinuation<RemoteDeviceProbeController.Outcome, Never>] = []
        let operation: @MainActor (SSHDestination) async -> RemoteDeviceProbeController.Outcome = { _ in
            await withCheckedContinuation { probes.append($0) }
        }

        let directTask = controller.run(destination: direct, operation: operation)
        await waitForProbeCount(1) { probes.count }
        let tmuxTask = controller.run(destination: tmux, operation: operation)
        await waitForProbeCount(2) { probes.count }

        probes[0].resume(returning: .succeeded)
        await directTask.value
        #expect(controller.state == .testing)

        probes[1].resume(returning: .succeeded)
        await tmuxTask.value
        #expect(controller.state == .succeeded)
    }

    @MainActor
    private func waitForProbeCount(_ count: Int, probeCount: () -> Int) async {
        while probeCount() < count {
            await Task.yield()
        }
    }
}
