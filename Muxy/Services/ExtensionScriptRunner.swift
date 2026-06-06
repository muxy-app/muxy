import Foundation
import JavaScriptCore
import MuxyShared
import os

private let logger = Logger(subsystem: "app.muxy", category: "ExtensionScriptRunner")

@MainActor
final class ExtensionScriptRunner {
    static let shared = ExtensionScriptRunner()

    enum RunError: Error, LocalizedError {
        case scriptUnreadable(URL)
        case evaluationFailed(String)

        var errorDescription: String? {
            switch self {
            case let .scriptUnreadable(url): "Could not read script at \(url.path)"
            case let .evaluationFailed(message): "Script error: \(message)"
            }
        }
    }

    private final class ContextHandle {
        let context: JSContext
        let queue: DispatchQueue
        let cancelFlag: ScriptCancelFlag
        var bridge: AnyObject?
        var pendingModals = 0
        var scriptFinished = false

        init(context: JSContext, queue: DispatchQueue, cancelFlag: ScriptCancelFlag) {
            self.context = context
            self.queue = queue
            self.cancelFlag = cancelFlag
        }

        var canEvict: Bool { scriptFinished && pendingModals <= 0 }
    }

    private var contexts: [String: ContextHandle] = [:]

    private init() {}

    func evict(extensionID: String) {
        if let handle = contexts.removeValue(forKey: extensionID) {
            handle.cancelFlag.cancel()
        }
        ExtensionModalService.shared.dismiss(extensionID: extensionID)
    }

    func runScript(
        extensionID: String,
        scriptURL: URL,
        appState: AppState,
        projectStore: ProjectStore?,
        worktreeStore: WorktreeStore?
    ) async throws {
        guard let source = try? String(contentsOf: scriptURL, encoding: .utf8) else {
            throw RunError.scriptUnreadable(scriptURL)
        }

        let handle = try makeContextHandle(for: extensionID)
        let bridge = ScriptBridge(
            extensionID: extensionID,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            cancelFlag: handle.cancelFlag
        )
        handle.bridge = bridge
        bridge.deliveryQueue = handle.queue
        bridge.modalPendingChanged = { [weak self, weak handle] delta in
            guard let self, let handle else { return }
            handle.pendingModals += delta
            self.evictIfIdle(extensionID: extensionID, handle: handle)
        }
        bridge.install(into: handle.context)

        defer {
            handle.scriptFinished = true
            evictIfIdle(extensionID: extensionID, handle: handle)
        }

        let contextBox = JSContextBox(handle.context)
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            handle.queue.async {
                let context = contextBox.context
                let capture = ExceptionCapture()
                context.exceptionHandler = { _, exception in
                    capture.message = exception?.toString() ?? "unknown error"
                }
                _ = context.evaluateScript(source, withSourceURL: scriptURL)
                context.exceptionHandler = nil
                if let message = capture.message {
                    logger.error("Extension \(extensionID) script error: \(message)")
                    continuation.resume(throwing: RunError.evaluationFailed(message))
                } else {
                    continuation.resume()
                }
            }
        }
    }

    private func evictIfIdle(extensionID: String, handle: ContextHandle) {
        guard handle.canEvict, contexts[extensionID] === handle else { return }
        contexts.removeValue(forKey: extensionID)
    }

    private final class ExceptionCapture {
        var message: String?
    }

    private func makeContextHandle(for extensionID: String) throws -> ContextHandle {
        let queue = DispatchQueue(label: "app.muxy.extension.\(extensionID)")
        guard let context = JSContext() else {
            throw RunError.evaluationFailed("Failed to create JSContext")
        }
        let handle = ContextHandle(context: context, queue: queue, cancelFlag: ScriptCancelFlag())
        contexts[extensionID] = handle
        return handle
    }
}

final class ScriptCancelFlag: @unchecked Sendable {
    private let lock = NSLock()
    private var cancelled = false

    var isCancelled: Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelled
    }

    func cancel() {
        lock.lock()
        cancelled = true
        lock.unlock()
    }
}

private final class ScriptBridge: @unchecked Sendable {
    private let extensionID: String
    private weak var appState: AppState?
    private weak var projectStore: ProjectStore?
    private weak var worktreeStore: WorktreeStore?
    private let cancelFlag: ScriptCancelFlag

    @MainActor
    init(
        extensionID: String,
        appState: AppState,
        projectStore: ProjectStore?,
        worktreeStore: WorktreeStore?,
        cancelFlag: ScriptCancelFlag
    ) {
        self.extensionID = extensionID
        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
        self.cancelFlag = cancelFlag
    }

    private weak var context: JSContext?

