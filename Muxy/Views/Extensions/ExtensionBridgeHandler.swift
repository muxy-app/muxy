import Foundation
import MuxyShared
import os
import WebKit

private let logger = Logger(subsystem: "app.muxy", category: "ExtensionBridge")

@MainActor
final class ExtensionBridgeHandler: NSObject, WKScriptMessageHandlerWithReply {
    private let extensionID: String
    private weak var appState: AppState?
    private weak var projectStore: ProjectStore?
    private weak var worktreeStore: WorktreeStore?
    private weak var webView: WKWebView?
    private var eventObservers: [String: UUID] = [:]
    private var extensionEventObservers: [String: UUID] = [:]
    private let modalProviderBridge = ModalProviderBridge()

    init(
        extensionID: String,
        appState: AppState,
        projectStore: ProjectStore?,
        worktreeStore: WorktreeStore?
    ) {
        self.extensionID = extensionID
        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
    }

    func attach(to webView: WKWebView) {
        self.webView = webView
    }

    func dropAllEventSubscriptions() {
        for token in eventObservers.values {
            NotificationSocketServer.shared.removeInProcessObserver(token)
        }
        eventObservers.removeAll()
        for token in extensionEventObservers.values {
            NotificationSocketServer.shared.removeExtensionEventObserver(token)
        }
        extensionEventObservers.removeAll()
        modalProviderBridge.cancelAll()
    }

    func userContentController(
        _: WKUserContentController,
        didReceive message: WKScriptMessage,
        replyHandler: @escaping @MainActor (Any?, String?) -> Void
    ) {
        let body = message.body
        Task { @MainActor in
            let reply = await self.dispatch(body)
            replyHandler(reply, nil)
        }
    }

    private func dispatch(_ body: Any) async -> [String: Any] {
        guard let payload = body as? [String: Any],
              let verb = payload["verb"] as? String,
              let requestID = payload["requestID"] as? String
        else {
            return ["ok": false, "error": "invalid message"]
        }
        let args = (payload["args"] as? [String: Any]) ?? [:]

        guard let appState else {
            return ["requestID": requestID, "ok": false, "error": "app state unavailable"]
        }

        do {
            let value = try await handle(verb: verb, args: args, appState: appState)
            return ["requestID": requestID, "ok": true, "value": value]
        } catch let error as APIError {
            return ["requestID": requestID, "ok": false, "error": error.message]
        } catch {
            return ["requestID": requestID, "ok": false, "error": error.localizedDescription]
        }
    }

    private func handle(verb: String, args: [String: Any], appState: AppState) async throws -> Any {
        switch verb {
        case "events.subscribe":
            return try handleSubscribe(args: args)
        case "events.unsubscribe":
            return try handleUnsubscribe(args: args)
        case "events.emit":
            return try await handleEmit(args: args)
        case "modal.provide":
            modalProviderBridge.resolve(args: args)
            return NSNull()
        default:
            return try await MuxyAPIDispatcher.dispatch(
                verb: verb,
                args: args,
                context: MuxyAPIDispatcher.Context(
                    extensionID: extensionID,
                    appState: appState,
                    projectStore: projectStore,
                    worktreeStore: worktreeStore,
                    modalProvider: { [weak self] providerID in
                        self?.makeModalProvider(providerID: providerID) ?? Self.emptyProvider
                    }
                )
            )
        }
    }

    private static let emptyProvider: ExtensionModalService.Provider = { _, _, _ in
        ExtensionModalService.Page(items: [], hasMore: false)
    }

    private func makeModalProvider(providerID: String) -> ExtensionModalService.Provider {
        let bridge = modalProviderBridge
        return { [weak self] query, offset, limit in
            guard let self else { return ExtensionModalService.Page(items: [], hasMore: false) }
            let token = bridge.nextToken()
            self.deliverModalProviderRequest(providerID: providerID, token: token, query: query, offset: offset, limit: limit)
            return await bridge.awaitPage(token: token)
        }
    }

    private func deliverModalProviderRequest(
        providerID: String,
        token: Int,
        query: String,
        offset: Int,
        limit: Int
    ) {
        guard let webView else { return }
        let payload: [String: Any] = [
            "providerID": providerID,
            "requestToken": token,
            "query": query,
            "offset": offset,
            "limit": limit,
        ]
        let payloadLiteral = jsLiteral(payloadJSON: payload)
        let script = """
        if (typeof window.__muxyEventDispatch === 'function') {
            window.__muxyEventDispatch("__muxiModalProvider", \(payloadLiteral));
        }
        """
        webView.evaluateJavaScript(script, completionHandler: nil)
    }

