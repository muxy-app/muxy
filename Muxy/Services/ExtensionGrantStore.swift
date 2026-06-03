import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "ExtensionGrantStore")

enum ExtensionGrantDecision: String, Codable, Equatable {
    case allow
    case deny
    case blocked
}

enum ExtensionGatedVerb: String, Codable, CaseIterable {
    case exec
    case panesSend = "panes.send"
    case panesSendKeys = "panes.sendKeys"
    case panesReadScreen = "panes.readScreen"
    case tabsOpenForeign = "tabs.openForeign"
    case remoteInvoke = "remote.invoke"
    case gitWrite = "git.write"
    case filesWrite = "files.write"

    var kindDisplayName: String {
        switch self {
        case .exec: "shell commands"
        case .panesSend: "terminal input"
        case .panesSendKeys: "terminal keystrokes"
        case .panesReadScreen: "terminal output reads"
        case .tabsOpenForeign: "foreign tab opens"
        case .remoteInvoke: "mobile requests"
        case .gitWrite: "git changes"
        case .filesWrite: "file changes"
        }
    }
}

enum ExtensionGrantMatch: Codable, Equatable {
    case any
    case argvExact([String])
    case argvPrefix([String])
    case shellExact(String)
    case paneEquals(String)
    case foreignTabEquals(targetExtensionID: String, tabTypeID: String)
    case remoteActionEquals(String)
    case gitOperationEquals(String)
    case fileOperationEquals(String)

    private enum CodingKeys: String, CodingKey {
        case kind
        case value
        case string
        case target
    }

    private enum Kind: String, Codable {
        case any
        case argvExact
        case argvPrefix
        case shellExact
        case paneEquals
        case foreignTabEquals
        case remoteActionEquals
        case gitOperationEquals
        case fileOperationEquals
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let kind = try container.decode(Kind.self, forKey: .kind)
        switch kind {
        case .any:
            self = .any
        case .argvExact:
            self = try .argvExact(container.decode([String].self, forKey: .value))
        case .argvPrefix:
            self = try .argvPrefix(container.decode([String].self, forKey: .value))
        case .shellExact:
            self = try .shellExact(container.decode(String.self, forKey: .string))
        case .paneEquals:
            self = try .paneEquals(container.decode(String.self, forKey: .string))
        case .foreignTabEquals:
            self = try .foreignTabEquals(
                targetExtensionID: container.decode(String.self, forKey: .target),
                tabTypeID: container.decode(String.self, forKey: .string)
            )
        case .remoteActionEquals:
            self = try .remoteActionEquals(container.decode(String.self, forKey: .string))
        case .gitOperationEquals:
            self = try .gitOperationEquals(container.decode(String.self, forKey: .string))
        case .fileOperationEquals:
            self = try .fileOperationEquals(container.decode(String.self, forKey: .string))
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .any:
            try container.encode(Kind.any, forKey: .kind)
        case let .argvExact(value):
            try container.encode(Kind.argvExact, forKey: .kind)
            try container.encode(value, forKey: .value)
        case let .argvPrefix(value):
            try container.encode(Kind.argvPrefix, forKey: .kind)
            try container.encode(value, forKey: .value)
        case let .shellExact(value):
            try container.encode(Kind.shellExact, forKey: .kind)
            try container.encode(value, forKey: .string)
        case let .paneEquals(value):
            try container.encode(Kind.paneEquals, forKey: .kind)
            try container.encode(value, forKey: .string)
        case let .foreignTabEquals(target, tab):
            try container.encode(Kind.foreignTabEquals, forKey: .kind)
            try container.encode(target, forKey: .target)
            try container.encode(tab, forKey: .string)
        case let .remoteActionEquals(action):
            try container.encode(Kind.remoteActionEquals, forKey: .kind)
            try container.encode(action, forKey: .string)
        case let .gitOperationEquals(operation):
            try container.encode(Kind.gitOperationEquals, forKey: .kind)
            try container.encode(operation, forKey: .string)
        case let .fileOperationEquals(operation):
            try container.encode(Kind.fileOperationEquals, forKey: .kind)
            try container.encode(operation, forKey: .string)
        }
    }

    var specificity: Int {
        switch self {
        case .any: 0
        case .paneEquals,
             .shellExact: 100
        case .remoteActionEquals: 120
        case .gitOperationEquals: 130
        case .fileOperationEquals: 135
        case .foreignTabEquals: 150
        case let .argvPrefix(tokens): 50 + tokens.count
        case let .argvExact(tokens): 200 + tokens.count
        }
    }

    var displayString: String {
        switch self {
        case .any: "(any)"
        case let .argvExact(tokens): tokens.joined(separator: " ")
        case let .argvPrefix(tokens): tokens.joined(separator: " ") + " *"
        case let .shellExact(value): "sh: \(value)"
        case let .paneEquals(value): "pane: \(value)"
        case let .foreignTabEquals(target, tab): "tab: \(target)/\(tab)"
        case let .remoteActionEquals(action): "action: \(action)"
        case let .gitOperationEquals(operation): "git: \(operation)"
        case let .fileOperationEquals(operation): "file: \(operation)"
        }
    }
}

enum ExtensionGatedPayload {
    case exec(argv: [String]?, shell: String?)
    case pane(id: String)
    case foreignTab(targetExtensionID: String, tabTypeID: String)
    case remote(action: String, deviceName: String)
    case git(operation: String, repoPath: String)
    case file(operation: String, path: String)

