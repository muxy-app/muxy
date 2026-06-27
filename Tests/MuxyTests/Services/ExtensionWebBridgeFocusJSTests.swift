import Foundation
import JavaScriptCore
import Testing

@testable import Muxy

@Suite("Extension web bridge focus JS")
struct ExtensionWebBridgeFocusJSTests {
    private func makeContext() -> JSContext {
        let context = JSContext()!
        context.evaluateScript("""
        var window = this;
        var document = { documentElement: { style: { setProperty: function () {} } }, addEventListener: function () {} };
        globalThis.messages = [];
        window.webkit = { messageHandlers: { muxy: { postMessage: function (message) {
            globalThis.messages.push(message);
            const value = message.verb === "modal.open" ? { requestID: "modal-1" } : null;
            return Promise.resolve({ ok: true, value });
        } } } };
        """)
        context.evaluateScript(ExtensionWebBridge.script(
            extensionID: "demo",
            tabInstanceID: "instance-1",
            data: nil,
            theme: [:]
        ))
        return context
    }

    @Test("focus defaults to false")
    func focusDefaultsToFalse() {
        let context = makeContext()
        #expect(context.evaluateScript("muxy.focused")?.toBool() == false)
    }

    @Test("onFocus fires with true when the tab becomes focused")
    func onFocusFiresOnFocusGained() {
        let context = makeContext()
        context.evaluateScript("globalThis.received = null; muxy.onFocus((value) => { globalThis.received = value; });")
        context.evaluateScript(ExtensionWebBridge.focusUpdateScript(focused: true))
        #expect(context.evaluateScript("globalThis.received")?.toBool() == true)
        #expect(context.evaluateScript("muxy.focused")?.toBool() == true)
    }

    @Test("onFocus does not fire when the focus value is unchanged")
    func onFocusSkipsUnchangedValue() {
        let context = makeContext()
        context.evaluateScript("globalThis.count = 0; muxy.onFocus(() => { globalThis.count += 1; });")
        context.evaluateScript(ExtensionWebBridge.focusUpdateScript(focused: false))
        #expect(context.evaluateScript("globalThis.count")?.toInt32() == 0)
    }

    @Test("onFocus unsubscribe stops further callbacks")
    func onFocusUnsubscribeStops() {
        let context = makeContext()
        context.evaluateScript("""
        globalThis.count = 0;
        const off = muxy.onFocus(() => { globalThis.count += 1; });
        off();
        """)
        context.evaluateScript(ExtensionWebBridge.focusUpdateScript(focused: true))
        #expect(context.evaluateScript("globalThis.count")?.toInt32() == 0)
    }

    @Test("modal open forwards dynamic flag for query handlers")
    func modalOpenForwardsDynamicFlagForQueryHandlers() {
        let context = makeContext()
        context.evaluateScript("""
        muxy.modal.open({
          items: [],
          onQueryChange() {}
        });
        """)
        #expect(context.evaluateScript("globalThis.messages[0].args.dynamic")?.toBool() == true)
    }

    @Test("modal query delivery forwards search options")
    func modalQueryDeliveryForwardsSearchOptions() {
        let script = ExtensionWebBridge.script(
            extensionID: "demo",
            tabInstanceID: "instance-1",
            data: nil,
            theme: [:]
        )

        #expect(script.contains("window.__muxyDeliverModalQuery = async (requestID, queryID, query, options)"))
        #expect(script.contains("o.onQueryChange(query, options || {})"))
    }

}
