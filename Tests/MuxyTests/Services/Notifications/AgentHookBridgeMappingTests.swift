import Foundation
import MuxyHookKit
import MuxyShared
import Testing

@Suite("Agent hook bridge mapping")
struct AgentHookBridgeMappingTests {
    @Test("command parser accepts the agent event interface in any option order")
    func parsesCommand() {
        let parsed = AgentHookCommand.parse([
            "agent-event",
            "--event", "stop",
            "--provider", "claude_hook",
            "--provider-title", "Claude Code",
        ])

        #expect(parsed == AgentHookCommand(
            provider: "claude_hook",
            providerTitle: "Claude Code",
            event: "stop"
        ))
        #expect(AgentHookCommand.parse(["agent-event", "--provider", "claude_hook"]) == nil)
        #expect(AgentHookCommand.parse(["other"]) == nil)
    }

    @Test("command parser recognizes the test flag")
    func parsesTestFlag() {
        let parsed = AgentHookCommand.parse([
            "agent-event",
            "--provider", "claude_hook",
            "--provider-title", "Claude Code",
            "--event", "test",
            "--test",
        ])

        #expect(parsed == AgentHookCommand(
            provider: "claude_hook",
            providerTitle: "Claude Code",
            event: "test",
            test: true
        ))
    }

    @Test("runtime builds a self-contained synthetic test message flagged as test")
    func runtimeBuildsTestMessage() {
        let captured = MessageBox()
        let runtime = AgentHookRuntime(
            environment: ["MUXY_SOCKET_PATH": "/tmp/live.sock"],
            socketClient: AgentHookSocketClient(sendAttempt: { _, line, _ in
                captured.store(try? AgentHookWireCodec.decodeEventLine(line))
            }),
            ancestorPIDs: { [999] },
            timestamp: { 42 }
        )

        let result = runtime.run(
            command: AgentHookCommand(
                provider: "claude_hook",
                providerTitle: "Claude Code",
                event: "test",
                test: true
            ),
            input: Data()
        )

        #expect(result == .success)
        let message = captured.value
        #expect(message?.test == true)
        #expect(message?.provider == "claude_hook")
        #expect(message?.phase == .finished)
        #expect(message?.title == "Claude Code test")
        #expect(message?.pids.isEmpty == true)
    }

    @Test("runtime reports failure for a test command when delivery fails")
    func runtimeTestReportsFailure() {
        struct DeliveryError: Error {}
        let runtime = AgentHookRuntime(
            environment: ["MUXY_SOCKET_PATH": "/tmp/live.sock"],
            socketClient: AgentHookSocketClient(
                maximumAttempts: 1,
                sendAttempt: { _, _, _ in throw DeliveryError() },
                sleep: { _ in }
            ),
            failureLogger: AgentHookFailureLogger(logFileURL: nil),
            timestamp: { 42 }
        )

        let result = runtime.run(
            command: AgentHookCommand(
                provider: "claude_hook",
                providerTitle: "Claude Code",
                event: "test",
                test: true
            ),
            input: Data()
        )

        guard case .failure = result else {
            Issue.record("expected failure result")
            return
        }
    }

    @Test("runtime never reports failure for non-test commands even on delivery error")
    func runtimeNonTestAlwaysSucceeds() {
        struct DeliveryError: Error {}
        let runtime = AgentHookRuntime(
            environment: ["MUXY_SOCKET_PATH": "/tmp/live.sock"],
            socketClient: AgentHookSocketClient(
                maximumAttempts: 1,
                sendAttempt: { _, _, _ in throw DeliveryError() },
                sleep: { _ in }
            ),
            failureLogger: AgentHookFailureLogger(logFileURL: nil),
            timestamp: { 42 }
        )

        let result = runtime.run(
            command: AgentHookCommand(
                provider: "claude_hook",
                providerTitle: "Claude Code",
                event: "stop"
            ),
            input: Data()
        )

        #expect(result == .success)
    }

    private final class MessageBox: @unchecked Sendable {
        private let lock = NSLock()
        private var storage: AgentHookEventMessage?
        var value: AgentHookEventMessage? { lock.withLock { storage } }
        func store(_ message: AgentHookEventMessage?) { lock.withLock { storage = message } }
    }

    @Test(
        "working event aliases map to an empty working lifecycle",
        arguments: [
            "user-prompt-submit",
            "pre-tool-use",
            "UserPromptSubmit",
            "PreToolUse",
            "beforeSubmitPrompt",
        ]
    )
    func mapsWorkingAliases(event: String) {
        #expect(map(event: event) == MappedAgentHookEvent(phase: .working, title: "", body: ""))
    }

    @Test("permission request aliases map to a waiting notification")
    func mapsPermissionRequests() {
        for event in ["permission-request", "PermissionRequest"] {
            #expect(map(event: event) == MappedAgentHookEvent(
                phase: .waiting,
                title: "Claude Code",
                body: "Needs attention"
            ))
        }
    }

    @Test(
        "notification types map to stable phases and fallbacks",
        arguments: [
            ("task_complete", AgentHookPhase.finished, "Task completed"),
            ("agent_error", AgentHookPhase.finished, "Agent error"),
            ("permission_prompt", AgentHookPhase.waiting, "Permission needed"),
            ("elicitation_dialog", AgentHookPhase.waiting, "Question waiting"),
            ("idle_prompt", AgentHookPhase.waiting, "Idle prompt"),
            ("unknown", AgentHookPhase.waiting, "Needs attention"),
        ]
    )
    func mapsNotificationTypes(type: String, phase: AgentHookPhase, fallback: String) {
        let input = data(["notification_type": type])
        #expect(map(event: "notification", input: input) == MappedAgentHookEvent(
            phase: phase,
            title: "Claude Code",
            body: fallback
        ))
    }

    @Test("notification type aliases and body fallback order are preserved")
    func preservesNotificationValueOrder() {
        let mapped = map(
            event: "Notification",
            input: data([
                "notificationType": "task_complete",
                "message": "First",
                "body": "Second",
                "title": "Third",
            ])
        )
        #expect(mapped == MappedAgentHookEvent(
            phase: .finished,
            title: "Claude Code",
            body: "First"
        ))

        let typeFallback = map(
            event: "notification",
            input: data(["type": "permission_prompt", "body": "Allow this?"])
        )
        #expect(typeFallback?.body == "Allow this?")
    }

    @Test(
        "settled notification types do not emit lifecycle events",
        arguments: ["auth_success", "elicitation_complete", "elicitation_response"]
    )
    func ignoresSettledNotifications(type: String) {
        #expect(map(event: "notification", input: data(["notification_type": type])) == nil)
    }

    @Test("stop mappings preserve completion and failure body fallbacks")
    func mapsStops() {
        let completed = map(
            event: "Stop",
            input: data([
                "last_assistant_message": "Implemented",
                "message": "Ignored",
                "body": "Ignored",
            ])
        )
        #expect(completed == MappedAgentHookEvent(
            phase: .finished,
            title: "Claude Code",
            body: "Implemented"
        ))
        #expect(map(event: "stop", input: Data("invalid".utf8))?.body == "Session completed")
        #expect(map(event: "stop-failure")?.body == "Session failed")
        #expect(map(event: "StopFailure", input: data(["title": "Failed safely"]))?.body == "Failed safely")
    }

    @Test("session end aliases finish silently and unknown events are ignored")
    func mapsSessionEnd() {
        for event in ["session-end", "SessionEnd", "sessionEnd"] {
            #expect(map(event: event) == MappedAgentHookEvent(phase: .finished, title: "", body: ""))
        }
        #expect(map(event: "session-start") == nil)
    }

    @Test("notification text is flattened and limited to 200 characters")
    func sanitizesText() {
        let longBody = "first\nsecond\rthird|" + String(repeating: "x", count: 250)
        let mapped = map(
            event: "notification",
            input: data(["notification_type": "permission_prompt", "message": longBody])
        )

        #expect(mapped?.body.count == 200)
        #expect(mapped?.body.hasPrefix("first second third ") == true)
        #expect(mapped?.body.contains("|") == false)
    }

    private func map(
        event: String,
        input: Data = Data("{}".utf8)
    ) -> MappedAgentHookEvent? {
        AgentHookEventMapper.map(event: event, providerTitle: "Claude Code", input: input)
    }

    private func data(_ object: [String: String]) -> Data {
        (try? JSONSerialization.data(withJSONObject: object)) ?? Data()
    }
}
