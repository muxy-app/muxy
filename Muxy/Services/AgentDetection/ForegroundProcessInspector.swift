import Darwin
import Foundation

enum ForegroundProcessInspector {
    static func executableNameCandidates(pid: UInt64) -> [String] {
        guard pid > 0, pid <= UInt64(Int32.max) else { return [] }
        guard let arguments = processArguments(pid: Int32(pid)) else { return [] }

        var candidates: [String] = []
        if let execName = lastPathComponent(of: arguments.executablePath) {
            candidates.append(execName)
        }
        if let firstArgument = arguments.argv.first, let argName = lastPathComponent(of: firstArgument) {
            candidates.append(argName)
        }
        return candidates
    }

    private static func lastPathComponent(of path: String) -> String? {
        let trimmed = path.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let component = (trimmed as NSString).lastPathComponent
        return component.isEmpty ? nil : component
    }

    private static func processArguments(pid: Int32) -> (executablePath: String, argv: [String])? {
        var mib = [CTL_KERN, KERN_PROCARGS2, pid]
        var size = 0
        guard sysctl(&mib, 3, nil, &size, nil, 0) == 0, size > MemoryLayout<Int32>.size else { return nil }

        var buffer = [UInt8](repeating: 0, count: size)
        guard sysctl(&mib, 3, &buffer, &size, nil, 0) == 0 else { return nil }

        let argc = buffer.withUnsafeBytes { $0.load(as: Int32.self) }
        var offset = MemoryLayout<Int32>.size

        func readCString() -> String {
            let start = offset
            while offset < buffer.count, buffer[offset] != 0 {
                offset += 1
            }
            let value = String(bytes: buffer[start ..< offset], encoding: .utf8) ?? ""
            offset += 1
            return value
        }

        let executablePath = readCString()
        while offset < buffer.count, buffer[offset] == 0 {
            offset += 1
        }

        var argv: [String] = []
        var index = 0
        while index < Int(argc), offset < buffer.count {
            argv.append(readCString())
            index += 1
        }
        return (executablePath, argv)
    }
}
