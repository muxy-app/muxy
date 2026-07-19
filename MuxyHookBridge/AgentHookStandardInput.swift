import Darwin
import Foundation

enum AgentHookStandardInput {
    static let maximumPayloadBytes = 1024 * 1024

    static func read(
        descriptor: Int32 = FileHandle.standardInput.fileDescriptor,
        limit: Int = maximumPayloadBytes
    ) -> Data {
        var collected = Data()
        var buffer = [UInt8](repeating: 0, count: 64 * 1024)
        var discarding = false

        while true {
            let count = Darwin.read(descriptor, &buffer, buffer.count)
            if count == 0 {
                return collected
            }
            if count < 0 {
                guard errno == EINTR else { return collected }
                continue
            }
            guard !discarding else { continue }

            let remaining = limit - collected.count
            guard remaining > 0 else {
                discarding = true
                continue
            }
            collected.append(contentsOf: buffer[0 ..< min(count, remaining)])
            if collected.count >= limit {
                discarding = true
            }
        }
    }
}
