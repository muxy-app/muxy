import Darwin
import Foundation
import os

private let profilerLogger = Logger(subsystem: "app.muxy", category: "Profiler")

protocol ProfilerRecording: AnyObject {
    var fileURL: URL { get }
    func start()
    func stop()
}

protocol ProfilerClock: Sendable {
    func now() -> Date
    func monotonicNanoseconds() -> UInt64
}

protocol ProfilerProcessSampling: Sendable {
    func sample() throws -> ProfilerProcessMeasurement
}

protocol ProfilerSessionProviding: Sendable {
    func session(timestamp: Date, samplingInterval: TimeInterval) -> ProfilerSessionRecord
}

protocol ProfilerScheduledTask: Sendable {
    func cancel()
}

protocol ProfilerScheduling: Sendable {
    func scheduleRepeating(
        every interval: TimeInterval,
        action: @escaping @Sendable () -> Void
    ) -> any ProfilerScheduledTask
    func scheduleOnce(
        after delay: TimeInterval,
        action: @escaping @Sendable () -> Void
    ) -> any ProfilerScheduledTask
}

struct SystemProfilerClock: ProfilerClock {
    func now() -> Date {
        Date()
    }

    func monotonicNanoseconds() -> UInt64 {
        clock_gettime_nsec_np(CLOCK_UPTIME_RAW)
    }
}

struct CurrentProcessProfilerSampler: ProfilerProcessSampling {
    func sample() throws -> ProfilerProcessMeasurement {
        var info = rusage_info_v4()
        let result = withUnsafeMutablePointer(to: &info) { pointer -> Int32 in
            pointer.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) { rebound in
                proc_pid_rusage(getpid(), RUSAGE_INFO_V4, rebound)
            }
        }
        guard result == 0 else {
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }
        return ProfilerProcessMeasurement(
            cumulativeCPUNanoseconds: info.ri_user_time &+ info.ri_system_time,
            physicalFootprintBytes: info.ri_phys_footprint
        )
    }
}

struct BundleProfilerSessionProvider: ProfilerSessionProviding {
    private let bundle: Bundle
    private let processInfo: ProcessInfo

    init(bundle: Bundle = .main, processInfo: ProcessInfo = .processInfo) {
        self.bundle = bundle
        self.processInfo = processInfo
    }

    func session(timestamp: Date, samplingInterval: TimeInterval) -> ProfilerSessionRecord {
        let version = bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "unknown"
        let build = bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "unknown"
        let operatingSystem = processInfo.operatingSystemVersion
        let macOSVersion = "\(operatingSystem.majorVersion).\(operatingSystem.minorVersion).\(operatingSystem.patchVersion)"
        return ProfilerSessionRecord(
            timestamp: timestamp,
            appVersion: version,
            appBuild: build,
            macOSVersion: macOSVersion,
            architecture: Self.architecture,
            samplingIntervalSeconds: samplingInterval
        )
    }

    private static var architecture: String {
        #if arch(arm64)
        "arm64"
        #elseif arch(x86_64)
        "x86_64"
        #else
        "unknown"
        #endif
    }
}

struct DispatchProfilerScheduler: ProfilerScheduling {
    private let queue: DispatchQueue

    init(queue: DispatchQueue) {
        self.queue = queue
    }

    func scheduleRepeating(
        every interval: TimeInterval,
        action: @escaping @Sendable () -> Void
    ) -> any ProfilerScheduledTask {
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(
            deadline: .now() + interval,
            repeating: interval,
            leeway: .seconds(1)
        )
        return activate(timer, action: action)
    }

    func scheduleOnce(
        after delay: TimeInterval,
        action: @escaping @Sendable () -> Void
    ) -> any ProfilerScheduledTask {
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + delay, leeway: .seconds(1))
        return activate(timer, action: action)
    }

    private func activate(
        _ timer: DispatchSourceTimer,
        action: @escaping @Sendable () -> Void
    ) -> any ProfilerScheduledTask {
        timer.setEventHandler(handler: action)
        timer.resume()
        return DispatchProfilerScheduledTask(timer: timer)
    }
}

