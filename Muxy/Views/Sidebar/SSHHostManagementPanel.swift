import SwiftUI

struct SSHHostManagementPanel: View {
    @Environment(ProjectStore.self) private var projectStore
    @Environment(\.dismiss) private var dismiss
    @State private var remoteHostStore = RemoteHostStore.shared
    @State private var isAddingHost = false
    @State private var isImportingConfig = false
    @State private var editingHost: RemoteHost?
    @State private var hostToDelete: RemoteHost?

    private var remoteProjects: [Project] {
        projectStore.storedProjects.filter(\.isRemote)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text("SSH Hosts")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            if remoteHostStore.hosts.isEmpty {
                VStack(spacing: UIMetrics.spacing4) {
                    Spacer()
                    Image(systemName: "server.rack")
                        .font(.system(size: 36))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Text("No saved SSH hosts")
                        .font(.system(size: UIMetrics.fontBody))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Spacer()
                }
                .frame(maxWidth: .infinity)
            } else {
                ScrollView {
                    VStack(spacing: UIMetrics.spacing4) {
                        ForEach(remoteHostStore.hosts) { host in
                            hostCard(host)
                        }
                    }
                }
            }

            HStack {
                Button(action: { isAddingHost = true }, label: {
                    Label("Add Host", systemImage: "plus")
                })
                Button(action: { isImportingConfig = true }, label: {
                    Label("Import from SSH Config", systemImage: "doc.text")
                })
                Spacer()
                Button("Cancel") { dismiss() }
                    .keyboardShortcut(.cancelAction)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(480), height: 420)
        .sheet(isPresented: $isAddingHost) {
            AddRemoteHostSheet(isPresented: $isAddingHost)
        }
        .sheet(isPresented: Binding(
            get: { editingHost != nil },
            set: { if !$0 { editingHost = nil } }
        )) {
            if let host = editingHost {
                AddRemoteHostSheet(
                    isPresented: Binding(
                        get: { editingHost != nil },
                        set: { if !$0 { editingHost = nil } }
                    ),
                    editingHost: host
                )
            }
        }
        .sheet(isPresented: $isImportingConfig) {
            SSHConfigImportSheet(isPresented: $isImportingConfig)
        }
        .alert("Delete Host", isPresented: Binding(
            get: { hostToDelete != nil },
            set: { if !$0 { hostToDelete = nil } }
        )) {
            Button("Cancel", role: .cancel) { hostToDelete = nil }
            Button("Delete", role: .destructive) {
                if let host = hostToDelete {
                    deleteHost(host)
                    hostToDelete = nil
                }
            }
        } message: {
            if let host = hostToDelete {
                let count = projectsForHost(host.id).count
                if count >= 1 {
                    Text("This host has \(count) remote project(s). Deleting the host will not remove these projects.")
                } else {
                    Text("Are you sure you want to delete host \"\(host.name)\"?")
                }
            }
        }
    }

    private func hostCard(_ host: RemoteHost) -> some View {
        let projects = projectsForHost(host.id)
        return VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
            HStack {
                Image(systemName: "server.rack")
                    .foregroundStyle(MuxyTheme.accent)
                Text(host.name)
                    .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                Spacer()
                Text(host.displaySummary)
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }

            if !projects.isEmpty {
                HStack(spacing: UIMetrics.spacing2) {
                    Image(systemName: "folder")
                        .font(.system(size: UIMetrics.fontCaption))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Text("Projects: \(projects.map(\.name).joined(separator: ", "))")
                        .font(.system(size: UIMetrics.fontCaption))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
            }

            HStack(spacing: UIMetrics.spacing4) {
                Button("Edit") { editingHost = host }
                Button("Delete", role: .destructive) { hostToDelete = host }
                Spacer()
            }
            .buttonStyle(.borderless)
            .font(.system(size: UIMetrics.fontCaption))
        }
        .padding(UIMetrics.spacing4)
        .background(MuxyTheme.surface.opacity(0.6))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    private func projectsForHost(_ hostID: UUID) -> [Project] {
        remoteProjects.filter { $0.remoteConfig?.hostID == hostID }
    }

    private func deleteHost(_ host: RemoteHost) {
        RemoteHostStore.shared.remove(id: host.id)
    }
}
