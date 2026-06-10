import AppKit
import SwiftUI

struct WorkspaceSwitcher: View {
    let isWide: Bool

    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @Environment(SSHConnectionService.self) private var sshConnections

    @State private var isShowingPopover = false
    @State private var isTriggerHovered = false
    @State private var editorMode: WorkspaceEditorMode?
    @State private var sshEditor: SSHWorkspaceEditorMode?
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
        .sheet(item: $sshEditor) { mode in
            SSHWorkspaceEditorSheet(
                mode: mode,
                onConnected: { name, data in
                    applySSH(mode: mode, name: name, data: data)
                    sshEditor = nil
                },
                onCancel: { sshEditor = nil }
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
            .padding(.vertical, UIMetrics.spacing3)
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
                            sshEditor = .edit(group)
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
                sshEditor = .create
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

    private func applySSH(mode: SSHWorkspaceEditorMode, name: String, data: SSHWorkspaceData) {
        switch mode {
        case .create:
            let group = projectGroupStore.addSSHWorkspace(name: name, data: data)
            projectGroupStore.selectGroup(id: group.id)
        case let .edit(group):
            projectGroupStore.renameGroup(id: group.id, to: name)
            projectGroupStore.updateSSHWorkspace(id: group.id, data: data)
        }
    }

    private func connectionState(for group: ProjectGroup) -> SSHConnectionState {
        guard let destination = group.sshData?.destination else { return .disconnected }
        return sshConnections.state(for: destination)
    }

    private func select(_ group: ProjectGroup) {
        guard group.type == .ssh, let destination = group.sshData?.destination else {
            projectGroupStore.selectGroup(id: group.id)
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
        }
    }

    private func failureMessage(for destination: SSHDestination) -> String {
        if case let .failed(message) = sshConnections.state(for: destination) { return message }
        return "Connection failed."
    }
}

enum SSHWorkspaceEditorMode: Identifiable {
    case create
    case edit(ProjectGroup)

    var id: String {
        switch self {
        case .create: "ssh-create"
        case let .edit(group): "ssh-edit-\(group.id.uuidString)"
        }
    }

    var title: String {
        switch self {
        case .create: "Remote Workspace"
        case .edit: "Edit Remote Workspace"
        }
    }

    var initialName: String {
        switch self {
        case .create: ""
        case let .edit(group): group.name
        }
    }

    var initialData: SSHWorkspaceData {
        switch self {
        case .create: SSHWorkspaceData(host: "")
        case let .edit(group): group.sshData ?? SSHWorkspaceData(host: "")
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

private struct SSHWorkspaceEditorSheet: View {
    let mode: SSHWorkspaceEditorMode
    let onConnected: (_ name: String, _ data: SSHWorkspaceData) -> Void
    let onCancel: () -> Void

    @Environment(SSHConnectionService.self) private var sshConnections

    @State private var name: String = ""
    @State private var host: String = ""
    @State private var root: String = ""
    @State private var port: String = ""
    @State private var user: String = ""
    @State private var identityFile: String = ""
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

    private var canProbe: Bool {
        SSHDestination.isValidHost(trimmedHost) && probeState != .testing
    }

    private var canSave: Bool {
        SSHDestination.isValidHost(trimmedHost) && !displayName.isEmpty
    }

    private var displayName: String {
        trimmedName.isEmpty ? trimmedHost : trimmedName
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text(mode.title)
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            field(label: "Name", placeholder: trimmedHost.isEmpty ? "Production" : trimmedHost, text: $name)
            field(label: "SSH Host", placeholder: "host or ~/.ssh/config alias", text: $host, focused: true)
                .onChange(of: host) { probeState = .idle }
            field(label: "Remote Root", placeholder: "~", text: $root)

            advancedSection

            statusRow

            HStack(spacing: UIMetrics.spacing3) {
                Button("Test Connection", action: runTest)
                    .disabled(!canProbe)
                Spacer()
                Button("Cancel", action: onCancel)
                    .keyboardShortcut(.cancelAction)
                Button(saveLabel, action: connect)
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canSave || probeState == .testing)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(440))
        .onAppear {
            let data = mode.initialData
            name = mode.initialName
            host = data.host
            root = data.remoteRoot
            port = data.port.map(String.init) ?? ""
            user = data.user ?? ""
            identityFile = data.identityFile ?? ""
            showAdvanced = data.port != nil || data.user != nil || data.identityFile != nil
            hostFocused = true
        }
    }

    private var advancedSection: some View {
        DisclosureGroup(isExpanded: $showAdvanced) {
            VStack(alignment: .leading, spacing: UIMetrics.scaled(10)) {
                HStack(spacing: UIMetrics.spacing4) {
                    field(label: "User", placeholder: "optional", text: $user)
                        .onChange(of: user) { probeState = .idle }
                    field(label: "Port", placeholder: "22", text: $port)
                        .onChange(of: port) { probeState = .idle }
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
            }
            .padding(.top, UIMetrics.spacing3)
        } label: {
            Text("Advanced")
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
    }

    private var saveLabel: String {
        probeState == .succeeded ? "Connect & Save" : "Connect"
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

    private var workspaceData: SSHWorkspaceData {
        SSHWorkspaceData(
            host: trimmedHost,
            remoteRoot: trimmedRoot,
            port: Int(port.trimmingCharacters(in: .whitespaces)),
            user: user,
            identityFile: identityFile
        )
    }

    private func runTest() {
        probeState = .testing
        let destination = workspaceData.destination
        Task {
            let success = await sshConnections.test(destination: destination)
            if success {
                probeState = .succeeded
            } else {
                probeState = .failed(failureMessage(for: destination))
            }
        }
    }

    private func connect() {
        guard canSave else { return }
        probeState = .testing
        let data = workspaceData
        Task {
            let success = await sshConnections.connect(destination: data.destination)
            guard success else {
                probeState = .failed(failureMessage(for: data.destination))
                return
            }
            onConnected(displayName, data)
        }
    }

    private func failureMessage(for destination: SSHDestination) -> String {
        if case let .failed(message) = sshConnections.state(for: destination) { return message }
        return "Connection failed."
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
