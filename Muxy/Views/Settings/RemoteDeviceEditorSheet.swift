import AppKit
import SwiftUI

@MainActor
@Observable
final class RemoteDeviceProbeController {
    enum State: Equatable {
        case idle
        case testing
        case succeeded
        case failed(String)
    }

    enum Outcome: Equatable {
        case succeeded
        case failed(String)
        case superseded
    }

    private(set) var state: State = .idle
    private var activeRequestID: UUID?
    private var activeDestination: SSHDestination?
    private var activeTask: Task<Void, Never>?

    @discardableResult
    func run(
        destination: SSHDestination,
        operation: @escaping @MainActor (SSHDestination) async -> Outcome
    ) -> Task<Void, Never> {
        invalidate()
        let requestID = UUID()
        activeRequestID = requestID
        activeDestination = destination
        state = .testing
        let task = Task { @MainActor [weak self] in
            let outcome = await operation(destination)
            guard let self,
                  !Task.isCancelled,
                  activeRequestID == requestID,
                  activeDestination == destination
            else { return }
            activeTask = nil
            switch outcome {
            case .succeeded:
                state = .succeeded
            case let .failed(message):
                state = .failed(message)
            case .superseded:
                state = .idle
            }
        }
        activeTask = task
        return task
    }

    func invalidate() {
        activeTask?.cancel()
        activeTask = nil
        activeRequestID = nil
        activeDestination = nil
        state = .idle
    }
}

struct RemoteDeviceEditorSheet: View {
    private static let tmuxHelp = "New terminals on this device keep running through SSH disconnects and when Muxy quits, "
        + "then reconnect when available. Requires tmux on the remote device. Existing terminals are not affected."

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
    @State private var keepsSessionsRunningWithTmux = false
    @State private var showAdvanced = false
    @State private var probe = RemoteDeviceProbeController()
    @FocusState private var hostFocused: Bool

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
        SSHDestination.isValidHost(trimmedHost) && isPortValid && environmentErrorMessage == nil && probe.state != .testing
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
        if case let .failure(error) = environmentResult {
            return error.localizedDescription
        }
        return nil
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text(L10n.resource(key: mode.title))
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            field(
                label: L10n.string("Name"),
                placeholder: trimmedHost.isEmpty ? L10n.string("Production") : trimmedHost,
                text: $name
            )
            field(label: L10n.string("SSH Host"), placeholder: L10n.string("host or ~/.ssh/config alias"), text: $host, focused: true)
                .onChange(of: host) { invalidateProbe() }
            field(label: L10n.string("Remote Root"), placeholder: L10n.string("~"), text: $root)
                .onChange(of: root) { invalidateProbe() }

            tmuxSessionToggle

            advancedSection

            statusRow

            HStack(spacing: UIMetrics.spacing3) {
                Button(L10n.string("Test Connection"), action: runTest)
                    .disabled(!canProbe)
                Spacer()
                Button(L10n.string("Cancel"), action: onCancel)
                    .keyboardShortcut(.cancelAction)
                Button(L10n.string("Save"), action: save)
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canSave || probe.state == .testing)
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
            keepsSessionsRunningWithTmux = ssh.remoteSessionMode == .tmux
            showAdvanced = ssh.port != nil
                || ssh.user != nil
                || ssh.identityFile != nil
                || ssh.environment != SSHEnvironmentVariables.default
            hostFocused = true
        }
        .onDisappear { probe.invalidate() }
    }

    private var tmuxSessionToggle: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            Toggle(L10n.resource("Keep terminal sessions running with tmux"), isOn: $keepsSessionsRunningWithTmux)
                .onChange(of: keepsSessionsRunningWithTmux) { invalidateProbe() }
                .accessibilityHint(L10n.string("Requires tmux on the remote device and affects new terminals only."))
            Text(L10n.resource(key: Self.tmuxHelp))
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var advancedSection: some View {
        DisclosureGroup(isExpanded: $showAdvanced) {
            VStack(alignment: .leading, spacing: UIMetrics.scaled(10)) {
                HStack(spacing: UIMetrics.spacing4) {
                    field(label: L10n.string("User"), placeholder: L10n.string("optional"), text: $user)
                        .onChange(of: user) { invalidateProbe() }
                    VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
                        field(label: L10n.string("Port"), placeholder: L10n.string("22"), text: $port)
                            .onChange(of: port) { invalidateProbe() }
                        if !isPortValid {
                            Text(L10n.resource("Port must be between 1 and 65535."))
                                .font(.system(size: UIMetrics.fontFootnote))
                                .foregroundStyle(.orange)
                        }
                    }
                    .frame(width: UIMetrics.scaled(90))
                }
                VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
                    Text(L10n.resource("Identity File"))
                        .font(.system(size: UIMetrics.fontFootnote))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    HStack(spacing: UIMetrics.spacing3) {
                        TextField(L10n.string("~/.ssh/id_ed25519"), text: $identityFile)
                            .textFieldStyle(.roundedBorder)
                            .onChange(of: identityFile) { invalidateProbe() }
                        Button(L10n.string("Browse…"), action: chooseIdentityFile)
                            .fixedSize(horizontal: true, vertical: false)
                    }
                }
                environmentEditor
            }
            .padding(.top, UIMetrics.spacing3)
        } label: {
            Text(L10n.resource("Advanced"))
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
    }

    private var environmentEditor: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            Text(L10n.resource("Environment"))
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
            TextEditor(text: $environmentText)
                .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
                .frame(minHeight: UIMetrics.scaled(72))
                .scrollContentBackground(.hidden)
                .background(MuxyTheme.surface)
                .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
                .onChange(of: environmentText) { invalidateProbe() }
            if let environmentErrorMessage {
                Text(environmentErrorMessage)
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(.orange)
            }
        }
    }

    @ViewBuilder
    private var statusRow: some View {
        switch probe.state {
        case .idle:
            Text(L10n.resource("Muxy uses your system SSH config, keys, and agent. No passwords are stored."))
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
        case .testing:
            HStack(spacing: UIMetrics.spacing2) {
                ProgressView().controlSize(.small)
                Text(L10n.resource(key: keepsSessionsRunningWithTmux ? "Testing connection and tmux…" : "Testing connection…"))
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
        case .succeeded:
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
                Text(L10n.resource(key: keepsSessionsRunningWithTmux ? "Connection and tmux are ready" : "Connection succeeded"))
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
        panel.message = L10n.string("Select an SSH private key")
        guard panel.runModal() == .OK, let url = panel.url else { return }
        identityFile = url.path
        invalidateProbe()
    }

    private var sshData: SSHWorkspaceData {
        SSHWorkspaceData(
            host: trimmedHost,
            remoteRoot: trimmedRoot,
            port: parsedPort,
            user: user,
            identityFile: identityFile,
            environment: (try? environmentResult.get()) ?? [:],
            remoteSessionMode: keepsSessionsRunningWithTmux ? .tmux : .direct
        )
    }

    private func runTest() {
        probe.run(destination: sshData.destination) { destination in
            switch await sshConnections.test(destination: destination) {
            case .succeeded:
                .succeeded
            case .failed:
                .failed(failureMessage(for: destination))
            case .superseded:
                .superseded
            }
        }
    }

    private func invalidateProbe() {
        probe.invalidate()
    }

    private func save() {
        guard canSave else { return }
        onSave(displayName, sshData)
    }

    private func failureMessage(for destination: SSHDestination) -> String {
        if case let .failed(message) = sshConnections.state(for: destination) {
            return message
        }
        return "Connection failed."
    }
}
