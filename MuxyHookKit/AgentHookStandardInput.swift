import Darwin
import Foundation

public enum AgentHookStandardInput {
    public static let maximumPayloadBytes = 1024 * 1024
    public static let maximumReadDuration = AgentHookExecutionBudget.defaultDuration

    public static func read(
        descriptor: Int32 = FileHandle.standardInput.fileDescriptor,
        limit: Int = maximumPayloadBytes,
        timeout: TimeInterval = maximumReadDuration
    ) -> Data {
        read(
            descriptor: descriptor,
            limit: limit,
            budget: AgentHookExecutionBudget(duration: timeout)
        )
    }

    public static func read(
        descriptor: Int32,
        limit: Int,
        budget: AgentHookExecutionBudget
    ) -> Data {
        var collected = Data()
        var buffer = [UInt8](repeating: 0, count: 64 * 1024)

        while true {
            guard collected.count < limit else { return collected }
            let remaining = limit - collected.count
            guard waitForInput(descriptor: descriptor, budget: budget) else { return collected }
            let count = Darwin.read(descriptor, &buffer, min(buffer.count, remaining))
            if count == 0 {
                return collected
            }
            if count < 0 {
                guard errno == EINTR else { return collected }
                continue
            }
            collected.append(contentsOf: buffer[0 ..< count])
            if collected.count >= limit {
                return collected
            }
        }
    }

    private static func waitForInput(descriptor: Int32, budget: AgentHookExecutionBudget) -> Bool {
        while true {
            guard let timeout = budget.pollTimeoutMilliseconds else { return false }
            var event = pollfd(fd: descriptor, events: Int16(POLLIN), revents: 0)
            let ready = poll(&event, 1, timeout)
            if ready > 0 {
                return true
            }
            if ready == 0 {
                return false
            }
            guard errno == EINTR else { return false }
        }
    }
}
