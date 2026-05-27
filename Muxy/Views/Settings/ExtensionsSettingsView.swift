import AppKit
import SwiftUI

struct ExtensionsSettingsView: View {
    @State private var store = ExtensionStore.shared
    @State private var grantStore = ExtensionGrantStore.shared

    var body: some View {
        SettingsContainer {
            developmentBanner

            SettingsSection("Location") {
                SettingsRow("Extensions Folder") {
                    Button {
                        NSWorkspace.shared.activateFileViewerSelecting([store.rootDirectory])
                    } label: {
                        Text("Reveal in Finder")
                    }
                    .buttonStyle(.plain)
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(SettingsStyle.accent)
                }
                SettingsRow("Path") {
                    Text(displayPath)
                        .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                SettingsRow("Refresh") {
                    Button {
                        store.reload()
                    } label: {
                        Text("Reload Extensions")
                    }
                    .buttonStyle(.plain)
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(SettingsStyle.accent)
                }
            }

            if !store.loadFailures.isEmpty {
                SettingsSection("Load Errors") {
                    ForEach(store.loadFailures) { failure in
                        ExtensionLoadFailureRow(failure: failure)
                    }
                }
            }

            SettingsSection("Installed") {
                if store.statuses.isEmpty {
                    Text("No extensions installed.")
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                        .padding(.horizontal, SettingsMetrics.horizontalPadding)
                        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
                } else {
                    ForEach(store.statuses) { status in
                        ExtensionRow(status: status, store: store)
                    }
                }
            }

            SettingsSection("Permissions", showsDivider: false) {
                ExtensionPermissionsSection(grantStore: grantStore, statuses: store.statuses)
            }
        }
    }

    private var developmentBanner: some View {
        HStack(alignment: .top, spacing: 8) {
            SettingsDevelopmentBadge(text: "DEV")
            Text("Extensions are under active development. APIs, manifest format, and behavior may change without notice.")
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .foregroundStyle(SettingsStyle.mutedForeground)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.top, SettingsMetrics.verticalPadding)
        .padding(.bottom, SettingsMetrics.rowVerticalPadding)
    }

    private var displayPath: String {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let path = store.rootDirectory.path
        guard path.hasPrefix(home) else { return path }
        return "~" + path.dropFirst(home.count)
    }
}

private struct ExtensionLoadFailureRow: View {
    let failure: ExtensionStore.LoadFailure

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(failure.directory.lastPathComponent)
                .font(.system(size: SettingsMetrics.labelFontSize, weight: .semibold))
                .foregroundStyle(SettingsStyle.destructive)
            Text(failure.message)
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .foregroundStyle(SettingsStyle.mutedForeground)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }
}

