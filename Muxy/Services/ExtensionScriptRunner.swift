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

    private struct ContextHandle {
        let context: JSContext
        let queue: DispatchQueue
        let cancelFlag: ScriptCancelFlag
    }

    private var contexts: [String: ContextHandle] = [:]

    private init() {}

    func evict(extensionID: String) {
        if let handle = contexts.removeValue(forKey: extensionID) {
            handle.cancelFlag.cancel()
        }
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
        defer {
            if contexts[extensionID]?.cancelFlag === handle.cancelFlag {
                contexts.removeValue(forKey: extensionID)
            }
        }
        let bridge = ScriptBridge(
            extensionID: extensionID,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            cancelFlag: handle.cancelFlag
        )
        bridge.install(into: handle.context)

        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            handle.queue.async {
                let capture = ExceptionCapture()
                handle.context.exceptionHandler = { _, exception in
                    capture.message = exception?.toString() ?? "unknown error"
                }
                _ = handle.context.evaluateScript(source, withSourceURL: scriptURL)
                handle.context.exceptionHandler = nil
                if let message = capture.message {
                    logger.error("Extension \(extensionID) script error: \(message)")
                    continuation.resume(throwing: RunError.evaluationFailed(message))
                } else {
                    continuation.resume()
                }
            }
        }
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

    @MainActor
    func install(into context: JSContext) {
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
                return try BridgeValue(from: raw)
            }
            return ["ok": true, "value": encoded.unwrap()]
        } catch let error as APIError {
            return Self.errorObject(error.message)
        } catch {
            return Self.errorObject(error.localizedDescription)
        }
    }

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
