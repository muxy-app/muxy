import AppKit
import SwiftUI

struct GeneralSettingsView: View {
    @AppStorage(GeneralSettingsKeys.autoExpandWorktreesOnProjectSwitch)
    private var autoExpandWorktrees = false
    @AppStorage(GeneralSettingsKeys.defaultWorktreeParentPath)
    private var defaultWorktreeParentPath = ""
    @AppStorage(TabCloseConfirmationPreferences.confirmRunningProcessKey)
    private var confirmRunningProcess = true
    @AppStorage(ProjectLifecyclePreferences.keepOpenWhenNoTabsKey)
    private var keepProjectsOpenWhenNoTabs = false
    @AppStorage(UpdateChannel.storageKey)
    private var updateChannelRaw = UpdateChannel.stable.rawValue
    @AppStorage(QuitConfirmationPreferences.confirmQuitKey)
    private var confirmQuit = true
    @State private var sentry = SentryService.shared

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Updates",
                footer: "The Beta channel ships every change merged to main and may be unstable. "
                    + "Switch back to Stable to receive only tagged releases."
            ) {
                SettingsRow("Update channel") {
                    Picker("", selection: channelBinding) {
                        ForEach(UpdateChannel.allCases) { channel in
                            Text(channel.displayName).tag(channel)
                        }
                    }
                    .labelsHidden()
                    .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
                }
            }

            SettingsSection(
                "Sidebar",
                footer: "Automatically reveal worktrees when you switch to a project."
            ) {
                SettingsToggleRow(
                    label: "Auto-expand worktrees on project switch",
                    isOn: $autoExpandWorktrees
                )
            }

            SettingsSection(
                "Projects",
                footer: "Keep projects in the sidebar after closing their last tab. "
                    + "To remove a project afterward, use the right-click menu."
            ) {
                SettingsToggleRow(
                    label: "Keep projects open after closing the last tab",
                    isOn: $keepProjectsOpenWhenNoTabs
                )
            }

            SettingsSection(
                "Worktrees",
                footer: "Muxy creates a project-named subfolder inside this folder. "
                    + "Projects can still override this from the new worktree dialog."
            ) {
                worktreeLocationControl
            }

            SettingsSection("Tabs") {
                SettingsToggleRow(
                    label: "Confirm before closing a tab with a running process",
                    isOn: $confirmRunningProcess
                )
            }

            SettingsSection("Quit", showsDivider: sentry.hasDSN) {
                SettingsToggleRow(
                    label: "Confirm before quitting Muxy",
                    isOn: $confirmQuit
                )
            }

            if sentry.hasDSN {
                SettingsSection(
                    "Diagnostics",
                    footer: "Anonymous crash reports help us fix bugs. "
                        + "Reports never include project paths, file contents, or personal data.",
                    showsDivider: false
                ) {
                    SettingsToggleRow(
                        label: "Send anonymous crash reports",
                        isOn: sentryConsentBinding
                    )
                }
            }
        }
    }

    private var sentryConsentBinding: Binding<Bool> {
        Binding(
            get: { sentry.consent == .allowed },
            set: { newValue in sentry.setConsent(newValue ? .allowed : .denied) }
        )
    }

    private var channelBinding: Binding<UpdateChannel> {
        Binding(
            get: { UpdateChannel(rawValue: updateChannelRaw) ?? .stable },
            set: { newValue in
                updateChannelRaw = newValue.rawValue
                UpdateService.shared.channel = newValue
            }
        )
    }

    private var defaultWorktreeLocationText: String {
        defaultWorktreeParentPath.isEmpty ? "Muxy App Support" : defaultWorktreeParentPath
    }

    private var worktreeLocationControl: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Default path for new worktrees")
                .font(.system(size: SettingsMetrics.labelFontSize))

            HStack(alignment: .center, spacing: 8) {
                pathDisplay
                    .layoutPriority(1)

                Button("Choose Folder...") {
                    chooseDefaultWorktreeParentPath()
                }
                .fixedSize(horizontal: true, vertical: false)

                Button("Use App Default") {
                    defaultWorktreeParentPath = ""
                }
                .fixedSize(horizontal: true, vertical: false)
                .disabled(defaultWorktreeParentPath.isEmpty)
            }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    private var pathDisplay: some View {
        HStack(spacing: 7) {
            Image(systemName: defaultWorktreeParentPath.isEmpty ? "internaldrive" : "folder")
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(.secondary)
                .frame(width: 15)

            Text(defaultWorktreeLocationText)
                .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                .foregroundStyle(defaultWorktreeParentPath.isEmpty ? .secondary : .primary)
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.horizontal, 9)
        .frame(minWidth: 170, maxWidth: .infinity, alignment: .leading)
        .frame(height: 22)
        .background(.quaternary.opacity(0.45), in: RoundedRectangle(cornerRadius: 6))
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(.quaternary.opacity(0.7), lineWidth: 1)
        )
    }

    private func chooseDefaultWorktreeParentPath() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.message = "Select the default folder for new worktrees"
        if let path = WorktreeLocationResolver.normalizedPath(defaultWorktreeParentPath) {
            panel.directoryURL = URL(fileURLWithPath: path, isDirectory: true)
        }
        guard panel.runModal() == .OK, let url = panel.url else { return }
        defaultWorktreeParentPath = url.path
    }
}
