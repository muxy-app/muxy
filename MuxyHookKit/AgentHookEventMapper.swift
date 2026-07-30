import Foundation
import MuxyShared

public struct MappedAgentHookEvent: Equatable {
    public let phase: AgentHookPhase
    public let title: String
    public let body: String
    public let sessionID: String?
    public let sessionEnded: Bool
    public let metadataOnly: Bool

    public init(
        phase: AgentHookPhase,
        title: String,
        body: String,
        sessionID: String? = nil,
        sessionEnded: Bool = false,
        metadataOnly: Bool = false
    ) {
        self.phase = phase
        self.title = title
        self.body = body
        self.sessionID = sessionID
        self.sessionEnded = sessionEnded
        self.metadataOnly = metadataOnly
    }
}

public enum AgentHookEventMapper {
    public static func map(event: String, providerTitle: String, input: Data) -> MappedAgentHookEvent? {
        let payload = payload(from: input)
        let sessionID = sessionID(from: payload)

        switch event {
        case "session-start",
             "SessionStart":
            return MappedAgentHookEvent(
                phase: .working,
                title: "",
                body: "",
                sessionID: sessionID,
                metadataOnly: true
            )
        case "user-prompt-submit",
             "pre-tool-use",
             "UserPromptSubmit",
             "PreToolUse",
             "beforeSubmitPrompt",
             "userPromptSubmitted",
             "preToolUse":
            return MappedAgentHookEvent(phase: .working, title: "", body: "", sessionID: sessionID)
        case "permission-request",
             "PermissionRequest":
            return MappedAgentHookEvent(
                phase: .waiting,
                title: sanitize(providerTitle),
                body: "Needs attention",
                sessionID: sessionID
            )
        case "notification",
             "Notification":
            return mapNotification(providerTitle: providerTitle, payload: payload, sessionID: sessionID)
        case "stop",
             "Stop",
             "agentStop":
            return MappedAgentHookEvent(
                phase: .finished,
                title: sanitize(providerTitle),
                body: firstValue(in: payload, keys: ["last_assistant_message", "message", "body"])
                    ?? "Session completed",
                sessionID: sessionID
            )
        case "stop-failure",
             "StopFailure":
            return MappedAgentHookEvent(
                phase: .finished,
                title: sanitize(providerTitle),
                body: notificationBody(in: payload, fallback: "Session failed"),
                sessionID: sessionID
            )
        case "errorOccurred":
            return mapErrorOccurred(providerTitle: providerTitle, payload: payload, sessionID: sessionID)
        case "session-end",
             "SessionEnd",
             "sessionEnd":
            return MappedAgentHookEvent(
                phase: .finished,
                title: "",
                body: "",
                sessionID: sessionID,
                sessionEnded: true
            )
        default:
            return nil
        }
    }

    static func sanitize(_ value: String) -> String {
        let flattened = value.map { character in
            character == "\n" || character == "\r" || character == "|" ? " " : character
        }
        return String(flattened.prefix(200))
    }

    private static func payload(from input: Data) -> [String: Any] {
        guard let object = try? JSONSerialization.jsonObject(with: input),
              let payload = object as? [String: Any]
        else { return [:] }
        return payload
    }

    private static func mapNotification(
        providerTitle: String,
        payload: [String: Any],
        sessionID: String?
    ) -> MappedAgentHookEvent? {
        let type = firstValue(in: payload, keys: ["notification_type", "notificationType", "type"]) ?? ""
        let title = sanitize(providerTitle)

        switch type {
        case "auth_success",
             "elicitation_complete",
             "elicitation_response",
             "shell_completed",
             "shell_detached_completed",
             "agent_completed",
             "agent_idle":
            return nil
        case "task_complete":
            return MappedAgentHookEvent(
                phase: .finished,
                title: title,
                body: notificationBody(in: payload, fallback: "Task completed"),
                sessionID: sessionID
            )
        case "agent_error":
            return MappedAgentHookEvent(
                phase: .finished,
                title: title,
                body: notificationBody(in: payload, fallback: "Agent error"),
                sessionID: sessionID
            )
        case "permission_prompt":
            return MappedAgentHookEvent(
                phase: .waiting,
                title: title,
                body: notificationBody(in: payload, fallback: "Permission needed"),
                sessionID: sessionID
            )
        case "elicitation_dialog":
            return MappedAgentHookEvent(
                phase: .waiting,
                title: title,
                body: notificationBody(in: payload, fallback: "Question waiting"),
                sessionID: sessionID
            )
        case "idle_prompt":
            return MappedAgentHookEvent(
                phase: .waiting,
                title: title,
                body: notificationBody(in: payload, fallback: "Idle prompt"),
                sessionID: sessionID
            )
        default:
            return MappedAgentHookEvent(
                phase: .waiting,
                title: title,
                body: notificationBody(in: payload, fallback: "Needs attention"),
                sessionID: sessionID
            )
        }
    }

    private static func mapErrorOccurred(
        providerTitle: String,
        payload: [String: Any],
        sessionID: String?
    ) -> MappedAgentHookEvent? {
        guard payload["recoverable"] as? Bool != true else { return nil }
        let error = payload["error"] as? [String: Any] ?? [:]
        return MappedAgentHookEvent(
            phase: .finished,
            title: sanitize(providerTitle),
            body: firstValue(in: error, keys: ["message"])
                ?? notificationBody(in: payload, fallback: "Session failed"),
            sessionID: sessionID
        )
    }

    private static func sessionID(from payload: [String: Any]) -> String? {
        for key in ["session_id", "sessionID"] {
            guard let value = payload[key] as? String,
                  !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            else { continue }
            return value
        }
        return nil
    }

    private static func notificationBody(in payload: [String: Any], fallback: String) -> String {
        firstValue(in: payload, keys: ["message", "body", "title"]) ?? fallback
    }

    private static func firstValue(in payload: [String: Any], keys: [String]) -> String? {
        for key in keys {
            guard let value = stringValue(payload[key]), !value.isEmpty else { continue }
            return sanitize(value)
        }
        return nil
    }

    private static func stringValue(_ value: Any?) -> String? {
        if let value = value as? String {
            return value
        }
        if let value = value as? Bool {
            return value ? "true" : "false"
        }
        if let value = value as? NSNumber {
            return value.stringValue
        }
        return nil
    }
}
