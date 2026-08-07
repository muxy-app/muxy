import Darwin
import Foundation

struct ProcessInvocation {
    let executablePath: String
    let arguments: [String]
    let workingDirectory: String?

    init(
        executablePath: String,
        arguments: [String],
        workingDirectory: String? = nil
    ) {
        self.executablePath = executablePath
        self.arguments = arguments
        self.workingDirectory = workingDirectory
    }
}

enum ProcessArgumentsInspector {
    static func invocation(pid: UInt64) -> ProcessInvocation? {
        guard pid > 0, pid <= UInt64(Int32.max) else { return nil }

        var mib = [CTL_KERN, KERN_PROCARGS2, Int32(pid)]
        var size = 0
        guard sysctl(&mib, 3, nil, &size, nil, 0) == 0, size > MemoryLayout<Int32>.size else { return nil }

        var buffer = [UInt8](repeating: 0, count: size)
        guard sysctl(&mib, 3, &buffer, &size, nil, 0) == 0 else { return nil }

        let argumentCount = buffer.withUnsafeBytes { $0.load(as: Int32.self) }
        var offset = MemoryLayout<Int32>.size

        func readString() -> String {
            let start = offset
            while offset < buffer.count, buffer[offset] != 0 {
                offset += 1
            }
            let value = String(bytes: buffer[start ..< offset], encoding: .utf8) ?? ""
            offset += 1
            return value
        }

        let executablePath = readString()
        while offset < buffer.count, buffer[offset] == 0 {
            offset += 1
        }

        var arguments: [String] = []
        while arguments.count < Int(argumentCount), offset < buffer.count {
            arguments.append(readString())
        }
        return ProcessInvocation(
            executablePath: executablePath,
            arguments: arguments,
            workingDirectory: workingDirectory(pid: Int32(pid))
        )
    }

    private static func workingDirectory(pid: Int32) -> String? {
        var info = proc_vnodepathinfo()
        let size = Int32(MemoryLayout<proc_vnodepathinfo>.size)
        let result = proc_pidinfo(pid, PROC_PIDVNODEPATHINFO, 0, &info, size)
        guard result == size else { return nil }
        let path = withUnsafeBytes(of: info.pvi_cdir.vip_path) { rawBuffer in
            let bytes = rawBuffer.prefix { $0 != 0 }
            return String(bytes: bytes, encoding: .utf8)
        }
        guard let path, !path.isEmpty else { return nil }
        return path
    }
}