private struct ExtensionRow: View {
    let status: ExtensionStore.ExtensionStatus
    let store: ExtensionStore
    @State private var expanded = false

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(status.muxyExtension.displayName)
                            .font(.system(size: SettingsMetrics.labelFontSize, weight: .semibold))
                            .foregroundStyle(SettingsStyle.foreground)
                        Text("v\(status.muxyExtension.manifest.version)")
                            .font(.system(size: SettingsMetrics.footnoteFontSize))
                            .foregroundStyle(SettingsStyle.mutedForeground)
                        statusBadge
                    }
                    if let description = status.muxyExtension.manifest.description, !description.isEmpty {
                        Text(description)
                            .font(.system(size: SettingsMetrics.footnoteFontSize))
                            .foregroundStyle(SettingsStyle.mutedForeground)
                    }
                    if !status.muxyExtension.manifest.permissions.isEmpty {
                        Text(permissionsText)
                            .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                            .foregroundStyle(SettingsStyle.dimForeground)
                    }
                    if !status.muxyExtension.manifest.tabTypes.isEmpty {
                        Text(tabTypesText)
                            .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                            .foregroundStyle(SettingsStyle.dimForeground)
                    }
                    if !status.muxyExtension.manifest.commands.isEmpty {
                        Text(commandsText)
                            .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                            .foregroundStyle(SettingsStyle.dimForeground)
                    }
                    if let error = status.lastError {
                        Text(error)
                            .font(.system(size: SettingsMetrics.footnoteFontSize))
                            .foregroundStyle(SettingsStyle.destructive)
                    }
                }
                Spacer()
                VStack(alignment: .trailing, spacing: 6) {
                    Toggle("", isOn: enabledBinding)
                        .labelsHidden()
                        .toggleStyle(.switch)
                        .controlSize(.small)
                    HStack(spacing: 8) {
                        Button {
                            NSWorkspace.shared.activateFileViewerSelecting([status.muxyExtension.directory])
                        } label: {
                            Text("Reveal")
                        }
                        .buttonStyle(.plain)
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(SettingsStyle.accent)
                        Button {
                            expanded.toggle()
                        } label: {
                            Text(expanded ? "Hide Logs" : "Show Logs")
                        }
                        .buttonStyle(.plain)
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(SettingsStyle.accent)
                    }
                }
            }
            .padding(.horizontal, SettingsMetrics.horizontalPadding)
            .padding(.vertical, SettingsMetrics.rowVerticalPadding)

            if expanded {
                logView
                    .padding(.horizontal, SettingsMetrics.horizontalPadding)
                    .padding(.bottom, SettingsMetrics.rowVerticalPadding)
            }
        }
    }

    private var enabledBinding: Binding<Bool> {
        Binding(
            get: { status.muxyExtension.manifest.enabled },
            set: { store.setEnabled($0, for: status.id) }
        )
    }

    private var statusBadge: some View {
        let label = status.isRunning ? "running" : (status.muxyExtension.manifest.enabled ? "stopped" : "disabled")
        let color = status.isRunning ? MuxyTheme.diffAddFg : SettingsStyle.mutedForeground
        return Text(label)
            .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
            .foregroundStyle(color)
            .padding(.horizontal, 6)
            .padding(.vertical, 1)
            .background(color.opacity(0.12), in: RoundedRectangle(cornerRadius: 4))
    }

    private var permissionsText: String {
        "perms: " + status.muxyExtension.manifest.permissions.map(\.rawValue).joined(separator: " ")
    }

    private var tabTypesText: String {
        "tabs: " + status.muxyExtension.manifest.tabTypes.map(\.id).joined(separator: " ")
    }

    private var commandsText: String {
        let descriptions = status.muxyExtension.manifest.commands.map { command in
            let actionLabel = switch command.action {
            case .event: "event"
            case let .openTab(tabType, _): "opens \(tabType)"
            case let .runScript(script): "runs \(script)"
            }
            return "\(command.id)(\(actionLabel))"
        }
        return "commands: " + descriptions.joined(separator: " ")
    }

    private var logView: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Button {
                    NSWorkspace.shared.activateFileViewerSelecting([status.logFileURL])
                } label: {
                    Text("Reveal Log File")
                }
                .buttonStyle(.plain)
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .foregroundStyle(SettingsStyle.accent)
                Text(status.logFileURL.path)
                    .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 1) {
                    let lines = tailLines
                    if lines.isEmpty {
                        Text("No log output.")
                            .font(.system(size: SettingsMetrics.footnoteFontSize))
                            .foregroundStyle(SettingsStyle.mutedForeground)
                    } else {
                        ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                            Text(line)
                                .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                                .foregroundStyle(SettingsStyle.mutedForeground)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }
                }
                .padding(8)
            }
            .frame(maxHeight: 160)
            .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 6))
        }
    }

    private var tailLines: [String] {
        ExtensionLogTail.read(url: status.logFileURL, maxLines: 200)
    }
}

