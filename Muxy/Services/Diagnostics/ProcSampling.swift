import Darwin
import Foundation

enum ProcSampling {
    private struct BSDInfo {
        let ppid: pid_t
        let pgid: pid_t
        let comm: String
    }

    private struct Usage {
        let footprintBytes: UInt64
        let cpuNanos: UInt64
    }

    static func topology() -> [ProcTopology] {
        listAllPIDs().compactMap(topology(for:))
    }

    static func enrich(_ topology: ProcTopology) -> ProcSnapshot? {
        guard let usage = usage(for: topology.pid) else { return nil }
        return ProcSnapshot(
            pid: topology.pid,
            ppid: topology.ppid,
            pgid: topology.pgid,
            name: name(topology.pid, fallback: topology.comm),
            memoryBytes: usage.footprintBytes,
            cpuTimeNanos: usage.cpuNanos
        )
    }

    private static func topology(for pid: pid_t) -> ProcTopology? {
        guard pid > 0, let bsd = bsdInfo(pid) else { return nil }
        return ProcTopology(pid: pid, ppid: bsd.ppid, pgid: bsd.pgid, comm: bsd.comm)
    }

    static func listAllPIDs() -> [pid_t] {
        let needed = proc_listpids(UInt32(PROC_ALL_PIDS), 0, nil, 0)
        guard needed > 0 else { return [] }
        let capacity = Int(needed) / MemoryLayout<pid_t>.stride + 16
        var pids = [pid_t](repeating: 0, count: capacity)
        let written = proc_listpids(UInt32(PROC_ALL_PIDS), 0, &pids, Int32(capacity * MemoryLayout<pid_t>.stride))
        guard written > 0 else { return [] }
        let count = Int(written) / MemoryLayout<pid_t>.stride
        return Array(pids.prefix(count)).filter { $0 > 0 }
    }

    private static func usage(for pid: pid_t) -> Usage? {
        var info = rusage_info_v4()
        let result = withUnsafeMutablePointer(to: &info) { pointer -> Int32 in
            pointer.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) { rebound in
                proc_pid_rusage(pid, RUSAGE_INFO_V4, rebound)
            }
        }
        guard result == 0 else { return nil }
        return Usage(footprintBytes: info.ri_phys_footprint, cpuNanos: info.ri_user_time &+ info.ri_system_time)
    }

    private static func bsdInfo(_ pid: pid_t) -> BSDInfo? {
        var info = proc_bsdinfo()
        let size = Int32(MemoryLayout<proc_bsdinfo>.size)
        let result = proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, size)
        guard result == size else { return nil }
        let comm = withUnsafeBytes(of: info.pbi_comm) { raw -> String in
            let bytes = Array(raw.prefix { $0 != 0 })
            return String(bytes: bytes, encoding: .utf8) ?? ""
        }
        return BSDInfo(ppid: pid_t(info.pbi_ppid), pgid: pid_t(info.pbi_pgid), comm: comm)
    }

    private static func name(_ pid: pid_t, fallback: String) -> String {
        var buffer = [UInt8](repeating: 0, count: 1024)
        let length = proc_name(Int32(pid), &buffer, UInt32(buffer.count))
        if length > 0 {
            let bytes = Array(buffer.prefix { $0 != 0 })
            if let name = String(bytes: bytes, encoding: .utf8), !name.isEmpty {
                return name
            }
        }
        return fallback.isEmpty ? "pid \(pid)" : fallback
    }
}
