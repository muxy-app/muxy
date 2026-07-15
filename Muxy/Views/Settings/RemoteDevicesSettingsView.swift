import AppKit
import SwiftUI

struct RemoteDevicesSettingsView: View {
    @Environment(RemoteDeviceStore.self) private var deviceStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @Environment(SSHConnectionService.self) private var sshConnections
    @Environment(RemoteMacWorkspaceStore.self) private var remoteMacWorkspace

    @State private var sshEditorMode: RemoteDeviceEditorMode?
    @State private var remoteMacEditorMode: RemoteMacDeviceEditorMode?
    @State private var devicePendingDelete: RemoteDevice?

    private static let footerText = """
    SSH devices provide remote shells. Muxy Macs expose their projects and live terminal workspaces. \
    Connect only over a trusted local network or VPN.
    """

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Remote Devices",
                footer: Self.footerText
            ) {
                if deviceStore.devices.isEmpty {
                    emptyState
                } else {
                    ForEach(deviceStore.devices) { device in
                        RemoteDeviceRow(
                            device: device,
                            connectionState: connectionState(for: device),
                            onEdit: { edit(device) },
                            onDelete: { devicePendingDelete = device }
                        )
                    }
                }
                addButton
            }
        }
        .sheet(item: $sshEditorMode) { mode in
            RemoteDeviceEditorSheet(
                mode: mode,
                onSave: { name, ssh in
                    save(mode: mode, name: name, ssh: ssh)
                    sshEditorMode = nil
                },
                onCancel: { sshEditorMode = nil }
            )
        }
        .sheet(item: $remoteMacEditorMode) { mode in
            RemoteMacDeviceEditorSheet(
                mode: mode,
                onSave: { id, name, connection in
                    saveRemoteMac(mode: mode, id: id, name: name, connection: connection)
                    remoteMacEditorMode = nil
                },
                onCancel: { remoteMacEditorMode = nil }
            )
        }
        .alert(
            "Delete “\(devicePendingDelete?.displayName ?? "")”?",
            isPresented: deleteAlertBinding,
            presenting: devicePendingDelete
        ) { device in
            Button("Delete", role: .destructive) {
                deleteDevice(device)
                devicePendingDelete = nil
            }
            .keyboardShortcut(.defaultAction)
            Button("Cancel", role: .cancel) {
                devicePendingDelete = nil
            }
        } message: { device in
            Text(deleteMessage(for: device))
        }
    }

    private var emptyState: some View {
        Text("No remote devices yet.")
            .font(.system(size: SettingsMetrics.labelFontSize))
            .foregroundStyle(SettingsStyle.mutedForeground)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, SettingsMetrics.horizontalPadding)
            .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    private var addButton: some View {
        Menu {
            Button {
                remoteMacEditorMode = .create
            } label: {
                Label("Muxy Mac", systemImage: "desktopcomputer")
            }
            Button {
                sshEditorMode = .create
            } label: {
                Label("SSH Server", systemImage: "network")
            }
        } label: {
            HStack(spacing: 6) {
                Image(systemName: "plus")
                    .font(.system(size: 11, weight: .semibold))
                Text("Add Remote Device")
                    .font(.system(size: SettingsMetrics.labelFontSize, weight: .medium))
            }
            .foregroundStyle(SettingsStyle.accent)
            .padding(.horizontal, SettingsMetrics.horizontalPadding)
            .padding(.vertical, SettingsMetrics.rowVerticalPadding)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
    }

    private var deleteAlertBinding: Binding<Bool> {
        Binding(
            get: { devicePendingDelete != nil },
            set: { if !$0 { devicePendingDelete = nil } }
        )
    }

    private func deleteMessage(for device: RemoteDevice) -> String {
        if device.kind == .muxy {
            return "This removes the saved connection and credentials from this Mac. Revoke it separately on the host Mac."
        }
        let names = projectGroupStore.workspaceNames(usingDevice: device.id)
        guard !names.isEmpty else {
            return "This device is not used by any workspace."
        }
        let list = names.map { "“\($0)”" }.joined(separator: ", ")
        return "Deleting this device also deletes the workspaces using it: \(list). Projects inside those workspaces are not removed."
    }

    private func save(mode: RemoteDeviceEditorMode, name: String, ssh: SSHWorkspaceData) {
        switch mode {
        case .create:
            deviceStore.add(name: name, ssh: ssh)
        case let .edit(device):
            deviceStore.update(id: device.id) {
                $0.name = name
                $0.ssh = ssh
            }
        }
    }

    private func saveRemoteMac(
        mode: RemoteMacDeviceEditorMode,
        id: UUID,
        name: String,
        connection: MuxyRemoteServerData
    ) {
        let wasActive = remoteMacWorkspace.activeDeviceID == id
        let device = deviceStore.add(id: id, name: name, muxy: connection)
        let previousScope = mode.existingCredentialScope
        let retiredScope = previousScope == connection.credentialScope ? nil : previousScope
        remoteMacWorkspace.resetConnection(for: id, retiringCredentialScope: retiredScope)
        if wasActive {
            Task { await remoteMacWorkspace.activate(device) }
        }
    }

    private func edit(_ device: RemoteDevice) {
        switch device.kind {
        case .ssh:
            sshEditorMode = .edit(device)
        case .muxy:
            remoteMacEditorMode = .edit(device)
        }
    }

    private func connectionState(for device: RemoteDevice) -> RemoteDeviceConnectionDisplayState {
        switch device.kind {
        case .ssh:
            .ssh(sshConnections.state(for: device.destination))
        case .muxy:
            .muxy(remoteMacWorkspace.connectionState(for: device.id))
        }
    }

    private func deleteDevice(_ device: RemoteDevice) {
        projectGroupStore.removeWorkspaces(usingDevice: device.id)
        if device.kind == .muxy {
            remoteMacWorkspace.removeDevice(device.id)
        }
        deviceStore.remove(id: device.id)
    }
}

