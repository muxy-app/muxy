import Darwin
import Foundation

struct TTYProcessEntry: Equatable, Sendable {
    let processID: Int32
    let processGroupID: Int32
    let foregroundGroupID: Int32
}

enum TerminalTTYForegroundProcess {
    static func select(from entries: [TTYProcessEntry]) -> Int32? {
        guard let group = entries.lazy.map(\.foregroundGroupID).first(where: { $0 > 0 }) else { return nil }
        if entries.contains(where: { $0.processID == group }) {
            return group
        }
        return entries.filter { $0.processGroupID == group }.map(\.processID).min()
    }

    static func isRunningCommand(foregroundProcessID: Int32?, shellProcessID: Int32) -> Bool {
        guard let foregroundProcessID, foregroundProcessID > 0, shellProcessID > 0 else { return false }
        return foregroundProcessID != shellProcessID
    }

    static func processID(ttyDevice: UInt64) -> Int32? {
        select(from: entries(ttyDevice: ttyDevice))
    }

    static func entries(ttyDevice: UInt64) -> [TTYProcessEntry] {
        guard ttyDevice != 0 else { return [] }
        var name: [Int32] = [
            CTL_KERN,
            KERN_PROC,
            KERN_PROC_TTY,
            Int32(bitPattern: UInt32(truncatingIfNeeded: ttyDevice)),
        ]
        var size = 0
        guard sysctl(&name, 4, nil, &size, nil, 0) == 0, size > 0 else { return [] }

        let stride = MemoryLayout<kinfo_proc>.stride
        var records = [kinfo_proc](repeating: kinfo_proc(), count: size / stride)
        guard sysctl(&name, 4, &records, &size, nil, 0) == 0 else { return [] }

        return records.prefix(size / stride).map { record in
            TTYProcessEntry(
                processID: record.kp_proc.p_pid,
                processGroupID: record.kp_eproc.e_pgid,
                foregroundGroupID: record.kp_eproc.e_tpgid
            )
        }
    }
}
