import Foundation
import JavaScriptCore
import MuxyShared
import Testing

@Suite("Extension bridge execAsync")
struct ExtensionBridgeExecAsyncTests {
    private final class Capture {
        var resolve: JSValue?
        var reject: JSValue?
        var cancelledJobID: String?
        var fedTitle: String?
        var finishedQueryID: Int?
    }

    @Test("is absent from the background surface")
    func absentFromBackgroundSurface() {
        let context = JSContext()!
        let dispatch: @convention(block) (String, [String: Any]) -> [String: Any] = { _, _ in
            ["ok": true, "value": NSNull()]
        }
        context.setObject(dispatch, forKeyedSubscript: "__muxyDispatch" as NSString)
        context.evaluateScript(ExtensionBridgeJS.script(extensionID: "demo", surface: .background))

        #expect(context.evaluateScript("typeof muxy.execAsync")?.toString() == "undefined")
    }

    @Test("modal onQuery maps an execAsync result into rows")
    func modalOnQueryMapsExecResultIntoRows() {
        let context = JSContext()!
        let capture = Capture()
        let dispatch: @convention(block) (String, [String: Any]) -> [String: Any] = { verb, payload in
            if verb == "modal.open" {
                return ["ok": true, "value": ["requestID": "modal-1"]]
            }
            if verb == "modal.feed",
               let items = payload["items"] as? [[String: Any]],
               let first = items.first
            {
                capture.fedTitle = first["title"] as? String
            }
            if verb == "modal.finish" {
                capture.finishedQueryID = payload["queryID"] as? Int
            }
            return ["ok": true, "value": NSNull()]
        }
        context.setObject(dispatch, forKeyedSubscript: "__muxyDispatch" as NSString)
        let start: @convention(block) (JSValue, JSValue, JSValue) -> String = { _, resolve, reject in
            capture.resolve = resolve
            capture.reject = reject
            return "search-job"
        }
        context.setObject(start, forKeyedSubscript: "__muxyStartExecAsync" as NSString)
        let cancel: @convention(block) (String) -> Bool = { _ in true }
        context.setObject(cancel, forKeyedSubscript: "__muxyCancelExec" as NSString)
        context.evaluateScript(ExtensionBridgeJS.script(extensionID: "demo", surface: .inProcess))

        context.evaluateScript("""
        muxy.modal.open({
          items: [],
          onQuery(query) {
            const job = muxy.execAsync(['/usr/bin/printf', query]);
            return job.result.then((result) => [{ id: 'hit', title: result.stdout }]);
          },
        });
        __muxyDeliverModalQuery('modal-1', 7, 'needle', {});
        """)
        capture.resolve?.call(withArguments: [[
            "stdout": "needle",
            "stderr": "",
            "exitCode": 0,
            "timedOut": false,
            "truncated": false,
        ]])

        #expect(capture.fedTitle == "needle")
        #expect(capture.finishedQueryID == 7)
    }

    @Test("returns before native completion and resolves the result")
    func returnsBeforeNativeCompletion() {
        let context = JSContext()!
        let capture = Capture()
        let dispatch: @convention(block) (String, [String: Any]) -> [String: Any] = { _, _ in
            ["ok": true, "value": NSNull()]
        }
        context.setObject(dispatch, forKeyedSubscript: "__muxyDispatch" as NSString)
        let start: @convention(block) (JSValue, JSValue, JSValue) -> String = { _, resolve, reject in
            capture.resolve = resolve
            capture.reject = reject
            return "job-1"
        }
        context.setObject(start, forKeyedSubscript: "__muxyStartExecAsync" as NSString)
        let cancel: @convention(block) (String) -> Bool = { jobID in
            capture.cancelledJobID = jobID
            return true
        }
        context.setObject(cancel, forKeyedSubscript: "__muxyCancelExec" as NSString)
        context.evaluateScript(ExtensionBridgeJS.script(extensionID: "demo", surface: .inProcess))

        context.evaluateScript("""
        globalThis.execSettled = false;
        globalThis.execValue = null;
        globalThis.execJob = muxy.execAsync(['/bin/sleep', '1']);
        execJob.result.then((value) => { execSettled = true; execValue = value; });
        globalThis.execReturned = true;
        """)

        #expect(context.evaluateScript("execReturned")?.toBool() == true)
        #expect(context.evaluateScript("execSettled")?.toBool() == false)
        #expect(context.evaluateScript("execJob.id")?.toString() == "job-1")

        capture.resolve?.call(withArguments: [[
            "stdout": "done",
            "stderr": "",
            "exitCode": 0,
            "timedOut": false,
            "truncated": false,
        ]])

        #expect(context.evaluateScript("execSettled")?.toBool() == true)
        #expect(context.evaluateScript("execValue.stdout")?.toString() == "done")
        #expect(context.evaluateScript("execJob.cancel()")?.toBool() == false)
    }

    @Test("forwards cancellation and maps the cancellation error")
    func forwardsCancellation() {
        let context = JSContext()!
        let capture = Capture()
        let dispatch: @convention(block) (String, [String: Any]) -> [String: Any] = { _, _ in
            ["ok": true, "value": NSNull()]
        }
        context.setObject(dispatch, forKeyedSubscript: "__muxyDispatch" as NSString)
        let start: @convention(block) (JSValue, JSValue, JSValue) -> String = { _, resolve, reject in
            capture.resolve = resolve
            capture.reject = reject
            return "job-2"
        }
        context.setObject(start, forKeyedSubscript: "__muxyStartExecAsync" as NSString)
        let cancel: @convention(block) (String) -> Bool = { jobID in
            capture.cancelledJobID = jobID
            return true
        }
        context.setObject(cancel, forKeyedSubscript: "__muxyCancelExec" as NSString)
        context.evaluateScript(ExtensionBridgeJS.script(extensionID: "demo", surface: .inProcess))

        context.evaluateScript("""
        globalThis.cancelError = null;
        globalThis.cancelJob = muxy.execAsync(['/bin/sleep', '1']);
        cancelJob.result.catch((error) => { cancelError = error; });
        """)

        #expect(context.evaluateScript("cancelJob.cancel()")?.toBool() == true)
        #expect(capture.cancelledJobID == "job-2")
        capture.reject?.call(withArguments: [[
            "message": "exec cancelled",
            "code": "cancelled",
            "cancelled": true,
        ]])

        #expect(context.evaluateScript("cancelError.code")?.toString() == "cancelled")
        #expect(context.evaluateScript("cancelError.cancelled")?.toBool() == true)
        #expect(context.evaluateScript("cancelError.message")?.toString() == "exec cancelled")
        #expect(context.evaluateScript("cancelJob.cancel()")?.toBool() == false)
    }
}
