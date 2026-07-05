import Foundation
import JavaScriptCore
import MuxyShared
import os

private let logger = Logger(subsystem: "app.muxy", category: "ExtensionScriptRunner")

final class JSExecutor: @unchecked Sendable {
    private let thread: Thread
    private let ready = DispatchSemaphore(value: 0)
    private let lock = NSLock()
    private var runLoop: CFRunLoop?
    private var stopped = false

    init(label: String) {
        let box = ThreadStartBox()
        thread = Thread {
            guard let runLoop = CFRunLoopGetCurrent() else {
                box.abandon()
                return
            }
            box.publish(runLoop)
            var sourceContext = CFRunLoopSourceContext()
            let source = CFRunLoopSourceCreate(kCFAllocatorDefault, 0, &sourceContext)
            CFRunLoopAddSource(runLoop, source, .commonModes)
            while !box.shouldStop() {
                CFRunLoopRunInMode(.defaultMode, 1_000_000_000, false)
            }
        }
        thread.name = label
        thread.stackSize = 4 << 20
        box.configure(executor: self)
        thread.start()
        ready.wait()
    }

    fileprivate func markReady(_ loop: CFRunLoop?) {
        lock.lock()
        runLoop = loop
        lock.unlock()
        ready.signal()
    }

    fileprivate func isStopped() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return stopped
    }

    func stop() {
        lock.lock()
        stopped = true
        let loop = runLoop
        lock.unlock()
        if let loop {
            CFRunLoopStop(loop)
            CFRunLoopWakeUp(loop)
        }
    }

    @discardableResult
    func async(_ work: @escaping @Sendable () -> Void) -> Bool {
        lock.lock()
        let loop = runLoop
        let alreadyStopped = stopped
        lock.unlock()
        guard let loop, !alreadyStopped else { return false }
        CFRunLoopPerformBlock(loop, CFRunLoopMode.defaultMode.rawValue, work)
        CFRunLoopWakeUp(loop)
        return true
    }
}

private final class ThreadStartBox: @unchecked Sendable {
    private weak var executor: JSExecutor?

    func configure(executor: JSExecutor) {
        self.executor = executor
    }

    func publish(_ loop: CFRunLoop) {
        executor?.markReady(loop)
    }

    func abandon() {
        executor?.markReady(nil)
    }

    func shouldStop() -> Bool {
        executor?.isStopped() ?? true
    }
}

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
        let executor: JSExecutor
        let cancelFlag: ScriptCancelFlag
        var bridge: AnyObject?
        var pendingModals = 0
        var scriptFinished = false

        init(context: JSContext, executor: JSExecutor, cancelFlag: ScriptCancelFlag) {
            self.context = context
            self.executor = executor
            self.cancelFlag = cancelFlag
        }

        var canEvict: Bool { scriptFinished && pendingModals <= 0 }
    }

    private var contexts: [String: ContextHandle] = [:]

    private init() {}

    func evict(extensionID: String) {
        if let handle = contexts.removeValue(forKey: extensionID) {
            handle.cancelFlag.cancel()
            handle.executor.stop()
        }
        ExtensionModalService.shared.dismiss(extensionID: extensionID)
        ExtensionDialogService.cancel(extensionID: extensionID)
        ExtensionWebviewModalService.shared.dismiss(extensionID: extensionID)
    }

    func evictAll() {
        for extensionID in Array(contexts.keys) {
            evict(extensionID: extensionID)
        }
        ExtensionDialogService.cancelAll()
    }

    func runScript(
        extensionID: String,
        scriptURL: URL,
        appState: AppState,
        stores: ExtensionAPIStores
    ) async throws {
        guard let source = try? String(contentsOf: scriptURL, encoding: .utf8) else {
            throw RunError.scriptUnreadable(scriptURL)
        }

        let handle = try makeContextHandle(for: extensionID)
        let bridge = ScriptBridge(
            extensionID: extensionID,
            appState: appState,
            stores: stores,
            cancelFlag: handle.cancelFlag
        )
        handle.bridge = bridge
        bridge.executor = handle.executor
        bridge.modalPendingChanged = { [weak self, weak handle] delta in
            guard let self, let handle else { return }
            handle.pendingModals += delta
            self.evictIfIdle(extensionID: extensionID, handle: handle)
        }

        defer {
            handle.scriptFinished = true
            evictIfIdle(extensionID: extensionID, handle: handle)
        }

        let contextBox = JSContextBox(handle.context)
        let bridgeBox = ScriptBridgeBox(bridge)
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            handle.executor.async {
                let context = contextBox.context
                bridgeBox.bridge.install(into: context)
                let capture = ExceptionCapture()
                context.exceptionHandler = { _, exception in
                    capture.message = exception?.toString() ?? "unknown error"
                }
                _ = context.evaluateScript(source, withSourceURL: scriptURL)
                context.exceptionHandler = { _, exception in
                    let message = exception?.toString() ?? "unknown error"
                    ExtensionLogStore.shared.append(extensionID: extensionID, line: "[err] \(message)")
                }
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
        evict(extensionID: extensionID)
        let executor = JSExecutor(label: "app.muxy.extension.\(extensionID)")
        let contextBox = MakeContextBox()
        let ready = DispatchSemaphore(value: 0)
        executor.async {
            contextBox.context = JSContext()
            ready.signal()
        }
        ready.wait()
        guard let context = contextBox.context else {
            executor.stop()
            throw RunError.evaluationFailed("Failed to create JSContext")
        }
        let handle = ContextHandle(context: context, executor: executor, cancelFlag: ScriptCancelFlag())
        contexts[extensionID] = handle
        return handle
    }
}

