import SwiftUI

struct SSHConfigImportSheet: View {
    @Binding var isPresented: Bool
    @State private var discoveredHosts: [SSHConfigParser.ParsedHost] = []
    @State private var selectedHosts: Set<String> = []
    @State private var isLoading = true

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text("Import from ~/.ssh/config")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            if isLoading {
                VStack(spacing: UIMetrics.spacing4) {
                    Spacer()
                    ProgressView()
                    Text("Parsing ~/.ssh/config...")
                        .font(.system(size: UIMetrics.fontBody))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Spacer()
                }
                .frame(maxWidth: .infinity)
            } else if discoveredHosts.isEmpty {
                VStack(spacing: UIMetrics.spacing4) {
                    Spacer()
                    Image(systemName: "doc.text.magnifyingglass")
                        .font(.system(size: 36))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Text("No hosts found")
                        .font(.system(size: UIMetrics.fontBody))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Spacer()
                }
                .frame(maxWidth: .infinity)
            } else {
                Text("Found \(discoveredHosts.count) hosts:")
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(MuxyTheme.fgMuted)

                ScrollView {
                    VStack(spacing: UIMetrics.spacing2) {
                        ForEach(discoveredHosts, id: \.name) { parsed in
                            HStack(spacing: UIMetrics.spacing3) {
                                Toggle(isOn: binding(for: parsed.name)) {
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(parsed.name)
                                            .font(.system(size: UIMetrics.fontBody, weight: .medium))
                                        Text("HostName \(parsed.hostName)  \(parsed.user.map { "User \($0)" } ?? "")")
                                            .font(.system(size: UIMetrics.fontCaption))
                                            .foregroundStyle(MuxyTheme.fgMuted)
                                    }
                                }
                            }
                            .padding(.horizontal, UIMetrics.spacing3)
                            .padding(.vertical, UIMetrics.spacing2)
                            .background(MuxyTheme.surface.opacity(0.5))
                            .clipShape(RoundedRectangle(cornerRadius: 6))
                        }
                    }
                }
            }

            HStack {
                Spacer()
                Button("Cancel") { isPresented = false }
                    .keyboardShortcut(.cancelAction)
                Button {
                    importSelected()
                    isPresented = false
                } label: {
                    Text("Import \(selectedHosts.count) Hosts")
                        .frame(minWidth: 120)
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
                .disabled(selectedHosts.isEmpty)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(460), height: 400)
        .onAppear {
            discoveredHosts = RemoteHostStore.shared.discoverSSHConfigHosts()
            selectedHosts = Set(discoveredHosts.map(\.name))
            isLoading = false
        }
    }

    private func binding(for name: String) -> Binding<Bool> {
        Binding(
            get: { selectedHosts.contains(name) },
            set: { checked in
                if checked {
                    selectedHosts.insert(name)
                } else {
                    selectedHosts.remove(name)
                }
            }
        )
    }

    private func importSelected() {
        for parsed in discoveredHosts where selectedHosts.contains(parsed.name) {
            let store = RemoteHostStore.shared
            guard !store.hosts.contains(where: { $0.host == parsed.hostName }) else { continue }
            let host = RemoteHost(
                name: parsed.name,
                host: parsed.hostName,
                port: parsed.port,
                user: parsed.user ?? NSUserName(),
                identityFile: parsed.identityFile
            )
            store.add(host)
        }
    }
}
