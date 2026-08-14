import Foundation
import Testing

@testable import Muxy

@Suite("Default profiler recorder")
struct ProfilerRecorderTests {
    @Test("start is idempotent and a scheduled tick records a sample")
    func recordsScheduledSample() throws {
        let sessionDate = Date(timeIntervalSince1970: 1_000)
        let sampleDate = Date(timeIntervalSince1970: 1_060)
        let harness = makeHarness(
            measurements: [
                .success(measurement(cpu: 10_000_000_000, memory: 100)),
                .success(measurement(cpu: 22_000_000_000, memory: 200)),
            ],
            dates: [sessionDate, sampleDate],
            monotonicNanoseconds: [1_000_000_000, 1_000_000_000, 61_000_000_000]
        )

        harness.recorder.start()
        harness.recorder.start()
        harness.drainQueue()

        #expect(harness.writer.sessions.count == 1)
        #expect(harness.writer.sessions.first?.timestamp == sessionDate)
        #expect(harness.writer.sessions.first?.samplingIntervalSeconds == 60)
        #expect(harness.sampler.sampleCount == 1)
        #expect(harness.scheduler.tasks.count == 1)
        #expect(harness.scheduler.tasks.first?.repeatingInterval == 60)

        try harness.fireLatestTimer()

        let sample = try #require(harness.writer.samples.first)
        #expect(harness.writer.samples.count == 1)
        #expect(sample.timestamp == sampleDate)
        #expect(sample.profilerUptimeSeconds == 60)
        #expect(sample.cpuPercent == 20)
        #expect(sample.physicalFootprintBytes == 200)
        #expect(harness.writer.sampleSessions.first == harness.writer.sessions.first)
        #expect(harness.sampler.sampleCount == 2)

        harness.recorder.stop()
        harness.drainQueue()
    }

    @Test("stop cancels scheduled sampling")
    func stopCancelsSampling() throws {
        let harness = makeHarness(
            measurements: [.success(measurement(cpu: 10, memory: 100))],
            dates: [Date(timeIntervalSince1970: 1_000)],
            monotonicNanoseconds: [1_000_000_000, 1_000_000_000]
        )

        harness.recorder.start()
        harness.drainQueue()
        let timer = try #require(harness.scheduler.tasks.last)

        harness.recorder.stop()
        harness.drainQueue()
        harness.fire(timer)

        #expect(timer.isCancelled)
        #expect(harness.writer.samples.isEmpty)
        #expect(harness.sampler.sampleCount == 1)
    }

    @Test("sampling failure retries with a new session")
    func samplingFailureRetries() throws {
        let firstSessionDate = Date(timeIntervalSince1970: 1_000)
        let secondSessionDate = Date(timeIntervalSince1970: 1_060)
        let sampleDate = Date(timeIntervalSince1970: 1_120)
        let harness = makeHarness(
            measurements: [
                .success(measurement(cpu: 10_000_000_000, memory: 100)),
                .failure(.samplingFailed),
                .success(measurement(cpu: 20_000_000_000, memory: 200)),
                .success(measurement(cpu: 26_000_000_000, memory: 300)),
            ],
            dates: [firstSessionDate, secondSessionDate, sampleDate],
            monotonicNanoseconds: [
                1_000_000_000,
                1_000_000_000,
                61_000_000_000,
                61_000_000_000,
                121_000_000_000,
            ]
        )

        harness.recorder.start()
        harness.drainQueue()
        let failedSamplingTimer = try #require(harness.scheduler.tasks.last)
        harness.fire(failedSamplingTimer)

        #expect(failedSamplingTimer.isCancelled)
        #expect(harness.writer.sessions.count == 1)
        #expect(harness.writer.samples.isEmpty)
        let retryTimer = try #require(harness.scheduler.tasks.last)
        #expect(retryTimer.repeatingInterval == nil)
        #expect(retryTimer.delay == 60)

        harness.fire(retryTimer)

        #expect(harness.writer.sessions.map(\.timestamp) == [firstSessionDate, secondSessionDate])
        let resumedSamplingTimer = try #require(harness.scheduler.tasks.last)
        #expect(resumedSamplingTimer.repeatingInterval == 60)

        harness.fire(resumedSamplingTimer)

        let sample = try #require(harness.writer.samples.first)
        #expect(sample.timestamp == sampleDate)
        #expect(sample.profilerUptimeSeconds == 60)
        #expect(sample.cpuPercent == 10)
        #expect(sample.physicalFootprintBytes == 300)
        #expect(harness.sampler.sampleCount == 4)

        harness.recorder.stop()
        harness.drainQueue()
    }

