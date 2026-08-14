import Foundation
import Testing

@testable import Muxy

@Suite("ProfilerService")
@MainActor
struct ProfilerServiceTests {
    @Test("profiling stays disabled when no preference exists")
    func profilingIsDisabledByDefault() {
        let suiteName = "muxy.tests.profiler.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let recorder = FakeProfilerRecorder()
        let service = ProfilerService(defaults: defaults, recorder: recorder)

        service.configure()
        service.configure()

        #expect(!service.isEnabled)
        #expect(defaults.object(forKey: ProfilerService.enabledKey) == nil)
        #expect(recorder.startCount == 0)
        #expect(recorder.stopCount == 0)
    }

    @Test("configure and setEnabled persist and reconcile recorder state")
    func enableStatePersistenceAndRecorderLifecycle() {
        let suiteName = "muxy.tests.profiler.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }
        defaults.set(true, forKey: ProfilerService.enabledKey)
        let recorder = FakeProfilerRecorder()
        let service = ProfilerService(defaults: defaults, recorder: recorder)

        #expect(service.isEnabled)
        #expect(service.fileURL == recorder.fileURL)
        #expect(recorder.startCount == 0)

        service.configure()
        service.configure()
        #expect(recorder.startCount == 1)

        service.setEnabled(false)
        service.setEnabled(false)
        #expect(!service.isEnabled)
        #expect(!defaults.bool(forKey: ProfilerService.enabledKey))
        #expect(recorder.stopCount == 1)

        service.setEnabled(true)
        service.setEnabled(true)
        #expect(service.isEnabled)
        #expect(defaults.bool(forKey: ProfilerService.enabledKey))
        #expect(recorder.startCount == 2)
    }
}

private final class FakeProfilerRecorder: ProfilerRecording {
    let fileURL = URL(fileURLWithPath: "/tmp/muxy-profiler-test.jsonl")
    private(set) var startCount = 0
    private(set) var stopCount = 0

    func start() {
        startCount += 1
    }

    func stop() {
        stopCount += 1
    }
}
