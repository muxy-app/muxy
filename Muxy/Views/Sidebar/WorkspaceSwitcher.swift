import AppKit
import SwiftUI

struct WorkspaceSwitcher: View {
    let isWide: Bool

    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @Environment(RemoteDeviceStore.self) private var deviceStore
    @Environment(SSHConnectionService.self) private var sshConnections
    @Environment(AppState.self) private var appState
    @Environment(ProjectStore.self) private var projectStore
    @Environment(WorktreeStore.self) private var worktreeStore

    @State private var isShowingPopover = false
    @State private var isTriggerHovered = false
    @State private var editorMode: WorkspaceEditorMode?
    @State private var remoteEditor: RemoteWorkspaceEditorMode?
    @State private var groupPendingDelete: ProjectGroup?

    private var activeGroup: ProjectGroup? {
        guard let id = projectGroupStore.activeGroupID else { return nil }
        return projectGroupStore.groups.first(where: { $0.id == id })
    }

    private var activeLabel: String {
        activeGroup?.name ?? "All Projects"
    }

    var body: some View {
        Group {
            if isWide {
                wideLayout
            } else {
                collapsedLayout
            }
        }
        .sheet(item: $editorMode) { mode in
            WorkspaceEditorSheet(
                mode: mode,
                onSubmit: { name in
                    apply(mode: mode, name: name)
                    editorMode = nil
                },
                onCancel: { editorMode = nil }
            )
        }
        .sheet(item: $remoteEditor) { mode in
            RemoteWorkspaceEditorSheet(
                mode: mode,
                onSubmit: { name, deviceID in
                    applyRemote(mode: mode, name: name, deviceID: deviceID)
                    remoteEditor = nil
                },
                onCancel: { remoteEditor = nil }
            )
        }
        .alert(
            "Delete “\(groupPendingDelete?.name ?? "")”?",
            isPresented: deleteAlertBinding,
            presenting: groupPendingDelete
        ) { group in
            Button("Delete", role: .destructive) {
                projectGroupStore.removeGroup(id: group.id)
                groupPendingDelete = nil
            }
            .keyboardShortcut(.defaultAction)
            Button("Cancel", role: .cancel) {
                groupPendingDelete = nil
            }
        } message: { _ in
            Text("Projects in this workspace will not be deleted.")
        }
    }

    private var wideLayout: some View {
        Button {
            isShowingPopover.toggle()
        } label: {
            HStack(spacing: UIMetrics.spacing2) {
                if activeGroup?.type == .ssh {
                    Image(systemName: "network")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                        .foregroundStyle(MuxyTheme.accent)
                }
                Text(activeLabel)
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .lineLimit(1)
                Spacer()
                Image(systemName: "chevron.down")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            .padding(.horizontal, UIMetrics.spacing4)
            .frame(height: UIMetrics.controlMedium)
            .background(
                isTriggerHovered ? MuxyTheme.hover : MuxyTheme.surface,
                in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { isTriggerHovered = $0 }
        .popover(isPresented: $isShowingPopover, arrowEdge: .bottom) {
            workspacePopover
        }
    }

    private var collapsedLayout: some View {
        Button {
            isShowingPopover.toggle()
        } label: {
            Image(systemName: "chevron.down")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.iconXXL, height: UIMetrics.iconXXL)
                .background(
                    isTriggerHovered ? MuxyTheme.hover : MuxyTheme.surface,
                    in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                )
        }
        .buttonStyle(.plain)
        .onHover { isTriggerHovered = $0 }
        .popover(isPresented: $isShowingPopover, arrowEdge: .trailing) {
            workspacePopover
        }
    }

    private var workspacePopover: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
            allProjectsRow
            Divider()
                .padding(.vertical, UIMetrics.spacing1)
            ForEach(projectGroupStore.groups) { group in
                WorkspaceRow(
                    group: group,
                    isActive: projectGroupStore.activeGroupID == group.id,
                    connectionState: group.type == .ssh ? connectionState(for: group) : nil,
                    onSelect: { select(group) },
                    onRename: {
                        isShowingPopover = false
                        if group.type == .ssh {
                            remoteEditor = .edit(group)
                        } else {
                            editorMode = .rename(group)
                        }
                    },
                    onDelete: {
                        isShowingPopover = false
                        groupPendingDelete = group
                    }
                )
            }
            if !projectGroupStore.groups.isEmpty {
                Divider()
                    .padding(.vertical, UIMetrics.spacing1)
            }
            newWorkspaceButton
        }
        .padding(UIMetrics.spacing3)
        .frame(minWidth: 180)
    }

