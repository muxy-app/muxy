import Foundation
import JavaScriptCore
import MuxyShared

final class HostBridge: @unchecked Sendable {
    private let client: HostSocketClient
    private let extensionID: String
    private let context: JSContext

    init(client: HostSocketClient, extensionID: String, context: JSContext) {
        self.client = client
        self.extensionID = extensionID
        self.context = context
    }

    func install() {
        let dispatch: @convention(block) (String, JSValue?) -> Any = { [weak self] verb, args in
            guard let self else { return ["ok": false, "error": "host released"] }
            return self.dispatch(verb: verb, args: args)
        }
        context.setObject(dispatch, forKeyedSubscript: "__muxyDispatch" as NSString)

        let console: @convention(block) (String, String) -> Void = { level, message in
            FileHandle.standardError.write(Data("[\(level)] \(message)\n".utf8))
        }
        context.setObject(console, forKeyedSubscript: "__muxyConsole" as NSString)

        let subscribe: @convention(block) (String) -> Void = { [weak self] name in
            self?.subscribe(name: name)
        }
        context.setObject(subscribe, forKeyedSubscript: "__muxySubscribe" as NSString)

        let invokeResolve: @convention(block) (String, String) -> Void = { [weak self] callID, json in
            self?.sendInvokeResult(callID: callID, ok: true, payload: Data(json.utf8))
        }
        context.setObject(invokeResolve, forKeyedSubscript: "__muxyInvokeResolve" as NSString)

        let invokeReject: @convention(block) (String, String) -> Void = { [weak self] callID, message in
            self?.sendInvokeResult(callID: callID, ok: false, payload: Data(message.utf8))
        }
        context.setObject(invokeReject, forKeyedSubscript: "__muxyInvokeReject" as NSString)

        context.evaluateScript(ExtensionBridgeJS.script(extensionID: extensionID, surface: .background))
    }

    private func dispatch(verb: String, args: JSValue?) -> Any {
        let dict = (args?.toDictionary() as? [String: Any]) ?? [:]
        switch verb {
        case "exec":
            return dispatchExec(dict)
        case "notifications.notify":
            return dispatchNotify(dict)
        case "dialog.confirm",
             "dialog.alert":
            return dispatchDialog(verb: verb, dict: dict)
        default:
            return ["ok": false, "error": "verb '\(verb)' is not available in background context"]
        }
    }

    private func dispatchExec(_ dict: [String: Any]) -> Any {
        guard let payload = try? JSONSerialization.data(withJSONObject: dict) else {
            return ["ok": false, "error": "could not encode exec payload"]
        }
        let line = "exec|\(payload.base64EncodedString())"
        do {
            let reply = try client.sendAndWaitReply(line)
            if reply.hasPrefix("error:") {
                return ["ok": false, "error": String(reply.dropFirst("error:".count))]
            }
            guard let data = Data(base64Encoded: reply),
                  let value = try? JSONSerialization.jsonObject(with: data)
            else {
                return ["ok": false, "error": "invalid exec reply"]
            }
            return ["ok": true, "value": value]
        } catch {
            return ["ok": false, "error": "\(error)"]
        }
    }

    private func dispatchDialog(verb: String, dict: [String: Any]) -> Any {
        guard let payload = try? JSONSerialization.data(withJSONObject: dict) else {
            return ["ok": false, "error": "could not encode dialog payload"]
        }
        let line = "\(verb)|\(payload.base64EncodedString())"
        do {
            let reply = try client.sendAndWaitReply(line)
            if reply.hasPrefix("error:") {
                return ["ok": false, "error": String(reply.dropFirst("error:".count))]
            }
            guard let data = Data(base64Encoded: reply),
                  let value = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
            else {
                return ["ok": false, "error": "invalid dialog reply"]
            }
            return ["ok": true, "value": value]
        } catch {
            return ["ok": false, "error": "\(error)"]
        }
    }

