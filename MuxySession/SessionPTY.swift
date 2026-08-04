import Darwin
import MuxySessionProtocol

struct SessionProcess {
    let masterDescriptor: Int32
    let processID: pid_t
    let ttyDevice: UInt64
}

enum SessionPTY {
    private static let resetSignals: [Int32] = [SIGPIPE, SIGCHLD, SIGHUP, SIGINT, SIGQUIT, SIGTERM, SIGTSTP, SIGTTIN, SIGTTOU]
    private static let terminationPollMicroseconds: useconds_t = 10000
    private static let gracefulTerminationAttempts = 20
    private static let forcedTerminationAttempts = 50

    static func spawn(
        invocation: SessionShellInvocation,
        workingDirectory: String,
        columns: UInt16,
        rows: UInt16
    ) -> SessionProcess? {
        let argumentList = SessionCStringArray(invocation.arguments)
        let environmentList = SessionCStringArray(invocation.environment.map { "\($0.key)=\($0.value)" })
        guard let executable = strdup(invocation.executable) else { return nil }
        let directory = strdup(workingDirectory)
        defer {
            free(executable)
            free(directory)
        }

        var size = winsize(
            ws_row: rows == 0 ? 24 : rows,
            ws_col: columns == 0 ? 80 : columns,
            ws_xpixel: 0,
            ws_ypixel: 0
        )
        var master: Int32 = -1
        var name = [CChar](repeating: 0, count: Int(PATH_MAX))

        let processID = forkpty(&master, &name, nil, &size)
        if processID < 0 {
            SessionLog.write("forkpty failed: \(String(cString: strerror(errno)))")
            return nil
        }

        if processID == 0 {
            var empty = sigset_t()
            sigemptyset(&empty)
            sigprocmask(SIG_SETMASK, &empty, nil)
            for number in resetSignals {
                signal(number, SIG_DFL)
            }
            if let directory, chdir(directory) != 0 {
                if let home = getenv("HOME") {
                    _ = chdir(home)
                } else {
                    _ = chdir("/")
                }
            }
            execve(executable, argumentList.pointer, environmentList.pointer)
            _exit(127)
        }

        SessionIO.setNonBlocking(master)
        SessionIO.setCloseOnExec(master)

        var status = stat()
        let device: UInt64 = stat(&name, &status) == 0
            ? UInt64(UInt32(bitPattern: status.st_rdev))
            : 0

        return SessionProcess(masterDescriptor: master, processID: processID, ttyDevice: device)
    }

    static func resize(masterDescriptor: Int32, columns: UInt16, rows: UInt16) {
        var size = winsize(
            ws_row: rows == 0 ? 24 : rows,
            ws_col: columns == 0 ? 80 : columns,
            ws_xpixel: 0,
            ws_ypixel: 0
        )
        _ = ioctl(masterDescriptor, TIOCSWINSZ, &size)
    }

    static func windowSize(descriptor: Int32) -> (columns: UInt16, rows: UInt16)? {
        var size = winsize()
        guard ioctl(descriptor, TIOCGWINSZ, &size) == 0 else { return nil }
        return (size.ws_col, size.ws_row)
    }

    static func foregroundProcessGroup(masterDescriptor: Int32) -> pid_t? {
        let group = tcgetpgrp(masterDescriptor)
        return group > 0 ? group : nil
    }

    static func requestRedraw(masterDescriptor: Int32, fallbackProcessID: pid_t) {
        let group = foregroundProcessGroup(masterDescriptor: masterDescriptor) ?? fallbackProcessID
        guard group > 0 else { return }
        _ = killpg(group, SIGWINCH)
    }

    static func terminate(processID: pid_t) {
        guard processID > 0 else { return }
        if killpg(processID, SIGHUP) != 0 {
            _ = kill(processID, SIGHUP)
        }
        _ = killpg(processID, SIGCONT)
    }

    static func forceTerminate(processID: pid_t) -> Bool {
        forceTerminate(processIDs: [processID]).isEmpty
    }