    @Test("stop cancels a pending retry after a writing failure")
    func stopCancelsRetry() throws {
        let harness = makeHarness(
            measurements: [
                .success(measurement(cpu: 10, memory: 100)),
                .success(measurement(cpu: 20, memory: 200)),
            ],
            dates: [
                Date(timeIntervalSince1970: 1_000),
                Date(timeIntervalSince1970: 1_060),
            ],
            monotonicNanoseconds: [1_000_000_000, 1_000_000_000, 61_000_000_000],
            appendResults: [.failure(.writingFailed)]
        )

        harness.recorder.start()
        harness.drainQueue()
        try harness.fireLatestTimer()
        let retryTimer = try #require(harness.scheduler.tasks.last)

        harness.recorder.stop()
        harness.drainQueue()
        harness.fire(retryTimer)

        #expect(retryTimer.isCancelled)
        #expect(harness.writer.sessions.count == 1)
        #expect(harness.writer.samples.isEmpty)
        #expect(harness.sampler.sampleCount == 2)
    }

    private func makeHarness(
        measurements: [Result<ProfilerProcessMeasurement, ProfilerRecorderTestError>],
        dates: [Date],
        monotonicNanoseconds: [UInt64],
        appendResults: [Result<Void, ProfilerRecorderTestError>] = []
    ) -> ProfilerRecorderHarness {
        let queue = DispatchQueue(label: "app.muxy.profiler.tests.\(UUID().uuidString)")
        let clock = SequenceProfilerClock(dates: dates, monotonicNanoseconds: monotonicNanoseconds)
        let sampler = SequenceProfilerSampler(results: measurements)
        let writer = CapturingProfilerWriter(appendResults: appendResults)
        let scheduler = ManualProfilerScheduler()
        let recorder = DefaultProfilerRecorder(
            samplingInterval: 60,
            queue: queue,
            clock: clock,
            sampler: sampler,
            writer: writer,
            sessionProvider: TestProfilerSessionProvider(),
            scheduler: scheduler
        )
        return ProfilerRecorderHarness(
            recorder: recorder,
            queue: queue,
            sampler: sampler,
            writer: writer,
            scheduler: scheduler
        )
    }

    private func measurement(cpu: UInt64, memory: UInt64) -> ProfilerProcessMeasurement {
        ProfilerProcessMeasurement(cumulativeCPUNanoseconds: cpu, physicalFootprintBytes: memory)
    }
}

private struct ProfilerRecorderHarness {
    let recorder: DefaultProfilerRecorder
    let queue: DispatchQueue
    let sampler: SequenceProfilerSampler
    let writer: CapturingProfilerWriter
    let scheduler: ManualProfilerScheduler

    func drainQueue() {
        queue.sync {}
    }

    func fireLatestTimer() throws {
        let timer = try #require(scheduler.tasks.last)
        fire(timer)
    }

    func fire(_ timer: ManualProfilerScheduledTask) {
        queue.sync {
            timer.fire()
        }
    }
}

private enum ProfilerRecorderTestError: Error {
    case samplingFailed
    case writingFailed
}

private final class SequenceProfilerClock: ProfilerClock, @unchecked Sendable {
    private var dates: [Date]
    private var monotonicValues: [UInt64]

