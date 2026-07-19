import Darwin

public enum AncestorProcessInspector {
    static let maximumDepth = 64

    public static func ancestorPIDs(startingAt processID: Int32 = getppid()) -> [Int32] {
        ancestorPIDs(startingAt: processID, parentPID: parentPID)
    }

    public static func ancestorPIDs(
        startingAt processID: Int32,
        parentPID: (Int32) -> Int32?
    ) -> [Int32] {
        var ancestors: [Int32] = []
        var visited: Set<Int32> = []
        var current = processID

        while current > 0, ancestors.count < maximumDepth, visited.insert(current).inserted {
            ancestors.append(current)
            guard current != 1, let parent = parentPID(current), parent > 0 else { break }
            current = parent
        }

        return ancestors
    }

    private static func parentPID(of processID: Int32) -> Int32? {
        var process = kinfo_proc()
        var size = MemoryLayout<kinfo_proc>.size
        var mib = [CTL_KERN, KERN_PROC, KERN_PROC_PID, processID]
        guard sysctl(&mib, u_int(mib.count), &process, &size, nil, 0) == 0,
              size == MemoryLayout<kinfo_proc>.size
        else { return nil }
        return process.kp_eproc.e_ppid
    }
}
