import Foundation
import MuxyShared

@MainActor
enum ExtensionBridgeShared {
    static func decodeExecRequest(_ args: [String: Any]) throws -> ExecRequest {
        let argv: [String]? = if let raw = args["argv"] as? [Any] {
            raw.compactMap { $0 as? String }
        } else {
            nil
        }

        let shell = args["shell"] as? String
        let cwd = args["cwd"] as? String
        let stdin = args["stdin"] as? String
        let timeoutMs = (args["timeoutMs"] as? Int)
            ?? (args["timeoutMs"] as? Double).map { Int($0) }

        var env: [String: String]?
        if let raw = args["env"] as? [String: Any] {
            var converted: [String: String] = [:]
            for (key, value) in raw {
                if let stringValue = value as? String {
                    converted[key] = stringValue
                }
            }
            env = converted
        } else {
            env = nil
        }

        if argv == nil, shell == nil {
            throw ExecError.invalidArguments("exec requires argv or shell")
        }
        if argv != nil, shell != nil {
            throw ExecError.invalidArguments("exec accepts either argv or shell, not both")
        }
        if let argv, argv.isEmpty {
            throw ExecError.invalidArguments("exec argv must be non-empty")
        }

        return ExecRequest(
            argv: argv,
            shell: shell,
            cwd: cwd,
            env: env,
            stdin: stdin,
            timeoutMs: timeoutMs
        )
    }

    static func encodeExecResult(_ result: ExecResult) -> [String: Any] {
        [
            "stdout": result.stdout,
            "stderr": result.stderr,
            "exitCode": Int(result.exitCode),
            "timedOut": result.timedOut,
            "truncated": result.truncated,
        ]
    }

    static func decodeHTTPRequest(_ args: [String: Any]) throws -> HTTPRequest {
        guard let url = args["url"] as? String, !url.isEmpty else {
            throw HTTPError.invalidArguments("http requires a url")
        }
        let headers: [String: String]? = (args["headers"] as? [String: Any]).map { raw in
            raw.compactMapValues { $0 as? String }
        }
        let timeoutMs = (args["timeoutMs"] as? Int)
            ?? (args["timeoutMs"] as? Double).map { Int($0) }
        return HTTPRequest(
            url: url,
            method: (args["method"] as? String) ?? "GET",
            headers: headers,
            body: args["body"] as? String,
            timeoutMs: timeoutMs
        )
    }

    static func encodeHTTPResult(_ result: HTTPResult) -> [String: Any] {
        [
            "status": result.status,
            "headers": result.headers,
            "body": result.body,
            "truncated": result.truncated,
        ]
    }

    static func activeWorktreePath(appState: AppState?, worktreeStore: WorktreeStore?) -> String? {
        guard let appState,
              let projectID = appState.activeProjectID,
              let worktreeID = appState.activeWorktreeID[projectID],
              let worktreeStore
        else { return nil }
        return worktreeStore.worktree(projectID: projectID, worktreeID: worktreeID)?.path
    }

    static func decodeExtensionLocalEvent(args: [String: Any]) throws -> ExtensionLocalEvent.Message {
        guard let name = args["event"] as? String, ExtensionLocalEvent.isValidName(name) else {
            throw APIError.invalidArguments("extension events must start with extension.")
        }
        do {
            return try ExtensionLocalEvent.Message(
                name: name,
                payload: ExtensionLocalEvent.encodePayload(args["payload"])
            )
        } catch {
            throw APIError.invalidArguments(
                "event payload must be JSON-serializable and at most \(ExtensionLocalEvent.maxPayloadBytes) bytes"
            )
        }
    }
}
