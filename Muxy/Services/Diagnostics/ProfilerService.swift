import Foundation
import Observation

@MainActor
@Observable
final class ProfilerService {
    static let shared = ProfilerService()
    static let enabledKey = "diagnostics.profiler.enabled"

    private(set) var isEnabled: Bool
    let fileURL: URL

    @ObservationIgnored private let defaults: UserDefaults
    @ObservationIgnored private let recorder: any ProfilerRecording
    @ObservationIgnored private var recorderStarted = false

    convenience init() {
        let fileURL = Self.defaultFileURL
        let writer = ProfilerJSONLWriter(fileURL: fileURL)
        let recorder = DefaultProfilerRecorder(writer: writer)
        self.init(defaults: .standard, recorder: recorder)
    }

    init(defaults: UserDefaults, recorder: any ProfilerRecording) {
        self.defaults = defaults
        self.recorder = recorder
        isEnabled = defaults.bool(forKey: Self.enabledKey)
        fileURL = recorder.fileURL
    }

    func configure() {
        isEnabled = defaults.bool(forKey: Self.enabledKey)
        reconcileRecorder()
    }

    func setEnabled(_ enabled: Bool) {
        defaults.set(enabled, forKey: Self.enabledKey)
        isEnabled = enabled
        reconcileRecorder()
    }

    private func reconcileRecorder() {
        if isEnabled {
            guard !recorderStarted else { return }
            recorder.start()
            recorderStarted = true
            return
        }

        guard recorderStarted else { return }
        recorder.stop()
        recorderStarted = false
    }

    private static var defaultFileURL: URL {
        let applicationSupport = FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        )[0]
        return applicationSupport
            .appendingPathComponent("Muxy", isDirectory: true)
            .appendingPathComponent("Diagnostics", isDirectory: true)
            .appendingPathComponent("profiler.jsonl", isDirectory: false)
    }
}