    @MainActor
    func install(into context: JSContext) {
        self.context = context
        let dispatcher: @convention(block) (String, JSValue?) -> Any = { [weak self] verb, args in
            guard let self else { return Self.errorObject("bridge released") }
            let dict = (args?.toDictionary() as? [String: Any]) ?? [:]
            return self.dispatch(verb: verb, args: dict)
        }
        context.setObject(dispatcher, forKeyedSubscript: "__muxyDispatch" as NSString)

        let extID = extensionID
        let consoleBridge: @convention(block) (String, String) -> Void = { level, message in
            ExtensionLogStore.shared.append(extensionID: extID, line: "[\(level)] \(message)")
        }
        context.setObject(consoleBridge, forKeyedSubscript: "__muxyConsole" as NSString)
        context.evaluateScript(ExtensionBridgeJS.script(extensionID: extensionID, surface: .inProcess))
    }

    private func dispatch(verb: String, args: [String: Any]) -> Any {
        if cancelFlag.isCancelled {
            return Self.errorObject("extension stopped")
        }
        let bridge = self
        let argsBox = AnyBox(args)
        do {
            let encoded = try syncAwait { @MainActor in
                let raw = try await bridge.handle(verb: verb, args: argsBox.value)
                if verb == "modal.open", let dict = raw as? [String: Any], let requestID = dict["requestID"] as? String {
                    bridge.registerModalDelivery(requestID: requestID)
                }
                return try BridgeValue(from: raw)
            }
            return ["ok": true, "value": encoded.unwrap()]
        } catch let error as APIError {
            return Self.errorObject(error.message)
        } catch {
            return Self.errorObject(error.localizedDescription)
        }
    }

    @MainActor
    private func registerModalDelivery(requestID: String) {
        let onPending = modalPendingChanged
        onPending?(1)
        ExtensionModalService.shared.onResult(requestID: requestID) { [weak self] item in
            self?.deliverModalResult(requestID: requestID, item: item)
            onPending?(-1)
        }
    }

    @MainActor
    private func deliverModalResult(requestID: String, item: ExtensionModalService.Item?) {
        guard let queue = deliveryQueue, let context else { return }
        let payload: Any
        if let item {
            var dict: [String: Any] = ["id": item.id, "title": item.title]
            dict["subtitle"] = item.subtitle ?? NSNull()
            payload = dict
        } else {
            payload = NSNull()
        }
        let delivery = ModalDeliveryBox(context: context, requestID: requestID, payload: payload)
        queue.async {
            let deliver = delivery.context.objectForKeyedSubscript("__muxiDeliverModalResult")
            deliver?.call(withArguments: [delivery.requestID, delivery.payload])
        }
    }

    var deliveryQueue: DispatchQueue?
    var modalPendingChanged: ((Int) -> Void)?

    private static func errorObject(_ message: String) -> [String: Any] {
        ["ok": false, "error": message]
    }

    @MainActor
    private func handle(verb: String, args: [String: Any]) async throws -> Any {
        guard let appState else { throw APIError.underlying("app state unavailable") }
        return try await MuxyAPIDispatcher.dispatch(
            verb: verb,
            args: args,
            context: MuxyAPIDispatcher.Context(
                extensionID: extensionID,
                appState: appState,
                projectStore: projectStore,
                worktreeStore: worktreeStore
            )
        )
    }
}

private final class ResultBox<T>: @unchecked Sendable {
    var value: Result<T, Error>?
}

private struct AnyBox<T>: @unchecked Sendable {
    let value: T
    init(_ value: T) {
        self.value = value
    }
}

private struct JSContextBox: @unchecked Sendable {
    let context: JSContext
    init(_ context: JSContext) {
        self.context = context
    }
}

private struct ModalDeliveryBox: @unchecked Sendable {
    let context: JSContext
    let requestID: String
    let payload: Any
}

private struct BridgeValue: @unchecked Sendable {
    private let storage: Any

    init(from value: Any) throws {
        if value is NSNull || value is String || value is Int || value is Double || value is Bool {
            storage = value
            return
        }
        if let array = value as? [Any] {
            storage = array
            return
        }
        if let dict = value as? [String: Any] {
            storage = dict
            return
        }
        throw APIError.underlying("unsupported bridge value type")
    }

    func unwrap() -> Any {
        storage
    }
}

private func syncAwait<T: Sendable>(_ operation: @MainActor @Sendable @escaping () async throws -> T) throws -> T {
    let semaphore = DispatchSemaphore(value: 0)
    let box = ResultBox<T>()
    Task { @MainActor in
        do {
            box.value = try await .success(operation())
        } catch {
            box.value = .failure(error)
        }
        semaphore.signal()
    }
    semaphore.wait()
    guard let result = box.value else {
        throw APIError.underlying("script bridge produced no result")
    }
    switch result {
    case let .success(value): return value
    case let .failure(error): throw error
    }
}
