import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "ExtensionConsentService")

struct ExtensionConsentRequest: Identifiable {
    let id = UUID()
    let extensionID: String
    let extensionDisplayName: String
    let verb: ExtensionGatedVerb
    let payload: ExtensionGatedPayload
    let payloadSummary: String
    let payloadDetails: [String]
    let suggestedMatch: ExtensionGrantMatch
    let source: String
}

enum ExtensionConsentChoice {
    case allowOnce
    case allowAndRemember
    case denyOnce
    case denyAndRemember
    case blockKind
}

@MainActor
@Observable
final class ExtensionConsentService {
    static let shared = ExtensionConsentService()

    static let promptTimeout: TimeInterval = 60
    static let maxQueuedPromptsPerExtension = 5

    private(set) var pendingPrompt: ExtensionConsentRequest?
    private(set) var queuedPrompts: [ExtensionConsentRequest] = []

    private var continuations: [UUID: CheckedContinuation<ExtensionConsentChoice, Never>] = [:]
    private var timeoutTasks: [UUID: Task<Void, Never>] = [:]

    private let grantStore: ExtensionGrantStore
    private let auditLog: ExtensionAuditLog

    init(
        grantStore: ExtensionGrantStore = .shared,
        auditLog: ExtensionAuditLog = .shared
    ) {
        self.grantStore = grantStore
        self.auditLog = auditLog
    }

    func gate(_ request: ExtensionConsentRequest) async -> ExtensionGrantDecision {
        let evaluation = grantStore.evaluate(
            extensionID: request.extensionID,
            verb: request.verb,
            payload: request.payload
        )
        switch evaluation {
        case let .allow(ruleID):
            recordAudit(request: request, decision: .allow, ruleID: ruleID)
            return .allow
        case let .deny(ruleID):
            recordAudit(request: request, decision: .deny, ruleID: ruleID)
            return .deny
        case .ask:
            return await prompt(request: request)
        }
    }

    func respond(requestID: UUID, choice: ExtensionConsentChoice) {
        guard let continuation = continuations.removeValue(forKey: requestID) else { return }
        timeoutTasks.removeValue(forKey: requestID)?.cancel()
        let request = (pendingPrompt?.id == requestID ? pendingPrompt : nil)
            ?? queuedPrompts.first { $0.id == requestID }

        if let request {
            applyChoice(request: request, choice: choice)
        }
        advanceQueue(removing: requestID)
        continuation.resume(returning: choice)
    }

    private func prompt(request: ExtensionConsentRequest) async -> ExtensionGrantDecision {
        if queuedPromptCount(for: request.extensionID) >= Self.maxQueuedPromptsPerExtension {
            recordAudit(request: request, decision: .deny, ruleID: nil, reason: "queue-flood")
            logger.warning("Auto-denying consent for \(request.extensionID): queue depth exceeded")
            return .deny
        }
        let choice = await withCheckedContinuation { (continuation: CheckedContinuation<ExtensionConsentChoice, Never>) in
            continuations[request.id] = continuation
            if pendingPrompt == nil {
                pendingPrompt = request
            } else {
                queuedPrompts.append(request)
            }
            scheduleTimeout(for: request.id)
        }
        switch choice {
        case .allowOnce,
             .allowAndRemember: return .allow
        case .denyOnce,
             .denyAndRemember,
             .blockKind: return .deny
        }
    }

    private func queuedPromptCount(for extensionID: String) -> Int {
        let pendingCount = pendingPrompt?.extensionID == extensionID ? 1 : 0
        return pendingCount + queuedPrompts.lazy.count(where: { $0.extensionID == extensionID })
    }

    private func applyChoice(request: ExtensionConsentRequest, choice: ExtensionConsentChoice) {
        switch choice {
        case .allowOnce:
            recordAudit(request: request, decision: .allow, ruleID: nil)
        case .denyOnce:
            recordAudit(request: request, decision: .deny, ruleID: nil)
        case .allowAndRemember:
            let rule = ExtensionGrantRule(
                extensionID: request.extensionID,
                verb: request.verb,
                match: request.suggestedMatch,
                decision: .allow
            )
            grantStore.add(rule)
            recordAudit(request: request, decision: .allow, ruleID: rule.id)
        case .denyAndRemember:
            let rule = ExtensionGrantRule(
                extensionID: request.extensionID,
                verb: request.verb,
                match: request.suggestedMatch,
                decision: .deny
            )
            grantStore.add(rule)
            recordAudit(request: request, decision: .deny, ruleID: rule.id)
        case .blockKind:
            let ruleID = grantStore.blockKind(extensionID: request.extensionID, verb: request.verb)
            recordAudit(request: request, decision: .blocked, ruleID: ruleID)
        }
    }

