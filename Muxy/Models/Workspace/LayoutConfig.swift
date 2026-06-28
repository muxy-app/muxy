import Foundation
import Yams

struct LayoutDescriptor: Equatable, Identifiable {
    let name: String
    let url: URL
    var id: String { url.path }
}

struct LayoutConfig: Equatable {
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

    static func directory(forProjectPath projectPath: String) -> URL {
        URL(fileURLWithPath: projectPath)
            .appendingPathComponent(".muxy")
            .appendingPathComponent("layouts")
    }

    static func discover(projectPath: String) -> [LayoutDescriptor] {
        let directory = directory(forProjectPath: projectPath)
        let allowed: Set = ["yaml", "yml", "json"]
        guard let entries = try? FileManager.default.contentsOfDirectory(
            at: directory,
            includingPropertiesForKeys: nil,
            options: [.skipsHiddenFiles]
        )
        else { return [] }
        return entries
            .filter { allowed.contains($0.pathExtension.lowercased()) }
            .map { LayoutDescriptor(name: $0.deletingPathExtension().lastPathComponent, url: $0) }
            .sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    }

    static func load(descriptor: LayoutDescriptor) -> LayoutConfig? {
        guard let text = try? String(contentsOf: descriptor.url, encoding: .utf8) else { return nil }
        guard let value = try? Yams.load(yaml: text) else { return nil }
        return parse(value)
    }

    static func load(projectPath: String, name: String) -> LayoutConfig? {
        guard let descriptor = discover(projectPath: projectPath).first(where: { $0.name == name }) else { return nil }
        return load(descriptor: descriptor)
    }

    static func parse(_ value: Any?) -> LayoutConfig? {
        guard let pane = parsePane(value) else { return nil }
        return LayoutConfig(root: pane)
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