    private func handleSubscribe(args: [String: Any]) throws -> Any {
        let event = try stringArg(args, "event")
        if ExtensionLocalEvent.isLocalName(event) {
            return try handleLocalSubscribe(event: event)
        }
        guard let muxyExtension = ExtensionStore.shared.loadedExtension(id: extensionID) else {
            throw APIError.invalidArguments("extension \(extensionID) not loaded")
        }
        let allowedEvents = Set(muxyExtension.manifest.events)
        let commandEvents = Set(muxyExtension.manifest.commands.map(\.eventName))
        guard allowedEvents.contains(event) || commandEvents.contains(event) else {
            throw APIError.invalidArguments("event \(event) not declared in manifest")
        }
        guard eventObservers[event] == nil else { return event }
        let token = NotificationSocketServer.shared.addInProcessObserver { [weak self] incoming in
            guard incoming.name == event else { return }
            Task { @MainActor [weak self] in
                self?.deliverEvent(incoming)
            }
        }
        eventObservers[event] = token
        return event
    }

    private func handleUnsubscribe(args: [String: Any]) throws -> Any {
        let event = try stringArg(args, "event")
        if ExtensionLocalEvent.isLocalName(event) {
            guard let token = extensionEventObservers.removeValue(forKey: event) else {
                return NSNull()
            }
            NotificationSocketServer.shared.removeExtensionEventObserver(token)
            return NSNull()
        }
        guard let token = eventObservers.removeValue(forKey: event) else {
            return NSNull()
        }
        NotificationSocketServer.shared.removeInProcessObserver(token)
        return NSNull()
    }

    private func handleLocalSubscribe(event: String) throws -> Any {
        guard ExtensionLocalEvent.isValidName(event) else {
            throw APIError.invalidArguments("extension events must start with extension.")
        }
        guard ExtensionStore.shared.loadedExtension(id: extensionID) != nil else {
            throw APIError.invalidArguments("extension \(extensionID) not loaded")
        }
        guard extensionEventObservers[event] == nil else { return event }
        let token = NotificationSocketServer.shared.addExtensionEventObserver(extensionID: extensionID) { [weak self] incoming in
            guard incoming.name == event else { return }
            Task { @MainActor [weak self] in
                self?.deliverExtensionEvent(incoming)
            }
        }
        extensionEventObservers[event] = token
        return event
    }

    private func handleEmit(args: [String: Any]) async throws -> Any {
        guard ExtensionStore.shared.loadedExtension(id: extensionID) != nil else {
            throw APIError.invalidArguments("extension \(extensionID) not loaded")
        }
        let event = try ExtensionBridgeShared.decodeExtensionLocalEvent(args: args)
        let delivered = await NotificationSocketServer.shared.emitExtensionEventToBackground(
            extensionID: extensionID,
            event: event
        )
        guard delivered else {
            throw APIError.invalidArguments("background script unavailable")
        }
        return NSNull()
    }

    private func deliverEvent(_ event: ExtensionEvent) {
        guard let webView else { return }
        let nameLiteral = jsLiteral(event.name)
        let payloadLiteral = jsLiteral(payloadJSON: event.payload)
        let script = """
        if (typeof window.__muxyEventDispatch === 'function') {
            window.__muxyEventDispatch(\(nameLiteral), \(payloadLiteral));
        }
        """
        webView.evaluateJavaScript(script, completionHandler: nil)
    }

    private func deliverExtensionEvent(_ event: ExtensionLocalEvent.Message) {
        guard let webView,
              let payloadLiteral = String(data: event.payload, encoding: .utf8)
        else { return }
        let nameLiteral = jsLiteral(event.name)
        let script = """
        if (typeof window.__muxyEventDispatch === 'function') {
            window.__muxyEventDispatch(\(nameLiteral), \(payloadLiteral));
        }
        """
        webView.evaluateJavaScript(script, completionHandler: nil)
    }

    private func jsLiteral(_ value: String) -> String {
        guard let data = try? JSONEncoder().encode(value),
              let literal = String(data: data, encoding: .utf8)
        else { return "\"\"" }
        return literal
    }

    private func jsLiteral(payloadJSON: [String: String]) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: payloadJSON),
              let literal = String(data: data, encoding: .utf8)
        else { return "{}" }
        return literal
    }

    private func jsLiteral(payloadJSON: [String: Any]) -> String {
        guard JSONSerialization.isValidJSONObject(payloadJSON),
              let data = try? JSONSerialization.data(withJSONObject: payloadJSON),
              let literal = String(data: data, encoding: .utf8)
        else { return "{}" }
        return literal
    }

    private func stringArg(_ args: [String: Any], _ key: String) throws -> String {
        if let value = args[key] as? String { return value }
        throw APIError.invalidArguments("missing argument '\(key)'")
    }
}
