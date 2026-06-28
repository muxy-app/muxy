import Foundation
import Testing

@testable import Muxy

@Suite("ProcessUsage")
struct ProcessUsageTests {
    private func row(_ pid: pid_t, cpu: Double?, mem: UInt64, group: ProcessGroup) -> ProcessUsageRow {
        ProcessUsageRow(pid: pid, name: "p\(pid)", cpuPercent: cpu, memoryBytes: mem, group: group)
    }

    @Test("compactCPU formats nil as em dash and rounds percent")
    func cpuFormatting() {
        #expect(ProcessUsageFormat.compactCPU(nil) == "—")
        #expect(ProcessUsageFormat.compactCPU(12.4) == "12%")
        #expect(ProcessUsageFormat.compactCPU(12.6) == "13%")
    }

    @Test("compactMemory uses single-letter units with minimal width")
    func compactMemoryFormatting() {
        #expect(ProcessUsageFormat.compactMemory(512) == "512B")
        #expect(ProcessUsageFormat.compactMemory(2048) == "2.0K")
        #expect(ProcessUsageFormat.compactMemory(15 * 1024) == "15K")
        #expect(ProcessUsageFormat.compactMemory(1_572_864) == "1.5M")
        #expect(ProcessUsageFormat.compactMemory(750 * 1_048_576) == "750M")
        #expect(ProcessUsageFormat.compactMemory(3 * 1_073_741_824) == "3.0G")
    }

    @Test("snapshot sorts by group then cpu then memory")
    func snapshotSorting() {
        let rows = [
            row(3, cpu: 5, mem: 100, group: .orphan),
            row(1, cpu: 1, mem: 100, group: .app),
            row(2, cpu: 9, mem: 100, group: .extensionHost),
            row(4, cpu: 9, mem: 500, group: .extensionHost),
        ]
        let snapshot = ProcessUsageSnapshot.make(from: rows)
        #expect(snapshot.rows.map(\.pid) == [1, 4, 2, 3])
    }

    @Test("totalCPU is nil when every row is cold, otherwise the sum of known values")
    func totalCPUAggregation() {
        let cold = ProcessUsageSnapshot.make(from: [row(1, cpu: nil, mem: 0, group: .app)])
        #expect(cold.totalCPUPercent == nil)

        let warm = ProcessUsageSnapshot.make(from: [
            row(1, cpu: 4, mem: 0, group: .app),
            row(2, cpu: nil, mem: 0, group: .app),
            row(3, cpu: 6, mem: 0, group: .app),
        ])
        #expect(warm.totalCPUPercent == 10)
    }

    @Test("totalMemory sums RSS across rows")
    func totalMemoryAggregation() {
        let snapshot = ProcessUsageSnapshot.make(from: [
            row(1, cpu: nil, mem: 100, group: .app),
            row(2, cpu: nil, mem: 250, group: .extensionHost),
        ])
        #expect(snapshot.totalMemoryBytes == 350)
    }

    @Test("rows(in:) filters by group")
    func rowsByGroup() {
        let snapshot = ProcessUsageSnapshot.make(from: [
            row(1, cpu: nil, mem: 0, group: .app),
            row(2, cpu: nil, mem: 0, group: .orphan),
        ])
        #expect(snapshot.rows(in: .orphan).map(\.pid) == [2])
        #expect(snapshot.rows(in: .extensionHost).isEmpty)
    }

    @Test("cpuPercentForTesting computes the delta ratio")
    func cpuDeltaMath() {
        let value = ProcessResourceMonitor.cpuPercentForTesting(
            prevNanos: 0, nowNanos: 500_000_000, prevWall: 0, nowWall: 1_000_000_000
        )
        #expect(value == 50)
    }

    @Test("cpuPercentForTesting returns nil on zero wall delta")
    func cpuZeroWall() {
        #expect(ProcessResourceMonitor.cpuPercentForTesting(prevNanos: 0, nowNanos: 1, prevWall: 5, nowWall: 5) == nil)
    }

    @Test("cpuPercentForTesting returns nil when the cpu counter decreases")
    func cpuCounterReset() {
        #expect(ProcessResourceMonitor.cpuPercentForTesting(prevNanos: 10, nowNanos: 5, prevWall: 0, nowWall: 10) == nil)
    }
}
