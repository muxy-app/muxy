import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionGrantStore")
@MainActor
struct ExtensionGrantStoreTests {
    @Test("evaluate returns .ask when no rules exist")
    func evaluateAskWhenEmpty() {
        let store = makeStore()
        let result = store.evaluate(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["git", "status"], shell: nil)
        )
        #expect(result == .ask)
    }

    @Test("argvExact rule matches identical argv")
    func argvExactMatch() {
        let store = makeStore()
        let rule = ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: .argvExact(["git", "status"]),
            decision: .allow
        )
        store.add(rule)
        let result = store.evaluate(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["git", "status"], shell: nil)
        )
        #expect(result == .allow(ruleID: rule.id))
    }

    @Test("argvPrefix rule matches longer argv")
    func argvPrefixMatch() {
        let store = makeStore()
        let rule = ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: .argvPrefix(["git", "status"]),
            decision: .allow
        )
        store.add(rule)
        let result = store.evaluate(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["git", "status", "--short"], shell: nil)
        )
        #expect(result == .allow(ruleID: rule.id))
    }

    @Test("argvPrefix does not match shorter argv")
    func argvPrefixRejectsShorter() {
        let store = makeStore()
        store.add(ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: .argvPrefix(["git", "status"]),
            decision: .allow
        ))
        let result = store.evaluate(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["git"], shell: nil)
        )
        #expect(result == .ask)
    }

    @Test("deny rule wins over allow rule on same payload")
    func denyBeatsAllow() {
        let store = makeStore()
        let allow = ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: .argvPrefix(["git"]),
            decision: .allow
        )
        let deny = ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: .argvPrefix(["git"]),
            decision: .deny
        )
        store.add(allow)
        store.add(deny)
        let result = store.evaluate(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["git", "status"], shell: nil)
        )
        if case .deny = result {} else { Issue.record("expected deny, got \(result)") }
    }

    @Test("more specific argvExact beats less specific argvPrefix")
    func specificityOrdering() {
        let store = makeStore()
        store.add(ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: .argvPrefix(["git"]),
            decision: .deny
        ))
        let specificAllow = ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: .argvExact(["git", "status"]),
            decision: .allow
        )
        store.add(specificAllow)
        let result = store.evaluate(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["git", "status"], shell: nil)
        )
        #expect(result == .allow(ruleID: specificAllow.id))
    }

    @Test("paneEquals matches exact pane id")
    func paneEqualsMatch() {
        let store = makeStore()
        let rule = ExtensionGrantRule(
            extensionID: "ext",
            verb: .panesSend,
            match: .paneEquals("abc"),
            decision: .allow
        )
        store.add(rule)
        let result = store.evaluate(
            extensionID: "ext",
            verb: .panesSend,
            payload: .pane(id: "abc")
        )
        #expect(result == .allow(ruleID: rule.id))
    }

    @Test("rules persist across store instances")
    func rulesPersist() throws {
        let url = tempURL()
        defer { try? FileManager.default.removeItem(at: url) }
        let first = ExtensionGrantStore(fileURL: url)
        let rule = ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: .argvExact(["echo", "hi"]),
            decision: .allow
        )
        first.add(rule)
        let second = ExtensionGrantStore(fileURL: url)
        #expect(second.rules.contains { $0.id == rule.id })
    }

    @Test("any-match rules are only applied for matching verb+extension")
    func anyMatchScope() {
        let store = makeStore()
        store.add(ExtensionGrantRule(
            extensionID: "ext-a",
            verb: .exec,
            match: .any,
            decision: .allow
        ))
        let result = store.evaluate(
            extensionID: "ext-b",
            verb: .exec,
            payload: .exec(argv: ["echo"], shell: nil)
        )
        #expect(result == .ask)
    }

    @Test("default remember match for exec uses two-token prefix")
    func defaultRememberPrefix() {
        let match = ExtensionGrantSuggestion.defaultRememberMatch(
            verb: .exec,
            payload: .exec(argv: ["git", "status", "--short"], shell: nil)
        )
        if case let .argvPrefix(tokens) = match {
            #expect(tokens == ["git", "status"])
        } else {
            Issue.record("expected argvPrefix, got \(match)")
        }
    }

    @Test("default remember match for two-token argv uses argvExact")
    func defaultRememberExactForShortArgv() {
        let match = ExtensionGrantSuggestion.defaultRememberMatch(
            verb: .exec,
            payload: .exec(argv: ["npm", "test"], shell: nil)
        )
        if case let .argvExact(tokens) = match {
            #expect(tokens == ["npm", "test"])
        } else {
            Issue.record("expected argvExact, got \(match)")
        }
    }

    @Test("default remember for panes uses paneEquals")
    func defaultRememberPane() {
        let match = ExtensionGrantSuggestion.defaultRememberMatch(
            verb: .panesReadScreen,
            payload: .pane(id: "pane-uuid")
        )
        #expect(match == .paneEquals("pane-uuid"))
    }

    private func makeStore() -> ExtensionGrantStore {
        ExtensionGrantStore(fileURL: tempURL())
    }

    private func tempURL() -> URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-grant-test-\(UUID().uuidString).json")
    }
}
