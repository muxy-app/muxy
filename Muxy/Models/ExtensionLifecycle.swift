import Foundation

enum LifecycleSurfaceKind: String {
    case tab
    case panel
    case popover
}

struct LifecycleSurfaceKey: Hashable {
    let kind: LifecycleSurfaceKind
    let instanceID: String
}

enum LifecycleVerdict {
    case allow
    case prevent
}

enum ExtensionLifecycle {
    static let beforeCloseTimeout: Duration = .seconds(5)
}

@MainActor
protocol BeforeCloseAsking: AnyObject {
    func requestBeforeClose(reason: LifecycleSurfaceKind, instanceID: String) async -> LifecycleVerdict
    func failPendingLifecycle()
}
