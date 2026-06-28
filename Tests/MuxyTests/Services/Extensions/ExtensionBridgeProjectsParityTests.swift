import Foundation
import JavaScriptCore
import MuxyShared
import Testing

@testable import Muxy

@Suite("Extension bridge projects parity")
struct ExtensionBridgeProjectsParityTests {
    private func projectVerbKeys(evaluating script: String, shim: (JSContext) -> Void) -> String? {
        let context = JSContext()!
        shim(context)
        context.evaluateScript(script)
        return context.evaluateScript("Object.keys(muxy.projects).sort().join(',')")?.toString()
    }

    @Test("sidebar webview bridge exposes the same projects verbs as the in-process bridge")
    func projectsVerbsStayInSyncAcrossBridges() {
        let webKeys = projectVerbKeys(
            evaluating: ExtensionWebBridge.script(
                extensionID: "demo",
                tabInstanceID: "instance-1",
                data: nil,
                theme: [:]
            )
        ) { context in
            context.evaluateScript("""
            var window = this;
            var document = { documentElement: { style: { setProperty: function () {} } }, addEventListener: function () {} };
            window.webkit = { messageHandlers: { muxy: { postMessage: function () { return Promise.resolve({ ok: true, value: null }); } } } };
            """)
        }

        let inProcessKeys = projectVerbKeys(
            evaluating: ExtensionBridgeJS.script(extensionID: "demo", surface: .inProcess)
        ) { context in
            let dispatch: @convention(block) (String, [String: Any]) -> [String: Any] = { _, _ in
                ["ok": true, "value": NSNull()]
            }
            context.setObject(dispatch, forKeyedSubscript: "__muxyDispatch" as NSString)
        }

        #expect(webKeys?.isEmpty == false)
        #expect(webKeys == inProcessKeys)
    }
}
