import Foundation
import MuxyShared

public struct MappedAgentHookEvent: Equatable {
    public let phase: AgentHookPhase
    public let title: String
    public let body: String

    public init(phase: AgentHookPhase, title: String, body: String) {
        self.phase = phase
        self.title = title
        self.body = body
    }
}

public enum AgentHookEventMapper {
    public static func map(event: String, providerTitle: String, input: Data) -> MappedAgentHookEvent? {
        let payload = payload(from: input)

        switch event {
        case "user-prompt-submit",
             "pre-tool-use",
             "UserPromptSubmit",
             "PreToolUse",
             "beforeSubmitPrompt":
            return MappedAgentHookEvent(phase: .working, title: "", body: "")
        case "permission-request",
             "PermissionRequest":
            return MappedAgentHookEvent(
                phase: .waiting,
                title: sanitize(providerTitle),
                body: "Needs attention"
            )
        case "notification",
             "Notification":
            return mapNotification(providerTitle: providerTitle, payload: payload)
        case "stop",
             "Stop":
            return MappedAgentHookEvent(
                phase: .finished,
                title: sanitize(providerTitle),
                body: firstValue(in: payload, keys: ["last_assistant_message", "message", "body"])
                    ?? "Session completed"
            )
        case "stop-failure",
             "StopFailure":
            return MappedAgentHookEvent(
                phase: .finished,
                title: sanitize(providerTitle),
                body: notificationBody(in: payload, fallback: "Session failed")
            )
        case "session-end",
             "SessionEnd",
             "sessionEnd":
            return MappedAgentHookEvent(phase: .finished, title: "", body: "")
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
        payload: [String: Any]
    ) -> MappedAgentHookEvent? {
        let type = firstValue(in: payload, keys: ["notification_type", "notificationType", "type"]) ?? ""
        let title = sanitize(providerTitle)

        switch type {
        case "auth_success",
             "elicitation_complete",
             "elicitation_response":
            return nil
        case "task_complete":
            return MappedAgentHookEvent(
                phase: .finished,
                title: title,
                body: notificationBody(in: payload, fallback: "Task completed")
            )
        case "agent_error":
            return MappedAgentHookEvent(
                phase: .finished,
                title: title,
                body: notificationBody(in: payload, fallback: "Agent error")
            )
        case "permission_prompt":
            return MappedAgentHookEvent(
                phase: .waiting,
                title: title,
                body: notificationBody(in: payload, fallback: "Permission needed")
            )
        case "elicitation_dialog":
            return MappedAgentHookEvent(
                phase: .waiting,
                title: title,
                body: notificationBody(in: payload, fallback: "Question waiting")
            )
        case "idle_prompt":
            return MappedAgentHookEvent(
                phase: .waiting,
                title: title,
                body: notificationBody(in: payload, fallback: "Idle prompt")
            )
        default:
            return MappedAgentHookEvent(
                phase: .waiting,
                title: title,
                body: notificationBody(in: payload, fallback: "Needs attention")
            )
        }
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
