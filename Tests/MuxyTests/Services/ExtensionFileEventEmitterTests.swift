import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionFileEventEmitter")
struct ExtensionFileEventEmitterTests {
    @Test("emits one file.changed event per path with path and projectPath")
    func emitsPerPath() async {
        let collector = EventCollector()
        let token = NotificationSocketServer.shared.addInProcessObserver { event in
            collector.add(event)
        }
        defer { NotificationSocketServer.shared.removeInProcessObserver(token) }

        ExtensionFileEventEmitter.emit(
            paths: ["/per-path-repo/a.txt", "/per-path-repo/b.txt"],
            projectPath: "/per-path-repo"
        )

        let delivered = await waitFor(timeout: 2.0) {
            collector.count(forProjectPath: "/per-path-repo") == 2
        }
        #expect(delivered)

        let events = collector.events(forProjectPath: "/per-path-repo")
        #expect(Set(events.map { $0.payload["path"] ?? "" }) == ["/per-path-repo/a.txt", "/per-path-repo/b.txt"])
    }

    @Test("emits nothing for an empty path list")
    func emitsNothingWhenEmpty() async {
        let collector = EventCollector()
        let token = NotificationSocketServer.shared.addInProcessObserver { event in
            collector.add(event)
        }
        defer { NotificationSocketServer.shared.removeInProcessObserver(token) }

        ExtensionFileEventEmitter.emit(paths: [], projectPath: "/empty-repo")
        try? await Task.sleep(nanoseconds: 300_000_000)

        #expect(collector.count(forProjectPath: "/empty-repo") == 0)
    }

    @Test("collapses duplicate path within the dedupe window")
    func dropsDuplicateWithinWindow() async {
        let collector = EventCollector()
        let token = NotificationSocketServer.shared.addInProcessObserver { event in
            collector.add(event)
        }
        defer { NotificationSocketServer.shared.removeInProcessObserver(token) }

        ExtensionFileEventEmitter.emit(paths: ["/dup-repo/a.txt"], projectPath: "/dup-repo")
        ExtensionFileEventEmitter.emit(paths: ["/dup-repo/a.txt"], projectPath: "/dup-repo")

        let delivered = await waitFor(timeout: 2.0) {
            collector.count(forProjectPath: "/dup-repo") == 1
        }
        #expect(delivered)
        try? await Task.sleep(nanoseconds: 200_000_000)
        #expect(collector.count(forProjectPath: "/dup-repo") == 1)
    }

    private func waitFor(timeout: TimeInterval, condition: () -> Bool) async -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() { return true }
            try? await Task.sleep(nanoseconds: 50_000_000)
        }
        return condition()
    }
}

private final class EventCollector: @unchecked Sendable {
    private let lock = NSLock()
    private var events: [ExtensionEvent] = []

    func add(_ event: ExtensionEvent) {
        lock.lock()
        events.append(event)
        lock.unlock()
    }

    func events(forProjectPath projectPath: String) -> [ExtensionEvent] {
        lock.lock()
        defer { lock.unlock() }
        return events.filter {
            $0.name == ExtensionEventName.fileChanged && $0.payload["projectPath"] == projectPath
        }
    }

    func count(forProjectPath projectPath: String) -> Int {
        events(forProjectPath: projectPath).count
    }
}