private final class MakeContextBox: @unchecked Sendable {
    var context: JSContext?
}

private struct ScriptBridgeBox: @unchecked Sendable {
    let bridge: ScriptBridge
    init(_ bridge: ScriptBridge) {
        self.bridge = bridge
    }
}

final class ScriptCancelFlag: @unchecked Sendable {
    private let lock = NSLock()
    private var cancelled = false
    private var waiters: [DispatchSemaphore] = []

    var isCancelled: Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelled
    }

    func cancel() {
        lock.lock()
        cancelled = true
        let pending = waiters
        waiters.removeAll()
        lock.unlock()
        for waiter in pending {
            waiter.signal()
        }
    }

    func register(_ waiter: DispatchSemaphore) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard !cancelled else { return false }
        waiters.append(waiter)
        return true
    }

    func unregister(_ waiter: DispatchSemaphore) {
        lock.lock()
        defer { lock.unlock() }
        waiters.removeAll { $0 === waiter }
    }
}

private final class ScriptBridge: @unchecked Sendable {
    private let extensionID: String
    private weak var appState: AppState?
    private let stores: ExtensionAPIStores
    private let cancelFlag: ScriptCancelFlag

    @MainActor
    init(
        extensionID: String,
        appState: AppState,
        stores: ExtensionAPIStores,
        cancelFlag: ScriptCancelFlag
    ) {
        self.extensionID = extensionID
        self.appState = appState
        self.stores = stores
        self.cancelFlag = cancelFlag
    }

