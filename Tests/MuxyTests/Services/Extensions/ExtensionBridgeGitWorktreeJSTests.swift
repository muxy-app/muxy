import JavaScriptCore
import MuxyShared
import Testing

@testable import Muxy

@Suite("Extension bridge Git worktree JS")
struct ExtensionBridgeGitWorktreeJSTests {
    @Test("in-process bridge forwards worktree removal timeout")
    func inProcessBridgeForwardsTimeout() {
        let context = JSContext()!
        var verb: String?
        var args: [String: Any]?
        let dispatch: @convention(block) (String, [String: Any]) -> [String: Any] = { dispatchedVerb, dispatchedArgs in
            verb = dispatchedVerb
            args = dispatchedArgs
            return ["ok": true, "value": ["path": "/tmp/worktree", "dirRemoved": true]]
        }
        context.setObject(dispatch, forKeyedSubscript: "__muxyDispatch" as NSString)
        context.evaluateScript(ExtensionBridgeJS.script(extensionID: "demo", surface: .inProcess))

        context.evaluateScript("muxy.git.worktree.remove({ path: '/tmp/worktree', force: true, timeoutMs: 1234 })")

        #expect(verb == "git.worktree.remove")
        #expect(args?["path"] as? String == "/tmp/worktree")
        #expect(args?["force"] as? Bool == true)
        #expect(args?["timeoutMs"] as? Int == 1234)

        context.evaluateScript("muxy.git.worktree.remove({ path: '/tmp/worktree', force: false })")

        #expect(args?["timeoutMs"] is NSNull)
    }

    @Test("web bridge forwards worktree removal timeout")
    func webBridgeForwardsTimeout() {
        let context = JSContext()!
        context.evaluateScript("""
        var window = this;
        var document = { documentElement: { style: { setProperty: function () {} } }, addEventListener: function () {} };
        var capturedMessage = null;
        window.webkit = { messageHandlers: { muxy: { postMessage: function (message) {
          capturedMessage = message;
          return Promise.resolve({ ok: true, value: { path: message.args.path, dirRemoved: true } });
        } } } };
        """)
        context.evaluateScript(ExtensionWebBridge.script(
            extensionID: "demo",
            tabInstanceID: "instance-1",
            data: nil,
            theme: [:]
        ))

        context.evaluateScript("muxy.git.worktree.remove({ path: '/tmp/worktree', force: true, timeoutMs: 1234 })")

        #expect(context.evaluateScript("capturedMessage.verb")?.toString() == "git.worktree.remove")
        #expect(context.evaluateScript("capturedMessage.args.path")?.toString() == "/tmp/worktree")
        #expect(context.evaluateScript("capturedMessage.args.force")?.toBool() == true)
        #expect(context.evaluateScript("capturedMessage.args.timeoutMs")?.toInt32() == 1234)

        context.evaluateScript("muxy.git.worktree.remove({ path: '/tmp/worktree', force: false })")

        #expect(context.evaluateScript("capturedMessage.args.timeoutMs === null")?.toBool() == true)
    }
}
