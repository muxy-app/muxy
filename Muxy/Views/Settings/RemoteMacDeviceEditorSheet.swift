import SwiftUI

enum RemoteMacDeviceEditorMode: Identifiable {
    case create
    case edit(RemoteDevice)

    var id: String {
        switch self {
        case .create: "remote-mac-create"
        case let .edit(device): "remote-mac-edit-\(device.id.uuidString)"
        }
    }

    var title: String {
        switch self {
        case .create: "Connect to Muxy Mac"
        case .edit: "Edit Muxy Mac"
        }
    }

    var existingDeviceID: UUID? {
        switch self {
        case .create: nil
        case let .edit(device): device.id
        }
    }

    var existingCredentialScope: String? {
        switch self {
        case .create: nil
        case let .edit(device): device.muxy?.credentialScope
        }
    }
}

struct RemoteMacDeviceEditorSheet: View {
    let mode: RemoteMacDeviceEditorMode
    let onSave: (_ id: UUID, _ name: String, _ connection: MuxyRemoteServerData) -> Void
    let onCancel: () -> Void

    @Environment(RemoteMacWorkspaceStore.self) private var workspaceStore
    @State private var deviceID: UUID
    @State private var discovery = RemoteMacDiscovery()
    @State private var name = ""
    @State private var host = ""
    @State private var port = "4865"
    @State private var serviceName: String?
    @State private var isConnecting = false
    @State private var connectionTask: Task<Void, Never>?
    @State private var didSave = false
    @State private var pendingCredentialScope: String?
    @State private var errorMessage: String?
    @FocusState private var hostFocused: Bool

    private var trimmedName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var trimmedHost: String { host.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var parsedPort: UInt16? { UInt16(port) }
    private var canConnect: Bool {
        !trimmedName.isEmpty && MuxyRemoteServerData.isValidHost(trimmedHost) && parsedPort != nil && !isConnecting
    }

    init(
        mode: RemoteMacDeviceEditorMode,
        onSave: @escaping (_ id: UUID, _ name: String, _ connection: MuxyRemoteServerData) -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.mode = mode
        self.onSave = onSave
        self.onCancel = onCancel
        _deviceID = State(initialValue: mode.existingDeviceID ?? UUID())
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
            Text(mode.title)
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            if !discovery.devices.isEmpty {
                discoveredDevices
            }

            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                field("Name", placeholder: "Studio Mac", text: $name)
                field("Host", placeholder: "mac.local or 192.168.1.10", text: $host)
                    .focused($hostFocused)
                field("Port", placeholder: "4865", text: $port)
            }

            Text("The first connection asks for approval on the remote Mac. Use a trusted network or VPN.")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)

            if let errorMessage {
                Label(errorMessage, systemImage: "exclamationmark.triangle.fill")
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(.orange)
            }

            HStack(spacing: UIMetrics.spacing3) {
                Spacer()
                Button("Cancel") {
                    cancel()
                    onCancel()
                }
                .keyboardShortcut(.cancelAction)
                .disabled(isConnecting)
                Button {
                    connect()
                } label: {
                    if isConnecting {
                        ProgressView().controlSize(.small)
                    } else {
                        Text("Connect")
                    }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(!canConnect)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(460))
        .onAppear {
            loadInitialValues()
            discovery.start()
            hostFocused = host.isEmpty
        }
        .onDisappear {
            discovery.stop()
            cancel()
        }
    }

    private var discoveredDevices: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            Text("Nearby Macs")
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
            ForEach(discovery.devices) { device in
                Button {
                    name = device.name
                    host = device.host
                    port = String(device.port)
                    serviceName = device.name
                } label: {
                    HStack {
                        Image(systemName: "desktopcomputer")
                        VStack(alignment: .leading, spacing: 1) {
                            Text(device.name)
                            Text("\(device.host):\(device.port)")
                                .font(.system(size: UIMetrics.fontCaption))
                                .foregroundStyle(MuxyTheme.fgMuted)
                        }
                        Spacer()
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .padding(UIMetrics.spacing3)
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            }
        }
    }

    private func field(_ label: String, placeholder: String, text: Binding<String>) -> some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            Text(label)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
            TextField(placeholder, text: text)
                .textFieldStyle(.roundedBorder)
        }
    }

    private func loadInitialValues() {
        guard case let .edit(device) = mode, let connection = device.muxy else { return }
        name = device.name
        host = connection.host
        port = String(connection.port)
        serviceName = connection.serviceName
    }

    private func connect() {
        guard let parsedPort, connectionTask == nil else { return }
        let connection = MuxyRemoteServerData(host: trimmedHost, port: parsedPort, serviceName: serviceName)
        let device = RemoteDevice(id: deviceID, name: trimmedName, muxy: connection)
        pendingCredentialScope = connection.credentialScope
        isConnecting = true
        errorMessage = nil
        connectionTask = Task {
            defer {
                connectionTask = nil
                isConnecting = false
            }
            do {
                try Task.checkCancellation()
                try await workspaceStore.connectForSetup(device)
                try Task.checkCancellation()
                didSave = true
                onSave(device.id, device.name, connection)
            } catch {
                guard !Task.isCancelled else { return }
                errorMessage = error.localizedDescription
            }
        }
    }

    private func cancel() {
        connectionTask?.cancel()
        connectionTask = nil
        let discardCredentialScope: String? = if didSave || pendingCredentialScope == mode.existingCredentialScope {
            nil
        } else {
            pendingCredentialScope
        }
        workspaceStore.cancelSetup(for: deviceID, discardCredentialScope: discardCredentialScope)
    }
}