    private func dispatchNotify(_ dict: [String: Any]) -> Any {
        let title = sanitize(dict["title"] as? String ?? "")
        let body = sanitize(dict["body"] as? String ?? "")
        guard !title.isEmpty || !body.isEmpty else {
            return ["ok": false, "error": "notification requires title or body"]
        }
        let paneID = sanitize(dict["paneID"] as? String ?? "")
        let line = "\(sanitize(extensionID))|\(paneID)|\(title)|\(body)"
        do {
            try client.send(line)
            return ["ok": true, "value": NSNull()]
        } catch {
            return ["ok": false, "error": "\(error)"]
        }
    }

    private func sanitize(_ value: String) -> String {
        value
            .replacingOccurrences(of: "|", with: " ")
            .replacingOccurrences(of: "\n", with: " ")
    }

    private func subscribe(name: String) {
        do {
            let reply = try client.sendAndWaitReply("subscribe|\(name)")
            guard reply == "ok" else {
                FileHandle.standardError.write(Data("[muxy-extension-host] subscribe \(name) failed: \(reply)\n".utf8))
                return
            }
        } catch {
            FileHandle.standardError.write(Data("[muxy-extension-host] subscribe \(name) error: \(error)\n".utf8))
        }
    }

    func handleInvokeLine(_ line: String) {
        guard let parsed = Self.parseInvoke(line) else { return }
        let payloadValue: Any = if let data = Data(base64Encoded: parsed.payload),
                                   let object = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
        {
            object
        } else {
            NSNull()
        }
        let box = ContextBox(context)
        DispatchQueue.main.async {
            let argument = JSValue(object: payloadValue, in: box.context) ?? JSValue(nullIn: box.context)
            let dispatcher = box.context.objectForKeyedSubscript("__muxyDispatchInvoke")
            dispatcher?.call(withArguments: [parsed.callID, parsed.action, argument as Any])
        }
    }

    private func sendInvokeResult(callID: String, ok: Bool, payload: Data) {
        let status = ok ? "ok" : "err"
        let line = "invoke-result|\(callID)|\(status)|\(payload.base64EncodedString())"
        try? client.send(line)
    }

    static func parseInvoke(_ line: String) -> (callID: String, action: String, payload: String)? {
        let parts = line.split(separator: "|", maxSplits: 3, omittingEmptySubsequences: false).map(String.init)
        guard parts.count >= 4, parts[0] == "invoke" else { return nil }
        let callID = parts[1]
        let action = parts[2]
        guard !callID.isEmpty, !action.isEmpty else { return nil }
        let payload = parts[3]
        return (callID, action, payload)
    }

    func handleEventLine(_ line: String) {
        let parsed = Self.parseEvent(line)
        guard let parsed else { return }
        let payloadJSON = Self.payloadJSON(parsed.payload)
        let dispatchScript = ExtensionBridgeJS.dispatchEvent(name: parsed.name, payloadJSON: payloadJSON)
        let box = ContextBox(context)
        DispatchQueue.main.async {
            box.context.evaluateScript(dispatchScript)
        }
    }

    static func parseEvent(_ line: String) -> (name: String, payload: [String: String])? {
        let parts = line.components(separatedBy: "|")
        guard parts.count >= 2, parts[0] == "event" else { return nil }
        let name = parts[1]
        guard !name.isEmpty else { return nil }
        var payload: [String: String] = [:]
        for segment in parts.dropFirst(2) {
            guard let separator = segment.firstIndex(of: "=") else { continue }
            let key = String(segment[segment.startIndex ..< separator])
            let value = String(segment[segment.index(after: separator)...])
            guard !key.isEmpty else { continue }
            payload[key] = value
        }
        return (name, payload)
    }

    static func payloadJSON(_ payload: [String: String]) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: payload),
              let json = String(data: data, encoding: .utf8)
        else { return "{}" }
        return json
    }
}

private struct ContextBox: @unchecked Sendable {
    let context: JSContext
    init(_ context: JSContext) {
        self.context = context
    }
}
