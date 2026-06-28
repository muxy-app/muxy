import Darwin
import Foundation
import Observation

@MainActor
@Observable
final class ProcessResourceMonitor {
    static let shared = ProcessResourceMonitor()

    private struct Baseline {
        let cpuNanos: UInt64
        let wallNanos: UInt64
    }

    private static let samplingInterval: TimeInterval = 2
    nonisolated private static let rootPID = getpid()

    private(set) var snapshot: ProcessUsageSnapshot = .empty

    private let queue = DispatchQueue(label: "app.muxy.resource-monitor", qos: .utility)
    private var timer: DispatchSourceTimer?
    private var observerCount = 0
    private var isSampling = false
    private var previous: [pid_t: Baseline] = [:]

    private init() {}

    func beginObserving() {
        observerCount += 1
        guard observerCount == 1 else { return }
        startTimer()
        refreshNow()
    }

    func endObserving() {
        guard observerCount > 0 else { return }
        observerCount -= 1
        guard observerCount == 0 else { return }
        stopTimer()
        isSampling = false
        previous = [:]
        snapshot = .empty
    }

    func refreshNow() {
        guard !isSampling else { return }
        isSampling = true
        let hostPIDs = ExtensionStore.shared.spawnedHostPIDs()
        let hostPGIDs = ExtensionStore.shared.spawnedHostPGIDs
        queue.async { [weak self] in
            let topology = ProcSampling.topology()
            let groups = ProcessTree.classify(
                rootPID: Self.rootPID,
                procs: topology,
                knownHostPIDs: hostPIDs,
                knownHostPGIDs: hostPGIDs,
                hostBinaryName: ExtensionHostLocator.binaryName
            )
            let procs = topology
                .filter { groups[$0.pid] != nil }
                .compactMap(ProcSampling.enrich)
            let wallNanos = clock_gettime_nsec_np(CLOCK_UPTIME_RAW)
            Task { @MainActor [weak self] in
                self?.apply(procs: procs, groups: groups, wallNanos: wallNanos)
            }
        }
    }

    private func startTimer() {
        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now() + Self.samplingInterval, repeating: Self.samplingInterval)
        timer.setEventHandler { [weak self] in
            MainActor.assumeIsolated {
                self?.refreshNow()
            }
        }
        timer.resume()
        self.timer = timer
    }

    private func stopTimer() {
        timer?.cancel()
        timer = nil
    }

    private func apply(procs: [ProcSnapshot], groups: [pid_t: ProcessGroup], wallNanos: UInt64) {
        defer { isSampling = false }

        var rows: [ProcessUsageRow] = []
        var nextPrevious: [pid_t: Baseline] = [:]
        for proc in procs {
            guard let group = groups[proc.pid] else { continue }
            let baseline = previous[proc.pid]
            let cpu = Self.cpuPercent(
                prevNanos: baseline?.cpuNanos,
                nowNanos: proc.cpuTimeNanos,
                prevWall: baseline?.wallNanos,
                nowWall: wallNanos
            )
            nextPrevious[proc.pid] = Baseline(cpuNanos: proc.cpuTimeNanos, wallNanos: wallNanos)
            rows.append(ProcessUsageRow(
                pid: proc.pid,
                name: proc.name,
                cpuPercent: cpu,
                memoryBytes: proc.memoryBytes,
                group: group
            ))
        }

        previous = nextPrevious
        snapshot = .make(from: rows)
    }

    private static func cpuPercent(prevNanos: UInt64?, nowNanos: UInt64, prevWall: UInt64?, nowWall: UInt64) -> Double? {
        guard let prevNanos, let prevWall else { return nil }
        return cpuPercentForTesting(prevNanos: prevNanos, nowNanos: nowNanos, prevWall: prevWall, nowWall: nowWall)
    }

    nonisolated static func cpuPercentForTesting(prevNanos: UInt64, nowNanos: UInt64, prevWall: UInt64, nowWall: UInt64) -> Double? {
        guard nowWall > prevWall, nowNanos >= prevNanos else { return nil }
        let cpuDelta = Double(nowNanos - prevNanos)
        let wallDelta = Double(nowWall - prevWall)
        return cpuDelta / wallDelta * 100
    }
}
