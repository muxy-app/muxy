import Foundation
import JavaScriptCore
import MuxyShared
import Testing

@testable import Muxy

@Suite("Extension bridge projects parity")
struct ExtensionBridgeProjectsParityTests {
    private func verbKeys(of namespace: String, evaluating script: String, shim: (JSContext) -> Void) -> String? {
        let context = JSContext()!
        shim(context)
        context.evaluateScript(script)
        return context.evaluateScript("Object.keys(muxy.\(namespace)).sort().join(',')")?.toString()
    }

    private func webBridgeKeys(of namespace: String) -> String? {
        verbKeys(
            of: namespace,
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
    }

    private func inProcessBridgeKeys(of namespace: String) -> String? {
        verbKeys(
            of: namespace,
            evaluating: ExtensionBridgeJS.script(extensionID: "demo", surface: .inProcess)
        ) { context in
            let dispatch: @convention(block) (String, [String: Any]) -> [String: Any] = { _, _ in
                ["ok": true, "value": NSNull()]
            }
            context.setObject(dispatch, forKeyedSubscript: "__muxyDispatch" as NSString)
        }
    }

    @Test("sidebar webview bridge exposes the same projects verbs as the in-process bridge")
    func projectsVerbsStayInSyncAcrossBridges() {
        let webKeys = webBridgeKeys(of: "projects")
        let inProcessKeys = inProcessBridgeKeys(of: "projects")

        #expect(webKeys?.isEmpty == false)
        #expect(webKeys == inProcessKeys)
        #expect(webKeys?.contains("create") == true)
        #expect(webKeys?.contains("attach") == true)
        #expect(webKeys?.contains("detach") == true)
    }

    @Test("sidebar webview bridge exposes the same workspaces verbs as the in-process bridge")
    func workspacesVerbsStayInSyncAcrossBridges() {
        let webKeys = webBridgeKeys(of: "workspaces")
        let inProcessKeys = inProcessBridgeKeys(of: "workspaces")

        #expect(webKeys?.isEmpty == false)
        #expect(webKeys == inProcessKeys)
        #expect(webKeys?.contains("create") == true)
        #expect(webKeys?.contains("delete") == true)
    }
}