private struct ExtensionPermissionsSection: View {
    let grantStore: ExtensionGrantStore
    let statuses: [ExtensionStore.ExtensionStatus]

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            auditRow
            if grantStore.rules.isEmpty {
                Text("No saved permission rules. Extensions will prompt the first time they call exec, send-keys, or read-screen.")
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .padding(.horizontal, SettingsMetrics.horizontalPadding)
                    .padding(.vertical, SettingsMetrics.rowVerticalPadding)
            } else {
                ForEach(groupedRules, id: \.extensionID) { group in
                    ExtensionGrantGroup(
                        extensionID: group.extensionID,
                        displayName: displayName(for: group.extensionID),
                        rules: group.rules,
                        grantStore: grantStore
                    )
                }
            }
        }
    }

    private var auditRow: some View {
        HStack(spacing: 8) {
            Text("Activity")
                .font(.system(size: SettingsMetrics.labelFontSize, weight: .semibold))
                .foregroundStyle(SettingsStyle.foreground)
            Spacer()
            Button {
                let url = ExtensionAuditLog.shared.auditFileURL
                if FileManager.default.fileExists(atPath: url.path) {
                    NSWorkspace.shared.activateFileViewerSelecting([url])
                } else {
                    NSWorkspace.shared.activateFileViewerSelecting([url.deletingLastPathComponent()])
                }
            } label: {
                Text("Reveal Audit Log")
            }
            .buttonStyle(.plain)
            .font(.system(size: SettingsMetrics.footnoteFontSize))
            .foregroundStyle(SettingsStyle.accent)
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    private struct Group {
        let extensionID: String
        let rules: [ExtensionGrantRule]
    }

    private var groupedRules: [Group] {
        let grouped = Dictionary(grouping: grantStore.rules, by: \.extensionID)
        return grouped
            .map { Group(extensionID: $0.key, rules: $0.value.sorted { $0.createdAt < $1.createdAt }) }
            .sorted { $0.extensionID < $1.extensionID }
    }

    private func displayName(for extensionID: String) -> String {
        statuses.first { $0.id == extensionID }?.muxyExtension.displayName ?? extensionID
    }
}

private struct ExtensionGrantGroup: View {
    let extensionID: String
    let displayName: String
    let rules: [ExtensionGrantRule]
    let grantStore: ExtensionGrantStore

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Text(displayName)
                    .font(.system(size: SettingsMetrics.labelFontSize, weight: .semibold))
                    .foregroundStyle(SettingsStyle.foreground)
                Text("(\(extensionID))")
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(SettingsStyle.mutedForeground)
                Spacer()
                Button("Clear All") {
                    grantStore.removeAll(for: extensionID)
                }
                .buttonStyle(.plain)
                .font(.system(size: SettingsMetrics.footnoteFontSize))
                .foregroundStyle(SettingsStyle.destructive)
            }
            VStack(spacing: 4) {
                ForEach(rules) { rule in
                    ExtensionGrantRuleRow(rule: rule, grantStore: grantStore)
                }
            }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }
}

private struct ExtensionGrantRuleRow: View {
    let rule: ExtensionGrantRule
    let grantStore: ExtensionGrantStore

    var body: some View {
        HStack(spacing: 8) {
            decisionBadge
            Text(rule.verb.rawValue)
                .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
                .foregroundStyle(SettingsStyle.foreground)
                .frame(width: 130, alignment: .leading)
            Text(rule.match.displayString)
                .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                .foregroundStyle(SettingsStyle.mutedForeground)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer()
            Button {
                grantStore.remove(ruleID: rule.id)
            } label: {
                Image(systemName: "xmark.circle.fill")
            }
            .buttonStyle(.plain)
            .foregroundStyle(SettingsStyle.mutedForeground)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 4))
    }

    private var decisionBadge: some View {
        let isAllow = rule.decision == .allow
        let label = isAllow ? "allow" : "deny"
        let color = isAllow ? MuxyTheme.diffAddFg : SettingsStyle.destructive
        return Text(label)
            .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .semibold))
            .foregroundStyle(color)
            .padding(.horizontal, 6)
            .padding(.vertical, 1)
            .background(color.opacity(0.12), in: RoundedRectangle(cornerRadius: 4))
    }
}

enum ExtensionLogTail {
    static func read(url: URL, maxLines: Int) -> [String] {
        guard let data = try? Data(contentsOf: url),
              let text = String(data: data, encoding: .utf8)
        else { return [] }
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        let filtered = (lines.last?.isEmpty == true) ? Array(lines.dropLast()) : lines
        if filtered.count > maxLines {
            return Array(filtered.suffix(maxLines))
        }
        return filtered
    }
}