    private func advanceQueue(removing requestID: UUID) {
        if pendingPrompt?.id == requestID {
            pendingPrompt = queuedPrompts.isEmpty ? nil : queuedPrompts.removeFirst()
        } else {
            queuedPrompts.removeAll { $0.id == requestID }
        }
    }

    private func scheduleTimeout(for requestID: UUID) {
        let task = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(Self.promptTimeout))
            guard let self else { return }
            guard let continuation = self.continuations.removeValue(forKey: requestID) else { return }
            self.timeoutTasks.removeValue(forKey: requestID)
            let request = (self.pendingPrompt?.id == requestID ? self.pendingPrompt : nil)
                ?? self.queuedPrompts.first { $0.id == requestID }
            if let request {
                self.recordAudit(request: request, decision: .deny, ruleID: nil, reason: "timeout")
            }
            self.advanceQueue(removing: requestID)
            logger.warning("Extension consent prompt timed out for \(requestID)")
            continuation.resume(returning: .denyOnce)
        }
        timeoutTasks[requestID] = task
    }

    private func recordAudit(
        request: ExtensionConsentRequest,
        decision: ExtensionGrantDecision,
        ruleID: UUID?,
        reason: String? = nil
    ) {
        let payloadSummary = reason.map { "\(request.payloadSummary) [\($0)]" } ?? request.payloadSummary
        auditLog.record(ExtensionAuditEntry(
            timestamp: Date(),
            extensionID: request.extensionID,
            verb: request.verb.rawValue,
            payloadSummary: payloadSummary,
            decision: decision.rawValue,
            ruleID: ruleID?.uuidString,
            source: request.source
        ))
    }
}

enum ExtensionConsentRequestBuilder {
    @MainActor
    static func make(
        extensionID: String,
        verb: ExtensionGatedVerb,
        payload: ExtensionGatedPayload,
        source: String
    ) -> ExtensionConsentRequest {
        let displayName = ExtensionStore.shared.loadedExtension(id: extensionID)?.displayName ?? extensionID
        let (summary, details) = describe(verb: verb, payload: payload)
        let suggested = ExtensionGrantSuggestion.defaultRememberMatch(verb: verb, payload: payload)
        return ExtensionConsentRequest(
            extensionID: extensionID,
            extensionDisplayName: displayName,
            verb: verb,
            payload: payload,
            payloadSummary: summary,
            payloadDetails: details,
            suggestedMatch: suggested,
            source: source
        )
    }

    private static func describe(
        verb: ExtensionGatedVerb,
        payload: ExtensionGatedPayload
    ) -> (summary: String, details: [String]) {
        switch (verb, payload) {
        case let (.exec, .exec(argv, shell)):
            if let argv {
                let joined = argv.joined(separator: " ")
                return (joined, ["argv: \(joined)"])
            }
            if let shell {
                return ("sh -c …", ["shell: \(shell)"])
            }
            return ("(empty)", [])
        case let (.panesSend, .pane(id)):
            return ("send to pane \(id)", ["pane: \(id)"])
        case let (.panesSendKeys, .pane(id)):
            return ("send-keys to pane \(id)", ["pane: \(id)"])
        case let (.panesReadScreen, .pane(id)):
            return ("read screen of pane \(id)", ["pane: \(id)"])
        case let (.tabsOpenForeign, .foreignTab(target, tab)):
            return ("open \(target) tab \(tab)", ["extension: \(target)", "tab type: \(tab)"])
        case let (.remoteInvoke, .remote(action, deviceName)):
            return ("\(deviceName) calls \(action)", ["device: \(deviceName)", "action: \(action)"])
        case let (.gitWrite, .git(operation, repoPath)):
            return ("git \(operation)", ["operation: \(operation)", "repo: \(repoPath)"])
        case let (.filesWrite, .file(operation, path)):
            return ("file \(operation)", ["operation: \(operation)", "path: \(path)"])
        case let (.httpFetch, .http(hostname, method, url)):
            return ("fetch from \(hostname)", ["host: \(hostname)", "method: \(method)", "url: \(url)"])
        case let (.tabsRunCommand, .tabCommand(command)):
            return (command, ["command: \(command)"])
        case let (.projectsDelete, .project(name, path)):
            return ("delete project \(name)", ["project: \(name)", "path: \(path)"])
        default:
            return ("(unknown)", [])
        }
    }
}