    private weak var context: JSContext?

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
            let encoded = try syncAwait(cancelFlag: cancelFlag) { @MainActor in
                let raw = try await bridge.handle(verb: verb, args: argsBox.value)
                if verb == "modal.open", let dict = raw as? [String: Any], let requestID = dict["requestID"] as? String {
                    bridge.registerModalDelivery(requestID: requestID)
                    bridge.registerModalQueryDelivery(requestID: requestID)
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
        let completion = ModalDeliveryCompletion(onPending)
        ExtensionModalService.shared.onResult(requestID: requestID) { [weak self] item in
            guard let self else {
                completion.finish()
                return
            }
            self.deliverModalResult(requestID: requestID, item: item, completion: completion)
        }
    }

    @MainActor
    private func deliverModalResult(
        requestID: String,
        item: ExtensionModalService.Item?,
        completion: ModalDeliveryCompletion
    ) {
        guard let executor, let context else {
            completion.finish()
            return
        }
        let payload: Any
        if let item {
            var dict: [String: Any] = ["id": item.id, "title": item.title]
            dict["subtitle"] = item.subtitle ?? NSNull()
            payload = dict
        } else {
            payload = NSNull()
        }
        let delivery = ModalDeliveryBox(context: context, requestID: requestID, payload: payload)
        let enqueued = executor.async {
            let deliver = delivery.context.objectForKeyedSubscript("__muxiDeliverModalResult")
            deliver?.call(withArguments: [delivery.requestID, delivery.payload])
            completion.finish()
        }
        if !enqueued {
            completion.finish()
        }
    }

    @MainActor
    private func registerModalQueryDelivery(requestID: String) {
        ExtensionModalService.shared.onQueryRequest(requestID: requestID) { [weak self] queryID, query, options in
            self?.deliverModalQuery(requestID: requestID, queryID: queryID, query: query, options: options)
        }
    }

    @MainActor
    private func deliverModalQuery(
        requestID: String,
        queryID: Int,
        query: String,
        options: ExtensionModalSearchOptions
    ) {
        guard let executor, let context else { return }
        modalQueryStamp.advance(requestID: requestID, queryID: queryID)
        let stamp = modalQueryStamp
        let delivery = ModalQueryDeliveryBox(
            context: context,
            requestID: requestID,
            queryID: queryID,
            query: query,
            options: options.payload
        )
        executor.async {
            guard stamp.isCurrent(requestID: delivery.requestID, queryID: delivery.queryID) else { return }
            let deliver = delivery.context.objectForKeyedSubscript("__muxyDeliverModalQuery")
            deliver?.call(withArguments: [delivery.requestID, delivery.queryID, delivery.query, delivery.options])
        }
    }

    var executor: JSExecutor?
    var modalPendingChanged: ((Int) -> Void)?
    private let modalQueryStamp = ModalQueryStamp()

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
                stores: stores
            )
        )
    }
}

private final class ModalQueryStamp: @unchecked Sendable {
    private let lock = NSLock()
    private var requestID: String?
    private var queryID = 0

    func advance(requestID: String, queryID: Int) {
        lock.lock()
        defer { lock.unlock() }
        self.requestID = requestID
        self.queryID = queryID
    }

    func isCurrent(requestID: String, queryID: Int) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return requestID == self.requestID && queryID == self.queryID
    }
}

private final class ResultBox<T>: @unchecked Sendable {
    private let lock = NSLock()
    private var stored: Result<T, Error>?

    var value: Result<T, Error>? {
        get {
            lock.lock()
            defer { lock.unlock() }
            return stored
        }
        set {
            lock.lock()
            stored = newValue
            lock.unlock()
        }
    }
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

private struct ModalQueryDeliveryBox: @unchecked Sendable {
    let context: JSContext
    let requestID: String
    let queryID: Int
    let query: String
    let options: [String: Bool]
}

private final class ModalDeliveryCompletion: @unchecked Sendable {
    private let onPending: ((Int) -> Void)?

    init(_ onPending: ((Int) -> Void)?) {
        self.onPending = onPending
    }

    func finish() {
        DispatchQueue.main.async { [self] in
            onPending?(-1)
        }
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

private func syncAwait<T: Sendable>(
    cancelFlag: ScriptCancelFlag,
    _ operation: @MainActor @Sendable @escaping () async throws -> T
) throws -> T {
    let semaphore = DispatchSemaphore(value: 0)
    let box = ResultBox<T>()
    let task = Task { @MainActor in
        do {
            box.value = try await .success(operation())
        } catch {
            box.value = .failure(error)
        }
        semaphore.signal()
    }
    guard cancelFlag.register(semaphore) else {
        task.cancel()
        throw APIError.underlying("extension stopped")
    }
    semaphore.wait()
    cancelFlag.unregister(semaphore)
    guard let result = box.value else {
        task.cancel()
        throw APIError.underlying("extension stopped")
    }
    switch result {
    case let .success(value): return value
    case let .failure(error): throw error
    }
}
