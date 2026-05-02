import Foundation
import Yams

struct StartupConfig: Equatable {
    enum Layout: String, Equatable {
        case horizontal
        case vertical
    }

    struct Tab: Equatable {
        let name: String?
        let command: String?
    }

    indirect enum Pane: Equatable {
        case leaf(tabs: [Tab])
        case branch(layout: Layout, panes: [Pane])
    }

    let root: Pane

    static func load(fromProjectPath projectPath: String) -> StartupConfig? {
        let directory = URL(fileURLWithPath: projectPath).appendingPathComponent(".muxy")
        let candidates = ["startup.yaml", "startup.yml", "startup.json"]
        for name in candidates {
            let url = directory.appendingPathComponent(name)
            guard FileManager.default.fileExists(atPath: url.path) else { continue }
            guard let text = try? String(contentsOf: url, encoding: .utf8) else { return nil }
            guard let value = try? Yams.load(yaml: text) else { return nil }
            return parse(value)
        }
        return nil
    }

    static func parse(_ value: Any?) -> StartupConfig? {
        guard let pane = parsePane(value) else { return nil }
        return StartupConfig(root: pane)
    }

    private static func parsePane(_ value: Any?) -> Pane? {
        guard let dict = value as? [String: Any] else { return nil }
        if let panesValue = dict["panes"] {
            guard let panesArray = panesValue as? [Any] else { return nil }
            let children = panesArray.compactMap { parsePane($0) }
            guard !children.isEmpty else { return nil }
            let layout = parseLayout(dict["layout"]) ?? .horizontal
            return .branch(layout: layout, panes: children)
        }
        if let tabsValue = dict["tabs"] {
            guard let tabsArray = tabsValue as? [Any] else { return nil }
            let tabs = tabsArray.compactMap { parseTab($0) }
            guard !tabs.isEmpty else { return nil }
            return .leaf(tabs: tabs)
        }
        return nil
    }

    private static func parseLayout(_ value: Any?) -> Layout? {
        guard let raw = value as? String else { return nil }
        return Layout(rawValue: raw.lowercased())
    }

    private static func parseTab(_ value: Any) -> Tab? {
        if let string = value as? String {
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? nil : Tab(name: nil, command: trimmed)
        }
        guard let dict = value as? [String: Any] else { return nil }
        let name = (dict["name"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines)
        let command = parseCommand(dict["command"])
        if name?.isEmpty ?? true, command?.isEmpty ?? true {
            return Tab(name: nil, command: nil)
        }
        return Tab(
            name: (name?.isEmpty ?? true) ? nil : name,
            command: (command?.isEmpty ?? true) ? nil : command
        )
    }

    private static func parseCommand(_ value: Any?) -> String? {
        if let string = value as? String {
            return string.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        if let array = value as? [String] {
            return array
                .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                .filter { !$0.isEmpty }
                .joined(separator: " && ")
        }
        return nil
    }
}