    init(dates: [Date], monotonicNanoseconds: [UInt64]) {
        self.dates = dates
        monotonicValues = monotonicNanoseconds
    }

    func now() -> Date {
        precondition(!dates.isEmpty)
        return dates.removeFirst()
    }

    func monotonicNanoseconds() -> UInt64 {
        precondition(!monotonicValues.isEmpty)
        return monotonicValues.removeFirst()
    }
}

private final class SequenceProfilerSampler: ProfilerProcessSampling, @unchecked Sendable {
    private var results: [Result<ProfilerProcessMeasurement, ProfilerRecorderTestError>]
    private(set) var sampleCount = 0

    init(results: [Result<ProfilerProcessMeasurement, ProfilerRecorderTestError>]) {
        self.results = results
    }

    func sample() throws -> ProfilerProcessMeasurement {
        precondition(!results.isEmpty)
        sampleCount += 1
        return try results.removeFirst().get()
    }
}

private struct TestProfilerSessionProvider: ProfilerSessionProviding {
    func session(timestamp: Date, samplingInterval: TimeInterval) -> ProfilerSessionRecord {
        ProfilerSessionRecord(
            timestamp: timestamp,
            appVersion: "1.2.3",
            appBuild: "456",
            macOSVersion: "15.4.0",
            architecture: "arm64",
            samplingIntervalSeconds: samplingInterval
        )
    }
}

private final class CapturingProfilerWriter: ProfilerRecordWriting, @unchecked Sendable {
    let fileURL = URL(fileURLWithPath: "/tmp/muxy-profiler-recorder-tests.jsonl")
    private(set) var sessions: [ProfilerSessionRecord] = []
    private(set) var samples: [ProfilerSampleRecord] = []
    private(set) var sampleSessions: [ProfilerSessionRecord] = []
    private var appendResults: [Result<Void, ProfilerRecorderTestError>]

    init(appendResults: [Result<Void, ProfilerRecorderTestError>]) {
        self.appendResults = appendResults
    }

    func startSession(_ session: ProfilerSessionRecord) throws {
        sessions.append(session)
    }

    func append(_ sample: ProfilerSampleRecord, session: ProfilerSessionRecord) throws {
        if !appendResults.isEmpty {
            try appendResults.removeFirst().get()
        }
        samples.append(sample)
        sampleSessions.append(session)
    }
}

private final class ManualProfilerScheduler: ProfilerScheduling, @unchecked Sendable {
    private(set) var tasks: [ManualProfilerScheduledTask] = []

    func scheduleRepeating(
        every interval: TimeInterval,
        action: @escaping @Sendable () -> Void
    ) -> any ProfilerScheduledTask {
        schedule(delay: interval, repeatingInterval: interval, action: action)
    }

    func scheduleOnce(
        after delay: TimeInterval,
        action: @escaping @Sendable () -> Void
    ) -> any ProfilerScheduledTask {
        schedule(delay: delay, repeatingInterval: nil, action: action)
    }

    private func schedule(
        delay: TimeInterval,
        repeatingInterval: TimeInterval?,
        action: @escaping @Sendable () -> Void
    ) -> ManualProfilerScheduledTask {
        let task = ManualProfilerScheduledTask(
            delay: delay,
            repeatingInterval: repeatingInterval,
            action: action
        )
        tasks.append(task)
        return task
    }
}

private final class ManualProfilerScheduledTask: ProfilerScheduledTask, @unchecked Sendable {
    let delay: TimeInterval
    let repeatingInterval: TimeInterval?
    private let action: @Sendable () -> Void
    private(set) var isCancelled = false

    init(
        delay: TimeInterval,
        repeatingInterval: TimeInterval?,
        action: @escaping @Sendable () -> Void
    ) {
        self.delay = delay
        self.repeatingInterval = repeatingInterval
        self.action = action
    }

    func fire() {
        guard !isCancelled else { return }
        if repeatingInterval == nil {
            isCancelled = true
        }
        action()
    }

    func cancel() {
        isCancelled = true
    }
}