    static func forceTerminate(processIDs: [pid_t]) -> Set<pid_t> {
        let targets = Set(processIDs.filter { $0 > 0 })
        guard !targets.isEmpty else { return [] }

        signalSessions(targets, signal: SIGTERM)
        let resistant = waitForSessionsExit(targets, attempts: gracefulTerminationAttempts)
        guard !resistant.isEmpty else { return [] }

        signalSessions(resistant, signal: SIGKILL)
        return waitForSessionsExit(resistant, attempts: forcedTerminationAttempts, repeatedSignal: SIGKILL)
    }

    private static func signalSessions(_ sessionIDs: Set<pid_t>, signal: Int32) {
        guard let members = sessionProcessIDs(sessionIDs: sessionIDs) else {
            for sessionID in sessionIDs {
                _ = killpg(sessionID, signal)
                _ = kill(sessionID, signal)
            }
            return
        }
        signalSessions(sessionIDs, members: members, signal: signal)
    }

    private static func signalSessions(_ sessionIDs: Set<pid_t>, members: [pid_t: [pid_t]], signal: Int32) {
        for sessionID in sessionIDs {
            let memberProcessIDs = members[sessionID] ?? []
            if memberProcessIDs.isEmpty {
                _ = kill(sessionID, signal)
                continue
            }
            for memberProcessID in memberProcessIDs {
                _ = kill(memberProcessID, signal)
            }
        }
    }

    private static func waitForSessionsExit(
        _ sessionIDs: Set<pid_t>,
        attempts: Int,
        repeatedSignal: Int32? = nil
    ) -> Set<pid_t> {
        var remaining = sessionIDs
        for _ in 0 ..< attempts {
            reap(processIDs: remaining)
            guard let members = sessionProcessIDs(sessionIDs: remaining) else {
                usleep(terminationPollMicroseconds)
                continue
            }
            remaining = activeSessionIDs(remaining, members: members)
            guard !remaining.isEmpty else { return [] }
            if let repeatedSignal {
                signalSessions(remaining, members: members, signal: repeatedSignal)
            }
            usleep(terminationPollMicroseconds)
        }

        reap(processIDs: remaining)
        guard let members = sessionProcessIDs(sessionIDs: remaining) else { return remaining }
        return activeSessionIDs(remaining, members: members)
    }

    private static func activeSessionIDs(_ sessionIDs: Set<pid_t>, members: [pid_t: [pid_t]]) -> Set<pid_t> {
        sessionIDs.filter { processExists($0) || !(members[$0] ?? []).isEmpty }
    }

    private static func reap(processIDs: Set<pid_t>) {
        for processID in processIDs {
            var status: Int32 = 0
            while waitpid(processID, &status, WNOHANG) < 0, errno == EINTR {}
        }
    }

    private static func processExists(_ processID: pid_t) -> Bool {
        guard kill(processID, 0) != 0 else { return true }
        return errno == EPERM
    }

    private static func sessionProcessIDs(sessionIDs: Set<pid_t>) -> [pid_t: [pid_t]]? {
        let stride = MemoryLayout<kinfo_proc>.stride
        for _ in 0 ..< 3 {
            var name: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_ALL]
            var size = 0
            guard sysctl(&name, 3, nil, &size, nil, 0) == 0, size > 0 else { return nil }

            var records = [kinfo_proc](repeating: kinfo_proc(), count: size / stride + 16)
            size = records.count * stride
            if sysctl(&name, 3, &records, &size, nil, 0) != 0 {
                guard errno == ENOMEM else { return nil }
                continue
            }

            var members: [pid_t: [pid_t]] = [:]
            for record in records.prefix(size / stride) {
                let processID = record.kp_proc.p_pid
                guard processID > 0 else { continue }
                let sessionID = getsid(processID)
                guard sessionIDs.contains(sessionID) else { continue }
                members[sessionID, default: []].append(processID)
            }
            return members
        }
        return nil
    }
}
