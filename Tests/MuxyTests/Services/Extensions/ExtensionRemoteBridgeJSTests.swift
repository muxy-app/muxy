import Foundation
import JavaScriptCore
import MuxyShared
import Testing

@Suite("Extension background remote bridge JS")
struct ExtensionRemoteBridgeJSTests {
    private final class Capture {
        var resolvedCallID: String?
        var resolvedJSON: String?
        var rejectedCallID: String?
        var rejectedMessage: String?
        var dispatchedVerb: String?
        var dispatchedArgs: [String: Any]?
        var modalOpenArgs: [String: Any]?
        var subscriptions: [String] = []
    }

    private func makeContext() -> (JSContext, Capture) {
        let context = JSContext()!
        let capture = Capture()

        let dispatch: @convention(block) (String, [String: Any]) -> [String: Any] = { verb, args in
            capture.dispatchedVerb = verb
            capture.dispatchedArgs = args
            if verb == "modal.open" {
                capture.modalOpenArgs = args
                return ["ok": true, "value": ["requestID": "modal-1"]]
            }
            return ["ok": true, "value": NSNull()]
        }
        context.setObject(dispatch, forKeyedSubscript: "__muxyDispatch" as NSString)
        let subscribe: @convention(block) (String) -> Void = { name in
            capture.subscriptions.append(name)
        }
        context.setObject(subscribe, forKeyedSubscript: "__muxySubscribe" as NSString)

        let resolve: @convention(block) (String, String) -> Void = { callID, json in
            capture.resolvedCallID = callID
            capture.resolvedJSON = json
        }
        context.setObject(resolve, forKeyedSubscript: "__muxyInvokeResolve" as NSString)
        let reject: @convention(block) (String, String) -> Void = { callID, message in
            capture.rejectedCallID = callID
            capture.rejectedMessage = message
        }
        context.setObject(reject, forKeyedSubscript: "__muxyInvokeReject" as NSString)

        context.evaluateScript(ExtensionBridgeJS.script(extensionID: "demo", surface: .background))
        return (context, capture)
    }

    private func dispatchInvoke(_ context: JSContext, callID: String, action: String, argument: Any) {
        let dispatcher = context.objectForKeyedSubscript("__muxyDispatchInvoke")
        let value = JSValue(object: argument, in: context) ?? JSValue(nullIn: context)
        dispatcher?.call(withArguments: [callID, action, value as Any])
    }

    @Test("resolves with the handler's JSON return value")
    func resolvesReturnValue() {
        let (context, capture) = makeContext()
        context.evaluateScript("muxy.remote.handle('ping', (p) => ({ pong: p.n }));")
        dispatchInvoke(context, callID: "c1", action: "ping", argument: ["n": 7])
        #expect(capture.resolvedCallID == "c1")
        #expect(capture.resolvedJSON == #"{"pong":7}"#)
        #expect(capture.rejectedMessage == nil)
    }

    @Test("rejects when the handler throws synchronously")
    func rejectsSynchronousThrow() {
        let (context, capture) = makeContext()
        context.evaluateScript("muxy.remote.handle('boom', () => { throw new Error('nope'); });")
        dispatchInvoke(context, callID: "c2", action: "boom", argument: NSNull())
        #expect(capture.resolvedJSON == nil)
        #expect(capture.rejectedCallID == "c2")
        #expect(capture.rejectedMessage == "nope")
    }

    @Test("rejects when no handler is registered")
    func rejectsUnknownAction() {
        let (context, capture) = makeContext()
        dispatchInvoke(context, callID: "c3", action: "missing", argument: NSNull())
        #expect(capture.resolvedJSON == nil)
        #expect(capture.rejectedCallID == "c3")
        #expect(capture.rejectedMessage == "no handler registered for 'missing'")
    }

    @Test("unhandle removes a registered handler")
    func unhandleRemovesHandler() {
        let (context, capture) = makeContext()
        context.evaluateScript("muxy.remote.handle('ping', () => 1); muxy.remote.unhandle('ping');")
        dispatchInvoke(context, callID: "c4", action: "ping", argument: NSNull())
        #expect(capture.rejectedMessage == "no handler registered for 'ping'")
    }

    @Test("extension-local event subscriptions stay in the background context")
    func localEventSubscribeDoesNotSubscribeOverSocket() {
        let (context, capture) = makeContext()
        context.evaluateScript("muxy.events.subscribe('extension.panel.request', () => {});")
        #expect(capture.subscriptions.isEmpty)
    }

    @Test("workspace event subscriptions still subscribe over the socket")
    func workspaceEventSubscribeUsesSocket() {
        let (context, capture) = makeContext()
        context.evaluateScript("muxy.events.subscribe('pane.created', () => {});")
        #expect(capture.subscriptions == ["pane.created"])
    }

    @Test("modal query change forwards search options")
    func modalQueryChangeForwardsSearchOptions() {
        let (context, _) = makeContext()
        context.evaluateScript("""
        globalThis.receivedQuery = null;
        globalThis.receivedOptions = null;
        muxy.modal.open({
          items: [],
          onQueryChange(query, options) {
            globalThis.receivedQuery = query;
            globalThis.receivedOptions = options;
          }
        });
        """)

        context.objectForKeyedSubscript("__muxyDeliverModalQuery")?.call(withArguments: [
            "modal-1",
            1,
            "한글",
            ["caseSensitive": true, "wholeWord": true, "regex": true],
        ])

        #expect(context.evaluateScript("globalThis.receivedQuery")?.toString() == "한글")
        #expect(context.evaluateScript("globalThis.receivedOptions.caseSensitive")?.toBool() == true)
        #expect(context.evaluateScript("globalThis.receivedOptions.wholeWord")?.toBool() == true)
        #expect(context.evaluateScript("globalThis.receivedOptions.regex")?.toBool() == true)
    }

    @Test("modal open forwards search toolbar request")
    func modalOpenForwardsSearchToolbarRequest() {
        let (context, capture) = makeContext()
        context.evaluateScript("""
        muxy.modal.open({
          items: [],
          searchToolbar: true
        });
        """)

        #expect(capture.modalOpenArgs?["searchToolbar"] as? Bool == true)
    }

    @Test("events.emit dispatches extension-local JSON payloads")
    func eventsEmitDispatchesPayload() {
        let (context, capture) = makeContext()
        context.evaluateScript("muxy.events.emit('extension.panel.response', { count: 2 });")
        #expect(capture.dispatchedVerb == "events.emit")
        #expect(capture.dispatchedArgs?["event"] as? String == "extension.panel.response")
        let payload = capture.dispatchedArgs?["payload"] as? [String: Any]
        #expect((payload?["count"] as? NSNumber)?.intValue == 2)
    }

    @Test("tabs.open is available in the background context")
    func tabsOpenDispatchesFromBackground() {
        let (context, capture) = makeContext()
        context.evaluateScript("""
        muxy.tabs.open({
            kind: 'extensionWebView',
            extension: { id: 'demo', tabType: 'viewer' },
        });
        """)
        #expect(capture.dispatchedVerb == "tabs.open")
        #expect(capture.dispatchedArgs?["kind"] as? String == "extensionWebView")
        let payload = capture.dispatchedArgs?["extension"] as? [String: Any]
        #expect(payload?["id"] as? String == "demo")
        #expect(payload?["tabType"] as? String == "viewer")
    }

}