private enum RemoteDeviceConnectionDisplayState {
    case ssh(SSHConnectionState)
    case muxy(RemoteMacConnectionState)
}

private struct RemoteDeviceRow: View {
    let device: RemoteDevice
    let connectionState: RemoteDeviceConnectionDisplayState
    let onEdit: () -> Void
    let onDelete: () -> Void

    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 10) {
            connectionDot
            VStack(alignment: .leading, spacing: 2) {
                Text(device.displayName)
                    .font(.system(size: SettingsMetrics.labelFontSize, weight: .medium))
                    .foregroundStyle(SettingsStyle.foreground)
                Text(connectionDetails)
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(SettingsStyle.mutedForeground)
            }
            Spacer()
            Button("Edit", action: onEdit)
                .buttonStyle(.plain)
                .font(.system(size: SettingsMetrics.footnoteFontSize, weight: .medium))
                .foregroundStyle(SettingsStyle.accent)
            Button {
                onDelete()
            } label: {
                Image(systemName: "trash")
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(SettingsStyle.destructive)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
        .background(
            isHovered ? SettingsStyle.hover : .clear,
            in: RoundedRectangle(cornerRadius: 6)
        )
        .onHover { isHovered = $0 }
    }

    @ViewBuilder
    private var connectionDot: some View {
        switch connectionState {
        case .ssh(.testing),
             .ssh(.connecting),
             .muxy(.connecting),
             .muxy(.awaitingApproval):
            ProgressView().controlSize(.mini)
        case .ssh(.connected),
             .muxy(.connected):
            Circle().fill(.green).frame(width: 6, height: 6)
        case .ssh(.failed),
             .muxy(.failed):
            Circle().fill(SettingsStyle.warning).frame(width: 6, height: 6)
        case .ssh(.disconnected),
             .muxy(.disconnected):
            Circle().fill(SettingsStyle.mutedForeground.opacity(0.4)).frame(width: 6, height: 6)
        }
    }

    private var connectionDetails: String {
        switch device.kind {
        case .ssh:
            device.destination.target
        case .muxy:
            device.muxy?.displayAddress ?? "Invalid address"
        }
    }
}

enum RemoteDeviceEditorMode: Identifiable {
    case create
    case edit(RemoteDevice)

    var id: String {
        switch self {
        case .create: "device-create"
        case let .edit(device): "device-edit-\(device.id.uuidString)"
        }
    }

    var title: String {
        switch self {
        case .create: "Add Remote Device"
        case .edit: "Edit Remote Device"
        }
    }

    var initialName: String {
        switch self {
        case .create: ""
        case let .edit(device): device.name
        }
    }

    var initialSSH: SSHWorkspaceData {
        switch self {
        case .create: SSHWorkspaceData(host: "")
        case let .edit(device): device.ssh
        }
    }
}
