import Darwin
import Foundation

final class ParentDeathMonitor {
    private let parentPID: pid_t
    private var source: DispatchSourceProcess?

    init() {
        parentPID = getppid()
    }

    func start() {
        guard parentPID > 1 else {
            exit(0)
        }

        let monitor = DispatchSource.makeProcessSource(
            identifier: parentPID,
            eventMask: .exit,
            queue: .global(qos: .utility)
        )
        monitor.setEventHandler {
            exit(0)
        }
        monitor.resume()
        source = monitor

        if getppid() <= 1 {
            exit(0)
        }
    }
}
