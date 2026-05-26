import AppKit
import SwiftUI

struct ExtensionsSettingsView: View {
    @State private var store = ExtensionStore.shared

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

            SettingsSection("Installed", showsDivider: false) {
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

    private var logView: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 1) {
                if status.logs.isEmpty {
                    Text("No log output.")
                        .font(.system(size: SettingsMetrics.footnoteFontSize))
                        .foregroundStyle(SettingsStyle.mutedForeground)
                } else {
                    ForEach(Array(status.logs.enumerated()), id: \.offset) { _, line in
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
