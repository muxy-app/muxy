import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionConsentService")
@MainActor
struct ExtensionConsentServiceTests {
    @Test("gate auto-allows when a matching allow rule exists")
    func gateUsesExistingAllowRule() async {
        let grantStore = makeGrantStore()
        let auditLog = makeAuditLog()
        let service = ExtensionConsentService(grantStore: grantStore, auditLog: auditLog)
        grantStore.add(ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: .argvExact(["echo"]),
            decision: .allow
        ))
        let request = ExtensionConsentRequestBuilder.make(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["echo"], shell: nil),
            source: "test"
        )
        let decision = await service.gate(request)
        #expect(decision == .allow)
    }

    @Test("gate auto-denies when a matching deny rule exists")
    func gateUsesExistingDenyRule() async {
        let grantStore = makeGrantStore()
        let service = ExtensionConsentService(grantStore: grantStore, auditLog: makeAuditLog())
        grantStore.add(ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: .any,
            decision: .deny
        ))
        let request = ExtensionConsentRequestBuilder.make(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["echo"], shell: nil),
            source: "test"
        )
        let decision = await service.gate(request)
        #expect(decision == .deny)
    }

    @Test("allowAndRemember persists a rule")
    func allowAndRememberPersists() async {
        let grantStore = makeGrantStore()
        let service = ExtensionConsentService(grantStore: grantStore, auditLog: makeAuditLog())
        let request = ExtensionConsentRequestBuilder.make(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["echo", "hi"], shell: nil),
            source: "test"
        )

        async let decision = service.gate(request)
        await waitUntil { service.pendingPrompt?.id == request.id }
        #expect(service.pendingPrompt?.id == request.id)
        service.respond(requestID: request.id, choice: .allowAndRemember)

        let resolved = await decision
        #expect(resolved == .allow)
        #expect(grantStore.rules.count == 1)
        #expect(grantStore.rules.first?.decision == .allow)
    }

    @Test("denyOnce does not persist a rule")
    func denyOnceDoesNotPersist() async {
        let grantStore = makeGrantStore()
        let service = ExtensionConsentService(grantStore: grantStore, auditLog: makeAuditLog())
        let request = ExtensionConsentRequestBuilder.make(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["echo"], shell: nil),
            source: "test"
        )

        async let decision = service.gate(request)
        await waitUntil { service.pendingPrompt?.id == request.id }
        service.respond(requestID: request.id, choice: .denyOnce)
        let resolved = await decision
        #expect(resolved == .deny)
        #expect(grantStore.rules.isEmpty)
    }

    @Test("blockKind overrides existing allow rules and silences future prompts")
    func blockKindBlocksWholeVerb() async {
        let grantStore = makeGrantStore()
        let service = ExtensionConsentService(grantStore: grantStore, auditLog: makeAuditLog())
        grantStore.add(ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: .argvPrefix(["git"]),
            decision: .allow
        ))
        let request = ExtensionConsentRequestBuilder.make(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["npm", "install"], shell: nil),
            source: "test"
        )

        async let decision = service.gate(request)
        await waitUntil { service.pendingPrompt?.id == request.id }
        service.respond(requestID: request.id, choice: .blockKind)

        let resolved = await decision
        #expect(resolved == .deny)
        #expect(grantStore.rules.count == 1)
        #expect(grantStore.rules.first?.match == .any)
        #expect(grantStore.rules.first?.decision == .blocked)

        let previouslyAllowed = ExtensionConsentRequestBuilder.make(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["git", "status"], shell: nil),
            source: "test"
        )
        let allowedDecision = await service.gate(previouslyAllowed)
        #expect(allowedDecision == .deny)
        #expect(service.pendingPrompt == nil)
    }

    @Test("queue flood for one extension auto-denies excess")
    func queueFloodAutoDenies() async {
        let grantStore = makeGrantStore()
        let service = ExtensionConsentService(grantStore: grantStore, auditLog: makeAuditLog())
        let cap = ExtensionConsentService.maxQueuedPromptsPerExtension
        var pending: [Task<ExtensionGrantDecision, Never>] = []
        for index in 0..<cap {
            let request = ExtensionConsentRequestBuilder.make(
                extensionID: "noisy",
                verb: .exec,
                payload: .exec(argv: ["cmd-\(index)"], shell: nil),
                source: "test"
            )
            pending.append(Task { await service.gate(request) })
        }
        await waitUntil {
            (service.pendingPrompt == nil ? 0 : 1) + service.queuedPrompts.count == cap
        }

        let overflow = ExtensionConsentRequestBuilder.make(
            extensionID: "noisy",
            verb: .exec,
            payload: .exec(argv: ["overflow"], shell: nil),
            source: "test"
        )
        let overflowDecision = await service.gate(overflow)
        #expect(overflowDecision == .deny)

        if let first = service.pendingPrompt {
            service.respond(requestID: first.id, choice: .denyOnce)
        }
        for task in pending {
            _ = await task.value
            if let next = service.pendingPrompt {
                service.respond(requestID: next.id, choice: .denyOnce)
            }
        }
    }

    @Test("second prompt queues behind the first")
    func queuesSecondPrompt() async {
        let grantStore = makeGrantStore()
        let service = ExtensionConsentService(grantStore: grantStore, auditLog: makeAuditLog())
        let first = ExtensionConsentRequestBuilder.make(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["one"], shell: nil),
            source: "test"
        )
        let second = ExtensionConsentRequestBuilder.make(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["two"], shell: nil),
            source: "test"
        )
        async let firstDecision = service.gate(first)
        await waitUntil { service.pendingPrompt?.id == first.id }

        async let secondDecision = service.gate(second)
        await waitUntil { service.queuedPrompts.count == 1 }
        #expect(service.pendingPrompt?.id == first.id)
        #expect(service.queuedPrompts.first?.id == second.id)

        service.respond(requestID: first.id, choice: .allowOnce)
        await waitUntil { service.pendingPrompt?.id == second.id }
        service.respond(requestID: second.id, choice: .denyOnce)
        let firstResult = await firstDecision
        let secondResult = await secondDecision
        #expect(firstResult == .allow)
        #expect(secondResult == .deny)
    }

    private func waitUntil(
        timeout: Duration = .seconds(2),
        _ condition: () -> Bool
    ) async {
        let deadline = ContinuousClock.now + timeout
        while !condition() {
            if ContinuousClock.now >= deadline { return }
            await Task.yield()
        }
    }

    private func makeGrantStore() -> ExtensionGrantStore {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-consent-grant-\(UUID().uuidString).json")
        return ExtensionGrantStore(fileURL: url)
    }

    private func makeAuditLog() -> ExtensionAuditLog {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-consent-audit-\(UUID().uuidString).log")
        return ExtensionAuditLog(fileURL: url)
    }
}