    func matches(_ match: ExtensionGrantMatch) -> Bool {
        switch (self, match) {
        case (_, .any):
            return true
        case let (.exec(argv, _), .argvExact(expected)):
            return argv == expected
        case let (.exec(argv, _), .argvPrefix(expected)):
            guard let argv else { return false }
            guard argv.count >= expected.count else { return false }
            return Array(argv.prefix(expected.count)) == expected
        case let (.exec(_, shell), .shellExact(expected)):
            return shell == expected
        case let (.pane(id), .paneEquals(expected)):
            return id == expected
        case let (.foreignTab(target, tab), .foreignTabEquals(expectedTarget, expectedTab)):
            return target == expectedTarget && tab == expectedTab
        case let (.remote(action, _), .remoteActionEquals(expected)):
            return action == expected
        case let (.git(operation, _), .gitOperationEquals(expected)):
            return operation == expected
        case let (.file(operation, _), .fileOperationEquals(expected)):
            return operation == expected
        default:
            return false
        }
    }
}

struct ExtensionGrantRule: Codable, Equatable, Identifiable {
    let id: UUID
    let extensionID: String
    let verb: ExtensionGatedVerb
    let match: ExtensionGrantMatch
    let decision: ExtensionGrantDecision
    let createdAt: Date

    init(
        id: UUID = UUID(),
        extensionID: String,
        verb: ExtensionGatedVerb,
        match: ExtensionGrantMatch,
        decision: ExtensionGrantDecision,
        createdAt: Date = Date()
    ) {
        self.id = id
        self.extensionID = extensionID
        self.verb = verb
        self.match = match
        self.decision = decision
        self.createdAt = createdAt
    }
}

enum ExtensionGrantEvaluation: Equatable {
    case allow(ruleID: UUID)
    case deny(ruleID: UUID)
    case ask
}

@MainActor
@Observable
final class ExtensionGrantStore {
    static let shared = ExtensionGrantStore()

    private(set) var rules: [ExtensionGrantRule] = []

    private let fileURL: URL
    private let encoder: JSONEncoder
    private let decoder: JSONDecoder

    init(fileURL: URL = ExtensionGrantStore.defaultFileURL) {
        self.fileURL = fileURL
        encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        load()
    }

    static var defaultFileURL: URL {
        MuxyFileStorage.appSupportDirectory().appendingPathComponent("extension-grants.json")
    }

    func rules(for extensionID: String) -> [ExtensionGrantRule] {
        rules.filter { $0.extensionID == extensionID }
    }

    func evaluate(
        extensionID: String,
        verb: ExtensionGatedVerb,
        payload: ExtensionGatedPayload
    ) -> ExtensionGrantEvaluation {
        let candidates = rules.filter { $0.extensionID == extensionID && $0.verb == verb && payload.matches($0.match) }
        guard !candidates.isEmpty else { return .ask }

        let sorted = candidates.sorted { lhs, rhs in
            if lhs.match.specificity != rhs.match.specificity {
                return lhs.match.specificity > rhs.match.specificity
            }
            if lhs.decision != rhs.decision {
                return rhs.decision == .allow
            }
            return lhs.createdAt < rhs.createdAt
        }

        guard let winner = sorted.first else { return .ask }
        return winner.decision == .allow ? .allow(ruleID: winner.id) : .deny(ruleID: winner.id)
    }

    func add(_ rule: ExtensionGrantRule) {
        rules.removeAll { existing in
            existing.extensionID == rule.extensionID
                && existing.verb == rule.verb
                && existing.match == rule.match
        }
        rules.append(rule)
        save()
    }

    func remove(ruleID: UUID) {
        rules.removeAll { $0.id == ruleID }
        save()
    }

    func removeAll(for extensionID: String) {
        rules.removeAll { $0.extensionID == extensionID }
        save()
    }

    @discardableResult
    func blockKind(extensionID: String, verb: ExtensionGatedVerb) -> UUID {
        rules.removeAll { $0.extensionID == extensionID && $0.verb == verb }
        let rule = ExtensionGrantRule(extensionID: extensionID, verb: verb, match: .any, decision: .blocked)
        rules.append(rule)
        save()
        return rule.id
    }

    private func load() {
        guard FileManager.default.fileExists(atPath: fileURL.path) else { return }
        do {
            let data = try Data(contentsOf: fileURL)
            rules = try decoder.decode([ExtensionGrantRule].self, from: data)
        } catch {
            logger.error("Failed to load extension grants: \(error.localizedDescription)")
            rules = []
        }
    }

    private func save() {
        do {
            let data = try encoder.encode(rules)
            try data.write(to: fileURL, options: .atomic)
            try? FileManager.default.setAttributes(
                [.posixPermissions: FilePermissions.privateFile],
                ofItemAtPath: fileURL.path
            )
        } catch {
            logger.error("Failed to save extension grants: \(error.localizedDescription)")
        }
    }
}

enum ExtensionGrantSuggestion {
    static func defaultRememberMatch(
        verb: ExtensionGatedVerb,
        payload: ExtensionGatedPayload
    ) -> ExtensionGrantMatch {
        switch (verb, payload) {
        case let (.exec, .exec(argv, shell)):
            if let base = argv?.first {
                return .argvPrefix([base])
            }
            if let shell { return .shellExact(shell) }
            return .any
        case let (.remoteInvoke, .remote(action, _)):
            return .remoteActionEquals(action)
        case let (.gitWrite, .git(operation, _)):
            return .gitOperationEquals(operation)
        case let (.filesWrite, .file(operation, _)):
            return .fileOperationEquals(operation)
        case (.panesSend, _),
             (.panesSendKeys, _),
             (.panesReadScreen, _),
             (.tabsOpenForeign, _):
            return .any
        default:
            return .any
        }
    }
}
