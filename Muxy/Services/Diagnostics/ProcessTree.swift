import Foundation

struct ProcTopology: Equatable {
    let pid: pid_t
    let ppid: pid_t
    let pgid: pid_t
    let comm: String
}

struct ProcSnapshot: Equatable {
    let pid: pid_t
    let ppid: pid_t
    let pgid: pid_t
    let name: String
    let memoryBytes: UInt64
    let cpuTimeNanos: UInt64
}

enum ProcessTree {
    static func descendants(of root: pid_t, in procs: [ProcTopology]) -> Set<pid_t> {
        var childrenByParent: [pid_t: [pid_t]] = [:]
        for proc in procs where proc.pid != proc.ppid {
            childrenByParent[proc.ppid, default: []].append(proc.pid)
        }

        var result: Set<pid_t> = [root]
        var queue = [root]
        while let current = queue.popLast() {
            for child in childrenByParent[current] ?? [] where !result.contains(child) {
                result.insert(child)
                queue.append(child)
            }
        }
        return result
    }

    static func classify(
        rootPID: pid_t,
        procs: [ProcTopology],
        knownHostPIDs: Set<pid_t>,
        knownHostPGIDs: Set<pid_t>,
        hostBinaryName: String
    ) -> [pid_t: ProcessGroup] {
        let byPID = Dictionary(procs.map { ($0.pid, $0) }, uniquingKeysWith: { first, _ in first })
        let subtree = descendants(of: rootPID, in: procs)

        var groups: [pid_t: ProcessGroup] = [:]
        for pid in subtree {
            groups[pid] = hasHostAncestor(pid: pid, rootPID: rootPID, byPID: byPID, knownHostPIDs: knownHostPIDs)
                ? .extensionHost
                : .app
        }

        for proc in procs where isOrphan(proc, subtree: subtree, knownHostPGIDs: knownHostPGIDs, hostBinaryName: hostBinaryName) {
            groups[proc.pid] = .orphan
        }

        return groups
    }

    private static func isOrphan(
        _ proc: ProcTopology,
        subtree: Set<pid_t>,
        knownHostPGIDs: Set<pid_t>,
        hostBinaryName: String
    ) -> Bool {
        !subtree.contains(proc.pid)
            && knownHostPGIDs.contains(proc.pgid)
            && hostBinaryName.hasPrefix(proc.comm)
            && !proc.comm.isEmpty
    }

    private static func hasHostAncestor(
        pid: pid_t,
        rootPID: pid_t,
        byPID: [pid_t: ProcTopology],
        knownHostPIDs: Set<pid_t>
    ) -> Bool {
        var current: pid_t? = pid
        var visited: Set<pid_t> = []
        while let value = current, value != rootPID, visited.insert(value).inserted {
            if knownHostPIDs.contains(value) {
                return true
            }
            current = byPID[value]?.ppid
        }
        return false
    }
}