    private var allProjectsRow: some View {
        Button {
            projectGroupStore.clearGroupSelection()
            selectFirstProject()
            isShowingPopover = false
        } label: {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: projectGroupStore.activeGroupID == nil ? "checkmark" : "")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.accent)
                    .frame(width: UIMetrics.fontCaption)
                Image(systemName: "square.grid.2x2")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: UIMetrics.fontBody)
                Text("All Projects")
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                Spacer()
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .padding(.vertical, UIMetrics.spacing2)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var newWorkspaceButton: some View {
        Menu {
            Button {
                isShowingPopover = false
                editorMode = .create
            } label: {
                Label("Local Workspace", systemImage: "square.stack.3d.up")
            }
            Button {
                isShowingPopover = false
                remoteEditor = .create
            } label: {
                Label("Remote (SSH)", systemImage: "network")
            }
        } label: {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: "plus")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.accent)
                    .frame(width: UIMetrics.fontCaption)
                Text("New Workspace")
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                Spacer()
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .padding(.vertical, UIMetrics.spacing2)
            .contentShape(Rectangle())
        }
        .menuStyle(.button)
        .menuIndicator(.hidden)
        .buttonStyle(.plain)
    }

    private var deleteAlertBinding: Binding<Bool> {
        Binding(
            get: { groupPendingDelete != nil },
            set: { newValue in
                if !newValue {
                    groupPendingDelete = nil
                }
            }
        )
    }

    private func apply(mode: WorkspaceEditorMode, name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        switch mode {
        case .create:
            projectGroupStore.addGroup(name: trimmed)
        case let .rename(group):
            projectGroupStore.renameGroup(id: group.id, to: trimmed)
        }
    }

    private func applyRemote(mode: RemoteWorkspaceEditorMode, name: String, deviceID: UUID) {
        switch mode {
        case .create:
            let group = projectGroupStore.addRemoteWorkspace(name: name, deviceID: deviceID)
            select(group)
        case let .edit(group):
            projectGroupStore.renameGroup(id: group.id, to: name)
            projectGroupStore.updateRemoteWorkspace(id: group.id, deviceID: deviceID)
        }
    }

    private func destination(for group: ProjectGroup) -> SSHDestination? {
        deviceStore.device(id: group.remoteDeviceID)?.destination
    }

    private func connectionState(for group: ProjectGroup) -> SSHConnectionState {
        guard let destination = destination(for: group) else { return .disconnected }
        return sshConnections.state(for: destination)
    }

    private func select(_ group: ProjectGroup) {
        guard group.type == .ssh, let destination = destination(for: group) else {
            projectGroupStore.selectGroup(id: group.id)
            selectFirstProject()
            isShowingPopover = false
            return
        }
        isShowingPopover = false
        Task {
            let connected = await sshConnections.connect(destination: destination)
            guard connected else {
                ToastState.shared.show("Could not connect to \(group.name): \(failureMessage(for: destination))")
                return
            }
            projectGroupStore.selectGroup(id: group.id)
            selectFirstProject()
        }
    }

    private func selectFirstProject() {
        WorkspaceSelectionService.selectFirstProject(
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
    }

    private func failureMessage(for destination: SSHDestination) -> String {
        if case let .failed(message) = sshConnections.state(for: destination) { return message }
        return "Connection failed."
    }
}

