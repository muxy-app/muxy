import Dispatch
import Foundation

public struct AgentHookExecutionBudget: Sendable {
    public static let defaultDuration: TimeInterval = 0.4

    private let uptimeNanoseconds: UInt64

    public init(duration: TimeInterval = defaultDuration) {
        let now = DispatchTime.now().uptimeNanoseconds
        let finiteDuration = duration.isFinite ? duration : 0
        let boundedDuration = max(0, min(finiteDuration, TimeInterval(Int32.max)))
        let interval = UInt64(boundedDuration * 1_000_000_000)
        let addition = now.addingReportingOverflow(interval)
        uptimeNanoseconds = addition.overflow ? UInt64.max : addition.partialValue
    }

    public var remainingDuration: TimeInterval {
        let now = DispatchTime.now().uptimeNanoseconds
        guard uptimeNanoseconds > now else { return 0 }
        return TimeInterval(uptimeNanoseconds - now) / 1_000_000_000
    }

    var pollTimeoutMilliseconds: Int32? {
        let remaining = remainingDuration
        guard remaining > 0 else { return nil }
        let milliseconds = UInt64(remaining * 1000) + 1
        return Int32(min(max(milliseconds, 1), UInt64(Int32.max)))
    }
}
