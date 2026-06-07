import Foundation

enum ProcessGroup: String, CaseIterable {
    case app
    case extensionHost
    case orphan

    var title: String {
        switch self {
        case .app: "App"
        case .extensionHost: "Extensions"
        case .orphan: "Orphans"
        }
    }
}

struct ProcessUsageRow: Identifiable, Equatable {
    let pid: pid_t
    let name: String
    let cpuPercent: Double?
    let memoryBytes: UInt64
    let group: ProcessGroup

    var id: pid_t { pid }
}

struct ProcessUsageSnapshot: Equatable {
    let rows: [ProcessUsageRow]
    let totalCPUPercent: Double?
    let totalMemoryBytes: UInt64

    static let empty = ProcessUsageSnapshot(rows: [], totalCPUPercent: nil, totalMemoryBytes: 0)

    func rows(in group: ProcessGroup) -> [ProcessUsageRow] {
        rows.filter { $0.group == group }
    }

    static func make(from rows: [ProcessUsageRow]) -> ProcessUsageSnapshot {
        let sorted = rows.sorted { lhs, rhs in
            if lhs.group != rhs.group {
                return groupOrder(lhs.group) < groupOrder(rhs.group)
            }
            if (lhs.cpuPercent ?? 0) != (rhs.cpuPercent ?? 0) {
                return (lhs.cpuPercent ?? 0) > (rhs.cpuPercent ?? 0)
            }
            return lhs.memoryBytes > rhs.memoryBytes
        }
        let cpuValues = rows.compactMap(\.cpuPercent)
        let totalCPU = cpuValues.isEmpty ? nil : cpuValues.reduce(0, +)
        let totalMemory = rows.reduce(UInt64(0)) { $0 + $1.memoryBytes }
        return ProcessUsageSnapshot(rows: sorted, totalCPUPercent: totalCPU, totalMemoryBytes: totalMemory)
    }

    private static func groupOrder(_ group: ProcessGroup) -> Int {
        switch group {
        case .app: 0
        case .extensionHost: 1
        case .orphan: 2
        }
    }
}

enum ProcessUsageFormat {
    static func compactMemory(_ bytes: UInt64) -> String {
        let units = ["B", "K", "M", "G", "T"]
        var value = Double(bytes)
        var unit = 0
        while value >= 1024, unit < units.count - 1 {
            value /= 1024
            unit += 1
        }
        if unit == 0 {
            return "\(Int(value))\(units[unit])"
        }
        let format = value < 10 ? "%.1f%@" : "%.0f%@"
        return String(format: format, value, units[unit])
    }

    static func compactCPU(_ percent: Double?) -> String {
        guard let percent else { return "—" }
        return "\(Int(percent.rounded()))%"
    }
}