enum RemoteWorkspaceEditorMode: Identifiable {
    case create
    case edit(ProjectGroup)

    var id: String {
        switch self {
        case .create: "remote-create"
        case let .edit(group): "remote-edit-\(group.id.uuidString)"
        }
    }

    var title: String {
        switch self {
        case .create: "Remote Workspace"
        case .edit: "Edit Remote Workspace"
        }
    }

    var actionLabel: String {
        switch self {
        case .create: "Create"
        case .edit: "Save"
        }
    }

    var initialName: String {
        switch self {
        case .create: ""
        case let .edit(group): group.name
        }
    }

    var initialDeviceID: UUID? {
        switch self {
        case .create: nil
        case let .edit(group): group.remoteDeviceID
        }
    }
}

enum WorkspaceEditorMode: Identifiable {
    case create
    case rename(ProjectGroup)

    var id: String {
        switch self {
        case .create: "create"
        case let .rename(group): "rename-\(group.id.uuidString)"
        }
    }

    var title: String {
        switch self {
        case .create: "New Workspace"
        case .rename: "Rename Workspace"
        }
    }

    var actionLabel: String {
        switch self {
        case .create: "Create"
        case .rename: "Rename"
        }
    }

    var initialName: String {
        switch self {
        case .create: ""
        case let .rename(group): group.name
        }
    }
}

struct ProjectGroupMembershipMenu: View {
    let project: Project

    @Environment(ProjectGroupStore.self) private var projectGroupStore

    private var localGroups: [ProjectGroup] {
        projectGroupStore.groups.filter { $0.type == .local }
    }

    var body: some View {
        if !project.isRemote, !localGroups.isEmpty {
            Menu("Move to Workspace") {
                ForEach(localGroups) { group in
                    let isInGroup = group.projectIDs.contains(project.id)
                    Button {
                        if isInGroup {
                            projectGroupStore.removeProject(projectID: project.id, fromGroup: group.id)
                        } else {
                            projectGroupStore.addProject(projectID: project.id, toGroup: group.id)
                        }
                    } label: {
                        Label(group.name, systemImage: isInGroup ? "checkmark" : "")
                    }
                }
            }
        }
    }
}

private struct WorkspaceRow: View {
    let group: ProjectGroup
    let isActive: Bool
    let connectionState: SSHConnectionState?
    let onSelect: () -> Void
    let onRename: () -> Void
    let onDelete: () -> Void

    @State private var isHovered = false

    var body: some View {
        Button(action: onSelect) {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: isActive ? "checkmark" : "")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.accent)
                    .frame(width: UIMetrics.fontCaption)
                Image(systemName: group.type == .ssh ? "network" : "square.stack.3d.up")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: UIMetrics.fontBody)
                Text(group.name)
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                Spacer()
                connectionIndicator
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .padding(.vertical, UIMetrics.spacing2)
            .background(isHovered ? MuxyTheme.hover : Color.clear, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
        .help(connectionHelp)
        .contextMenu {
            if connectionState != nil {
                Button("Edit Connection", action: onRename)
            } else {
                Button("Rename", action: onRename)
            }
            Button("Delete", role: .destructive, action: onDelete)
        }
    }

    @ViewBuilder
    private var connectionIndicator: some View {
        switch connectionState {
        case .testing,
             .connecting:
            ProgressView()
                .controlSize(.mini)
        case .connected:
            Circle().fill(.green).frame(width: UIMetrics.spacing2, height: UIMetrics.spacing2)
        case .failed:
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(.orange)
        case .disconnected,
             .none:
            EmptyView()
        }
    }

    private var connectionHelp: String {
        guard case let .failed(message) = connectionState else { return "" }
        return message
    }
}

