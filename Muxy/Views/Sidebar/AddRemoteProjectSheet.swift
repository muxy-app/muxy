import SwiftUI

struct AddRemoteProjectSheet: View {
    @Binding var isPresented: Bool
    var preselectedHostID: UUID?

    @Environment(ProjectStore.self) private var projectStore

    @State private var selectedHostID: UUID?
    @State private var projectName: String = ""
    @State private var remotePath: String = ""

    private var hosts: [RemoteHost] {
        RemoteHostStore.shared.hosts
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text("Add Remote Project")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                Text("Host").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                Picker("", selection: $selectedHostID) {
                    Text("Select host...").tag(nil as UUID?)
                    ForEach(hosts) { host in
                        Text("\(host.name) (\(host.displaySummary))")
                            .tag(host.id as UUID?)
                    }
                }
                .labelsHidden()
            }

            if let host = hosts.first(where: { $0.id == selectedHostID }) {
                HStack(spacing: UIMetrics.spacing3) {
                    Image(systemName: "server.rack")
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Text(host.displaySummary)
                        .font(.system(size: UIMetrics.fontCaption))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
            }

            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                Text("Project Name").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                TextField("my-remote-project", text: $projectName)
                    .textFieldStyle(.roundedBorder)
            }

            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                Text("Remote Path").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                TextField("/home/user/project", text: $remotePath)
                    .textFieldStyle(.roundedBorder)
            }

            HStack {
                Spacer()
                Button("Cancel") { isPresented = false }
                    .keyboardShortcut(.cancelAction)
                Button("Add") {
                    addProject()
                    isPresented = false
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
                .disabled(selectedHostID == nil || projectName.isEmpty || remotePath.isEmpty)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(460))
        .onAppear {
            if selectedHostID == nil {
                selectedHostID = preselectedHostID ?? hosts.first?.id
            }
        }
    }

    private func addProject() {
        guard let hostID = selectedHostID else { return }
        let config = RemoteProjectConfig(
            hostID: hostID,
            remotePath: remotePath,
            displayName: projectName
        )
        _ = projectStore.addRemote(name: projectName, config: config)
    }
}
