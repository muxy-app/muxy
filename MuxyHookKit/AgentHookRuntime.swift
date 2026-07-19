import Foundation
import MuxyShared

public struct AgentHookRuntime {
    private let environment: [String: String]
    private let socketClient: AgentHookSocketClient
    private let failureLogger: AgentHookFailureLogger
    private let ancestorPIDs: () -> [Int32]
    private let timestamp: () -> Int64
    private let eventID: () -> String

    public init(
        environment: [String: String] = ProcessInfo.processInfo.environment,
        socketClient: AgentHookSocketClient = AgentHookSocketClient(),
        failureLogger: AgentHookFailureLogger = AgentHookFailureLogger(),
        ancestorPIDs: @escaping () -> [Int32] = { AncestorProcessInspector.ancestorPIDs() },
        timestamp: @escaping () -> Int64 = { Int64(Date().timeIntervalSince1970) },
        eventID: @escaping () -> String = { UUID().uuidString }
    ) {
        self.environment = environment
        self.socketClient = socketClient
        self.failureLogger = failureLogger
        self.ancestorPIDs = ancestorPIDs
        self.timestamp = timestamp
        self.eventID = eventID
    }

    public enum RunResult: Equatable {
        case success
        case failure(String)
    }

    @discardableResult
    public func run(
        command: AgentHookCommand,
        input: Data,
        budget: AgentHookExecutionBudget? = nil
    ) -> RunResult {
        guard let message = message(for: command, input: input) else { return .success }
        guard let socketPath = resolvedSocketPath else {
            return command.test ? .failure("No socket path configured") : .success
        }

        do {
            if let budget {
                try socketClient.send(message, to: socketPath, budget: budget)
            } else {
                try socketClient.send(message, to: socketPath)
            }
            return .success
        } catch {
            failureLogger.append(
                provider: command.provider,
                event: command.event,
                error: error,
                timestamp: message.ts
            )
            return command.test ? .failure(String(describing: error)) : .success
        }
    }

    private func message(for command: AgentHookCommand, input: Data) -> AgentHookEventMessage? {
        if command.test {
            return testMessage(for: command)
        }
        guard let mapped = AgentHookEventMapper.map(
            event: command.event,
            providerTitle: command.providerTitle,
            input: input
        )
        else { return nil }

        let paneID = resolvedPaneID
        return AgentHookEventMessage(
            id: eventID(),
            provider: command.provider,
            paneID: paneID,
            phase: mapped.phase,
            title: mapped.title,
            body: mapped.body,
            pids: paneID == nil ? ancestorPIDs() : [],
            ts: timestamp()
        )
    }

    private func testMessage(for command: AgentHookCommand) -> AgentHookEventMessage {
        let title = command.providerTitle.isEmpty ? "Notifications" : "\(command.providerTitle) test"
        return AgentHookEventMessage(
            id: eventID(),
            provider: command.provider,
            paneID: resolvedPaneID,
            phase: .finished,
            title: title,
            body: "Hook pipeline is working",
            pids: [],
            ts: timestamp(),
            test: true
        )
    }

    private var resolvedPaneID: String? {
        guard let value = environment["MUXY_PANE_ID"], UUID(uuidString: value) != nil else { return nil }
        return value
    }

    private var resolvedSocketPath: String? {
        if let value = environment["MUXY_SOCKET_PATH"], !value.isEmpty {
            return value
        }
        return AgentHookPaths.defaultSocketPath
    }
}
