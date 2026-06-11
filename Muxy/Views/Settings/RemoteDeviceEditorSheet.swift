import AppKit
import SwiftUI

struct RemoteDeviceEditorSheet: View {
    let mode: RemoteDeviceEditorMode
    let onSave: (_ name: String, _ ssh: SSHWorkspaceData) -> Void
    let onCancel: () -> Void

    @Environment(SSHConnectionService.self) private var sshConnections

    @State private var name: String = ""
    @State private var host: String = ""
    @State private var root: String = ""
    @State private var port: String = ""
    @State private var user: String = ""
    @State private var identityFile: String = ""
    @State private var environmentText: String = ""
    @State private var showAdvanced = false
    @State private var probeState: ProbeState = .idle
    @FocusState private var hostFocused: Bool

    private enum ProbeState: Equatable {
        case idle
        case testing
        case succeeded
        case failed(String)
    }

    private var trimmedName: String { name.trimmingCharacters(in: .whitespaces) }
    private var trimmedHost: String { host.trimmingCharacters(in: .whitespaces) }
    private var trimmedRoot: String {
        let value = root.trimmingCharacters(in: .whitespaces)
        return value.isEmpty ? "~" : value
    }

    private var trimmedPort: String { port.trimmingCharacters(in: .whitespaces) }

    private var parsedPort: Int? { Int(trimmedPort) }

    private var isPortValid: Bool {
        guard !trimmedPort.isEmpty else { return true }
        guard let parsedPort else { return false }
        return (1 ... 65535).contains(parsedPort)
    }

    private var canProbe: Bool {
        SSHDestination.isValidHost(trimmedHost) && isPortValid && environmentErrorMessage == nil && probeState != .testing
    }

    private var canSave: Bool {
        SSHDestination.isValidHost(trimmedHost) && isPortValid && environmentErrorMessage == nil && !displayName.isEmpty
    }

    private var displayName: String {
        trimmedName.isEmpty ? trimmedHost : trimmedName
    }

    private var environmentResult: Result<[String: String], SSHEnvironmentTextError> {
        SSHEnvironmentText.parse(environmentText)
    }

    private var environmentErrorMessage: String? {
        if case let .failure(error) = environmentResult { return error.localizedDescription }
        return nil
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text(mode.title)
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            field(label: "Name", placeholder: trimmedHost.isEmpty ? "Production" : trimmedHost, text: $name)
            field(label: "SSH Host", placeholder: "host or ~/.ssh/config alias", text: $host, focused: true)
                .onChange(of: host) { probeState = .idle }
            field(label: "Remote Root", placeholder: "~", text: $root)
                .onChange(of: root) { probeState = .idle }

            advancedSection

            statusRow

            HStack(spacing: UIMetrics.spacing3) {
                Button("Test Connection", action: runTest)
                    .disabled(!canProbe)
                Spacer()
                Button("Cancel", action: onCancel)
                    .keyboardShortcut(.cancelAction)
                Button("Save", action: save)
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canSave || probeState == .testing)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(440))
        .onAppear {
            let ssh = mode.initialSSH
            name = mode.initialName
            host = ssh.host
            root = ssh.remoteRoot
            port = ssh.port.map(String.init) ?? ""
            user = ssh.user ?? ""
            identityFile = ssh.identityFile ?? ""
            environmentText = SSHEnvironmentText.format(ssh.environment)
            showAdvanced = ssh.port != nil
                || ssh.user != nil
                || ssh.identityFile != nil
                || ssh.environment != SSHEnvironmentVariables.default
            hostFocused = true
        }
    }

    private var advancedSection: some View {
        DisclosureGroup(isExpanded: $showAdvanced) {
            VStack(alignment: .leading, spacing: UIMetrics.scaled(10)) {
                HStack(spacing: UIMetrics.spacing4) {
                    field(label: "User", placeholder: "optional", text: $user)
                        .onChange(of: user) { probeState = .idle }
                    VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
                        field(label: "Port", placeholder: "22", text: $port)
                            .onChange(of: port) { probeState = .idle }
                        if !isPortValid {
                            Text("Port must be between 1 and 65535.")
                                .font(.system(size: UIMetrics.fontFootnote))
                                .foregroundStyle(.orange)
                        }
                    }
                    .frame(width: UIMetrics.scaled(90))
                }
                VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
                    Text("Identity File")
                        .font(.system(size: UIMetrics.fontFootnote))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    HStack(spacing: UIMetrics.spacing3) {
                        TextField("~/.ssh/id_ed25519", text: $identityFile)
                            .textFieldStyle(.roundedBorder)
                            .onChange(of: identityFile) { probeState = .idle }
                        Button("Browse…", action: chooseIdentityFile)
                            .fixedSize(horizontal: true, vertical: false)
                    }
                }
                environmentEditor
            }
            .padding(.top, UIMetrics.spacing3)
        } label: {
            Text("Advanced")
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
    }

    private var environmentEditor: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            Text("Environment")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
            TextEditor(text: $environmentText)
                .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
                .frame(minHeight: UIMetrics.scaled(72))
                .scrollContentBackground(.hidden)
                .background(MuxyTheme.surface)
                .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
                .onChange(of: environmentText) { probeState = .idle }
            if let environmentErrorMessage {
                Text(environmentErrorMessage)
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(.orange)
            }
        }
    }

    @ViewBuilder
    private var statusRow: some View {
        switch probeState {
        case .idle:
            Text("Muxy uses your system SSH config, keys, and agent. No passwords are stored.")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
        case .testing:
            HStack(spacing: UIMetrics.spacing2) {
                ProgressView().controlSize(.small)
                Text("Testing connection…")
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
        case .succeeded:
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
                Text("Connection succeeded")
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fg)
            }
        case let .failed(message):
            HStack(alignment: .top, spacing: UIMetrics.spacing2) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                Text(message)
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .textSelection(.enabled)
            }
        }
    }

    private func field(
        label: String,
        placeholder: String,
        text: Binding<String>,
        focused: Bool = false
    ) -> some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            Text(label)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
            if focused {
                TextField(placeholder, text: text)
                    .textFieldStyle(.roundedBorder)
                    .focused($hostFocused)
            } else {
                TextField(placeholder, text: text)
                    .textFieldStyle(.roundedBorder)
            }
        }
    }

    private func chooseIdentityFile() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.showsHiddenFiles = true
        panel.directoryURL = URL(fileURLWithPath: NSString(string: "~/.ssh").expandingTildeInPath)
        panel.message = "Select an SSH private key"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        identityFile = url.path
        probeState = .idle
    }

    private var sshData: SSHWorkspaceData {
        SSHWorkspaceData(
            host: trimmedHost,
            remoteRoot: trimmedRoot,
            port: parsedPort,
            user: user,
            identityFile: identityFile,
            environment: (try? environmentResult.get()) ?? [:]
        )
    }

    private func runTest() {
        probeState = .testing
        let destination = sshData.destination
        Task {
            let success = await sshConnections.test(destination: destination)
            if success {
                probeState = .succeeded
            } else {
                probeState = .failed(failureMessage(for: destination))
            }
        }
    }

    private func save() {
        guard canSave else { return }
        onSave(displayName, sshData)
    }

    private func failureMessage(for destination: SSHDestination) -> String {
        if case let .failed(message) = sshConnections.state(for: destination) { return message }
        return "Connection failed."
    }
}