private final class DispatchProfilerScheduledTask: ProfilerScheduledTask, @unchecked Sendable {
    private let timer: DispatchSourceTimer

    init(timer: DispatchSourceTimer) {
        self.timer = timer
    }

    func cancel() {
        timer.cancel()
    }
}

final class DefaultProfilerRecorder: ProfilerRecording, @unchecked Sendable {
    let fileURL: URL

    private let samplingInterval: TimeInterval
    private let queue: DispatchQueue
    private let clock: any ProfilerClock
    private let sampler: any ProfilerProcessSampling
    private let writer: any ProfilerRecordWriting
    private let sessionProvider: any ProfilerSessionProviding
    private let scheduler: any ProfilerScheduling

    private var timer: (any ProfilerScheduledTask)?
    private var session: ProfilerSessionRecord?
    private var sampleBuilder: ProfilerSampleBuilder?
    private var isRunning = false

    init(
        samplingInterval: TimeInterval = 60,
        queue: DispatchQueue = DispatchQueue(label: "app.muxy.profiler", qos: .utility),
        clock: any ProfilerClock = SystemProfilerClock(),
        sampler: any ProfilerProcessSampling = CurrentProcessProfilerSampler(),
        writer: any ProfilerRecordWriting,
        sessionProvider: any ProfilerSessionProviding = BundleProfilerSessionProvider(),
        scheduler: (any ProfilerScheduling)? = nil
    ) {
        self.samplingInterval = samplingInterval
        self.queue = queue
        self.clock = clock
        self.sampler = sampler
        self.writer = writer
        self.sessionProvider = sessionProvider
        self.scheduler = scheduler ?? DispatchProfilerScheduler(queue: queue)
        fileURL = writer.fileURL
    }

    func start() {
        queue.async { [weak self] in
            self?.startOnQueue()
        }
    }

    func stop() {
        queue.async { [weak self] in
            self?.stopOnQueue()
        }
    }

    private func startOnQueue() {
        guard !isRunning else { return }
        timer?.cancel()
        timer = nil
        isRunning = true

        do {
            let sessionStart = clock.monotonicNanoseconds()
            let currentSession = sessionProvider.session(
                timestamp: clock.now(),
                samplingInterval: samplingInterval
            )
            try writer.startSession(currentSession)
            let baselineMeasurement = try sampler.sample()
            sampleBuilder = ProfilerSampleBuilder(
                sessionStartNanoseconds: sessionStart,
                baselineMeasurement: baselineMeasurement,
                baselineMonotonicNanoseconds: clock.monotonicNanoseconds()
            )
            session = currentSession
            startTimer()
            profilerLogger.info("Profiler started")
        } catch {
            scheduleRetryOnQueue(after: error)
        }
    }

    private func startTimer() {
        timer = scheduler.scheduleRepeating(every: samplingInterval) { [weak self] in
            self?.recordSampleOnQueue()
        }
    }

    private func recordSampleOnQueue() {
        guard isRunning, let session, var sampleBuilder else { return }

        do {
            let measurement = try sampler.sample()
            let sample = try sampleBuilder.makeSample(
                measurement: measurement,
                timestamp: clock.now(),
                monotonicNanoseconds: clock.monotonicNanoseconds()
            )
            try writer.append(sample, session: session)
            self.sampleBuilder = sampleBuilder
        } catch {
            scheduleRetryOnQueue(after: error)
        }
    }

    private func stopOnQueue() {
        let wasActive = isRunning || timer != nil
        timer?.cancel()
        timer = nil
        isRunning = false
        session = nil
        sampleBuilder = nil
        if wasActive {
            profilerLogger.info("Profiler stopped")
        }
    }

    private func scheduleRetryOnQueue(after error: Error) {
        timer?.cancel()
        timer = nil
        isRunning = false
        session = nil
        sampleBuilder = nil

        timer = scheduler.scheduleOnce(after: samplingInterval) { [weak self] in
            self?.startOnQueue()
        }
        profilerLogger.error("Profiler will retry after a sampling or write failure: \(error.localizedDescription, privacy: .public)")
    }
}
