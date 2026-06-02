import Darwin
import Foundation

final class ParentDeathMonitor {
    private var source: DispatchSourceProcess?

    func start() {
        guard getppid() > 1 else {
            exit(0)
        }

        let monitor = DispatchSource.makeProcessSource(
            identifier: getppid(),
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
