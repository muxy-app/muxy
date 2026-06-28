import Foundation
import os

@MainActor
final class TerminalOfflineService {
    static let shared = TerminalOfflineService()

    nonisolated private static let minScanInterval: TimeInterval = 5
    nonisolated private static let maxScanInterval: TimeInterval = 30

    private let logger = Logger(subsystem: "app.muxy", category: "TerminalOffline")
    private var timer: DispatchSourceTimer?

    private init() {}

    nonisolated static func scanInterval(for idleThreshold: TimeInterval) -> TimeInterval {
        min(maxScanInterval, max(minScanInterval, idleThreshold))
    }

    func start() {
        reload()
    }

    func reload() {
        stopTimer()
        guard TerminalOfflinePreferences.isEnabled else { return }
        startTimer()
    }

    private func startTimer() {
        guard timer == nil else { return }
        let interval = Self.scanInterval(for: TerminalOfflinePreferences.idleThreshold)
        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now() + interval, repeating: interval)
        timer.setEventHandler { [weak self] in
            MainActor.assumeIsolated {
                self?.scan()
            }
        }
        timer.resume()
        self.timer = timer
    }

    private func stopTimer() {
        timer?.cancel()
        timer = nil
    }

    private func scan() {
        guard TerminalOfflinePreferences.isEnabled else {
            stopTimer()
            return
        }
        let idleThreshold = TerminalOfflinePreferences.idleThreshold
        let now = Date()
        var freed = 0
        for view in TerminalViewRegistry.shared.liveViews {
            guard view.hasLiveSurface, !view.isTakenOffline, !view.isOfflineBlockedByRemote else { continue }
            guard let invisibleSince = view.offlineInvisibleSince else { continue }
            let invisibleDuration = now.timeIntervalSince(invisibleSince)
            guard invisibleDuration >= idleThreshold else { continue }
            let candidate = TerminalOfflinePolicy.Candidate(
                hasLiveSurface: true,
                isAlreadyOffline: false,
                invisibleDuration: invisibleDuration,
                isIdle: view.isTerminalIdle()
            )
            guard TerminalOfflinePolicy.shouldTakeOffline(
                candidate,
                isEnabled: true,
                idleThreshold: idleThreshold
            )
            else { continue }
            view.takeOffline()
            freed += 1
        }
        if freed > 0 {
            logger.debug("Took \(freed) idle terminal(s) offline")
        }
    }
}
