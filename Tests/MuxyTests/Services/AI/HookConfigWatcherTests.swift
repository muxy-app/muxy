import Foundation
import Testing

@testable import Muxy

@Suite("HookConfigWatcher")
struct HookConfigWatcherTests {
    private static let watched: Set<String> = [
        "/home/.claude/settings.json",
        "/home/.codex/hooks.json",
    ]

    @Test("matches an exact watched config path")
    func matchesExactPath() {
        #expect(HookConfigWatcher.isRelevantChange(path: "/home/.claude/settings.json", watchedPaths: Self.watched))
    }

    @Test("matches atomic write temp files sharing the config prefix")
    func matchesAtomicWriteSibling() {
        #expect(HookConfigWatcher.isRelevantChange(
            path: "/home/.codex/hooks.json.tmp",
            watchedPaths: Self.watched
        ))
    }

    @Test("ignores muxy backup files")
    func ignoresBackupFiles() {
        #expect(!HookConfigWatcher.isRelevantChange(
            path: "/home/.claude/settings.json.muxy-backup",
            watchedPaths: Self.watched
        ))
    }

    @Test("ignores unrelated files in the same directory")
    func ignoresUnrelatedFiles() {
        #expect(!HookConfigWatcher.isRelevantChange(
            path: "/home/.claude/other.json",
            watchedPaths: Self.watched
        ))
    }

    @Test("returns nil when no directories can be derived")
    func initReturnsNilWithoutDirectories() {
        let watcher = HookConfigWatcher(configPaths: []) {}
        #expect(watcher == nil)
    }

    @Test("debounce coalesces rapid signals into a single handler call")
    func debounceCoalescesRapidSignals() {
        let scheduler = ManualScheduler()
        let counter = Counter()
        let trigger = DebouncedTrigger(interval: 0.4, scheduler: scheduler, handler: { counter.increment() })

        trigger.signal()
        trigger.signal()
        trigger.signal()

        #expect(counter.value == 0)
        #expect(scheduler.cancelledCount == 2)
        scheduler.fireLatest()
        #expect(counter.value == 1)
    }

    @Test("a signal after the handler fires schedules a fresh handler call")
    func debounceReArmsAfterFiring() {
        let scheduler = ManualScheduler()
        let counter = Counter()
        let trigger = DebouncedTrigger(interval: 0.4, scheduler: scheduler, handler: { counter.increment() })

        trigger.signal()
        scheduler.fireLatest()
        #expect(counter.value == 1)

        trigger.signal()
        scheduler.fireLatest()
        #expect(counter.value == 2)
    }

    private final class ManualScheduler: DebounceScheduler, @unchecked Sendable {
        private let lock = NSLock()
        private var pending: [ManualCancellable] = []
        private var cancelled = 0

        var cancelledCount: Int { lock.withLock { cancelled } }

        func schedule(after _: TimeInterval, _ work: @escaping @Sendable () -> Void) -> DebounceCancellable {
            let cancellable = ManualCancellable(work: work) { [weak self] in
                self?.lock.withLock { self?.cancelled += 1 }
            }
            lock.withLock { pending.append(cancellable) }
            return cancellable
        }

        func fireLatest() {
            let work: (@Sendable () -> Void)? = lock.withLock {
                pending.last(where: { !$0.isCancelled })?.work
            }
            work?()
        }

        final class ManualCancellable: DebounceCancellable {
            let work: @Sendable () -> Void
            private let onCancel: @Sendable () -> Void
            private(set) var isCancelled = false

            init(work: @escaping @Sendable () -> Void, onCancel: @escaping @Sendable () -> Void) {
                self.work = work
                self.onCancel = onCancel
            }

            func cancel() {
                isCancelled = true
                onCancel()
            }
        }
    }

    private final class Counter: @unchecked Sendable {
        private let lock = NSLock()
        private var storage = 0
        var value: Int { lock.withLock { storage } }
        func increment() { lock.withLock { storage += 1 } }
    }
}
