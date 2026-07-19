import Foundation
import MuxyShared

struct AgentHookRuntime {
    private let environment: [String: String]
    private let socketClient: AgentHookSocketClient
    private let failureLogger: AgentHookFailureLogger
    private let ancestorPIDs: () -> [Int32]
    private let timestamp: () -> Int64

    init(
        environment: [String: String] = ProcessInfo.processInfo.environment,
        socketClient: AgentHookSocketClient = AgentHookSocketClient(),
        failureLogger: AgentHookFailureLogger = AgentHookFailureLogger(),
        ancestorPIDs: @escaping () -> [Int32] = { AncestorProcessInspector.ancestorPIDs() },
        timestamp: @escaping () -> Int64 = { Int64(Date().timeIntervalSince1970) }
    ) {
        self.environment = environment
        self.socketClient = socketClient
        self.failureLogger = failureLogger
        self.ancestorPIDs = ancestorPIDs
        self.timestamp = timestamp
    }

    func run(command: AgentHookCommand, input: Data) {
        guard let mapped = AgentHookEventMapper.map(
            event: command.event,
            providerTitle: command.providerTitle,
            input: input
        )
        else { return }
        guard let socketPath = resolvedSocketPath else { return }

        let currentTimestamp = timestamp()
        let paneID = resolvedPaneID
        let message = AgentHookEventMessage(
            provider: command.provider,
            paneID: paneID,
            phase: mapped.phase,
            title: mapped.title,
            body: mapped.body,
            pids: paneID == nil ? ancestorPIDs() : [],
            ts: currentTimestamp
        )

        do {
            try socketClient.send(message, to: socketPath)
        } catch {
            failureLogger.append(
                provider: command.provider,
                event: command.event,
                error: error,
                timestamp: currentTimestamp
            )
        }
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
