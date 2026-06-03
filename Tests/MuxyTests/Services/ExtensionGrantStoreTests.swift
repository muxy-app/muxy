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

    @Test("default remember match for exec uses base command only")
    func defaultRememberBaseCommand() {
        let match = ExtensionGrantSuggestion.defaultRememberMatch(
            verb: .exec,
            payload: .exec(argv: ["git", "status", "--short"], shell: nil)
        )
        #expect(match == .argvPrefix(["git"]))
    }

    @Test("default remember match for shell-form exec uses shellExact")
    func defaultRememberShellForm() {
        let match = ExtensionGrantSuggestion.defaultRememberMatch(
            verb: .exec,
            payload: .exec(argv: nil, shell: "echo hi | grep h")
        )
        #expect(match == .shellExact("echo hi | grep h"))
    }

    @Test("default remember match for empty argv falls back to any")
    func defaultRememberEmptyArgv() {
        let match = ExtensionGrantSuggestion.defaultRememberMatch(
            verb: .exec,
            payload: .exec(argv: [], shell: nil)
        )
        #expect(match == .any)
    }

    @Test("remembered exec allows other subcommands of the same base")
    func rememberedExecAllowsOtherSubcommands() {
        let store = makeStore()
        let rule = ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: ExtensionGrantSuggestion.defaultRememberMatch(
                verb: .exec,
                payload: .exec(argv: ["git", "status"], shell: nil)
            ),
            decision: .allow
        )
        store.add(rule)
        let result = store.evaluate(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["git", "push"], shell: nil)
        )
        #expect(result == .allow(ruleID: rule.id))
    }

    @Test("remembered exec does not allow a different base command")
    func rememberedExecRejectsDifferentBase() {
        let store = makeStore()
        store.add(ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: ExtensionGrantSuggestion.defaultRememberMatch(
                verb: .exec,
                payload: .exec(argv: ["git", "status"], shell: nil)
            ),
            decision: .allow
        ))
        let result = store.evaluate(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["rm", "-rf", "/"], shell: nil)
        )
        #expect(result == .ask)
    }

    @Test("default remember for panes allows the verb for any pane")
    func defaultRememberPane() {
        let match = ExtensionGrantSuggestion.defaultRememberMatch(
            verb: .panesReadScreen,
            payload: .pane(id: "pane-uuid")
        )
        #expect(match == .any)
    }

    @Test("remembered pane verb allows a different pane in a later session")
    func rememberedPaneAllowsDifferentPane() {
        let store = makeStore()
        let rule = ExtensionGrantRule(
            extensionID: "ext",
            verb: .panesSendKeys,
            match: ExtensionGrantSuggestion.defaultRememberMatch(
                verb: .panesSendKeys,
                payload: .pane(id: "pane-a")
            ),
            decision: .allow
        )
        store.add(rule)
        let result = store.evaluate(
            extensionID: "ext",
            verb: .panesSendKeys,
            payload: .pane(id: "pane-b")
        )
        #expect(result == .allow(ruleID: rule.id))
    }

    @Test("default remember for foreign tabs allows the verb for any tab")
    func defaultRememberForeignTab() {
        let match = ExtensionGrantSuggestion.defaultRememberMatch(
            verb: .tabsOpenForeign,
            payload: .foreignTab(targetExtensionID: "target", tabTypeID: "tab")
        )
        #expect(match == .any)
    }

    @Test("remoteActionEquals rule matches only the same action")
    func remoteActionEqualsMatch() {
        let store = makeStore()
        let rule = ExtensionGrantRule(
            extensionID: "ext",
            verb: .remoteInvoke,
            match: .remoteActionEquals("forecast"),
            decision: .allow
        )
        store.add(rule)
        #expect(store.evaluate(
            extensionID: "ext",
            verb: .remoteInvoke,
            payload: .remote(action: "forecast", deviceName: "iPad")
        ) == .allow(ruleID: rule.id))
        #expect(store.evaluate(
            extensionID: "ext",
            verb: .remoteInvoke,
            payload: .remote(action: "other", deviceName: "iPad")
        ) == .ask)
    }

    @Test("remote invoke default remember match scopes to the action")
    func remoteInvokeDefaultRemember() {
        let match = ExtensionGrantSuggestion.defaultRememberMatch(
            verb: .remoteInvoke,
            payload: .remote(action: "forecast", deviceName: "iPad")
        )
        #expect(match == .remoteActionEquals("forecast"))
    }

    @Test("gitOperationEquals rule matches only the same operation")
    func gitOperationEqualsMatch() {
        let store = makeStore()
        let rule = ExtensionGrantRule(
            extensionID: "ext",
            verb: .gitWrite,
            match: .gitOperationEquals("push"),
            decision: .allow
        )
        store.add(rule)
        #expect(store.evaluate(
            extensionID: "ext",
            verb: .gitWrite,
            payload: .git(operation: "push", repoPath: "/repo")
        ) == .allow(ruleID: rule.id))
        #expect(store.evaluate(
            extensionID: "ext",
            verb: .gitWrite,
            payload: .git(operation: "discard", repoPath: "/repo")
        ) == .ask)
    }

    @Test("git write default remember match scopes to the operation")
    func gitWriteDefaultRemember() {
        let match = ExtensionGrantSuggestion.defaultRememberMatch(
            verb: .gitWrite,
            payload: .git(operation: "push", repoPath: "/repo")
        )
        #expect(match == .gitOperationEquals("push"))
    }

    @Test("blockKind replaces every rule for the verb with a blocked any-deny")
    func blockKindReplacesRules() {
        let store = makeStore()
        store.add(ExtensionGrantRule(
            extensionID: "ext",
            verb: .exec,
            match: .argvExact(["git", "status"]),
            decision: .allow
        ))
        store.blockKind(extensionID: "ext", verb: .exec)

        #expect(store.rules.count == 1)
        let rule = store.rules.first
        #expect(rule?.match == .any)
        #expect(rule?.decision == .blocked)
        #expect(store.evaluate(
            extensionID: "ext",
            verb: .exec,
            payload: .exec(argv: ["git", "status"], shell: nil)
        ) == .deny(ruleID: rule!.id))
    }

    @Test("deny-remember on an any-default verb stays deny, not blocked")
    func denyRememberStaysDeny() {
        let store = makeStore()
        store.add(ExtensionGrantRule(
            extensionID: "ext",
            verb: .panesSend,
            match: .any,
            decision: .deny
        ))
        #expect(store.rules.first?.decision == .deny)
    }

    private func makeStore() -> ExtensionGrantStore {
        ExtensionGrantStore(fileURL: tempURL())
    }

    private func tempURL() -> URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-grant-test-\(UUID().uuidString).json")
    }
}
