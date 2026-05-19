import Darwin
import Foundation
import os

private let processTimeoutLogger = Logger(subsystem: "app.muxy", category: "ProcessTimeout")

enum ProcessTimeoutWatcher {
    private static let timerQueue = DispatchQueue(
        label: "app.muxy.process-timeout-watcher",
        qos: .userInitiated,
        attributes: .concurrent
    )

    static let graceBeforeKillSeconds: TimeInterval = 2.0

    @discardableResult
    static func install(
        on process: Process,
        timeout: TimeInterval,
        onFire: (() -> Void)? = nil
    ) -> DispatchWorkItem {
        let item = DispatchWorkItem { [weak process] in
            guard let process, process.isRunning else { return }
            onFire?()
            let pid = process.processIdentifier
            if !signalProcessGroup(pid: pid, signal: SIGTERM) {
                processTimeoutLogger.debug("Process group SIGTERM unavailable pid=\(pid)")
                process.terminate()
            }
            timerQueue.asyncAfter(deadline: .now() + graceBeforeKillSeconds) { [weak process] in
                guard let process, process.isRunning, pid > 0 else { return }
                if !signalProcessGroup(pid: pid, signal: SIGKILL) {
                    kill(pid, SIGKILL)
                }
            }
        }
        timerQueue.asyncAfter(deadline: .now() + timeout, execute: item)
        return item
    }

    private static func signalProcessGroup(pid: pid_t, signal: Int32) -> Bool {
        guard pid > 0 else { return false }
        return kill(-pid, signal) == 0
    }
}