private struct RemoteWorkspaceEditorSheet: View {
    let mode: RemoteWorkspaceEditorMode
    let onSubmit: (_ name: String, _ deviceID: UUID) -> Void
    let onCancel: () -> Void

    @Environment(RemoteDeviceStore.self) private var deviceStore

    @State private var name: String = ""
    @State private var selectedDeviceID: UUID?
    @State private var deviceEditor: RemoteDeviceEditorMode?
    @FocusState private var nameFocused: Bool

    private var devices: [RemoteDevice] { deviceStore.sshDevices() }

    private var selectedDevice: RemoteDevice? {
        deviceStore.device(id: selectedDeviceID)
    }

    private var trimmedName: String { name.trimmingCharacters(in: .whitespaces) }

    private var displayName: String {
        trimmedName.isEmpty ? (selectedDevice?.displayName ?? "") : trimmedName
    }

    private var canSubmit: Bool {
        selectedDeviceID != nil && !displayName.isEmpty
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text(mode.title)
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            if devices.isEmpty {
                emptyState
            } else {
                devicePicker
                nameField
            }

            HStack(spacing: UIMetrics.spacing3) {
                Spacer()
                Button("Cancel", action: onCancel)
                    .keyboardShortcut(.cancelAction)
                Button(mode.actionLabel, action: submit)
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canSubmit)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(420))
        .sheet(item: $deviceEditor) { editorMode in
            RemoteDeviceEditorSheet(
                mode: editorMode,
                onSave: { deviceName, ssh in
                    let device = deviceStore.add(name: deviceName, ssh: ssh)
                    selectedDeviceID = device.id
                    deviceEditor = nil
                },
                onCancel: { deviceEditor = nil }
            )
        }
        .onAppear {
            name = mode.initialName
            selectedDeviceID = mode.initialDeviceID ?? devices.first?.id
            nameFocused = !devices.isEmpty
        }
    }

    private var emptyState: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
            Text("Add a remote device to connect this workspace to a server.")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
            Button {
                deviceEditor = .create
            } label: {
                Label("Add Remote Device", systemImage: "plus")
            }
        }
    }

    private var devicePicker: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            Text("Device")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
            HStack(spacing: UIMetrics.spacing3) {
                Picker("", selection: $selectedDeviceID) {
                    ForEach(devices) { device in
                        Text(device.displayName).tag(Optional(device.id))
                    }
                }
                .labelsHidden()
                Button {
                    deviceEditor = .create
                } label: {
                    Image(systemName: "plus")
                }
                .help("Add Remote Device")
            }
        }
    }

    private var nameField: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            Text("Name")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
            TextField(selectedDevice?.displayName ?? "Production", text: $name)
                .textFieldStyle(.roundedBorder)
                .focused($nameFocused)
                .onSubmit { if canSubmit { submit() } }
        }
    }

    private func submit() {
        guard let deviceID = selectedDeviceID, canSubmit else { return }
        onSubmit(displayName, deviceID)
    }
}

private struct WorkspaceEditorSheet: View {
    let mode: WorkspaceEditorMode
    let onSubmit: (String) -> Void
    let onCancel: () -> Void

    @State private var name: String = ""
    @FocusState private var nameFocused: Bool

    private var trimmed: String {
        name.trimmingCharacters(in: .whitespaces)
    }

    private var canSubmit: Bool {
        !trimmed.isEmpty
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text(mode.title)
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                Text("Workspace Name")
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgMuted)
                TextField("Personal", text: $name)
                    .textFieldStyle(.roundedBorder)
                    .focused($nameFocused)
                    .onSubmit { if canSubmit { onSubmit(trimmed) } }
            }

            HStack {
                Spacer()
                Button("Cancel", action: onCancel)
                    .keyboardShortcut(.cancelAction)
                Button(mode.actionLabel) { onSubmit(trimmed) }
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canSubmit)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(360))
        .onAppear {
            name = mode.initialName
            nameFocused = true
        }
    }
}
