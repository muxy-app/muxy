import SwiftUI

struct AddRemoteHostSheet: View {
    @Binding var isPresented: Bool
    var editingHost: RemoteHost?

    @State private var name: String = ""
    @State private var host: String = ""
    @State private var port: String = "22"
    @State private var user: String = NSUserName()
    @State private var identityFile: String = ""
    @State private var useKeychain: Bool = false
    @State private var password: String = ""

    init(isPresented: Binding<Bool>, editingHost: RemoteHost? = nil) {
        _isPresented = isPresented
        self.editingHost = editingHost
        if let host = editingHost {
            _name = State(initialValue: host.name)
            _host = State(initialValue: host.host)
            _port = State(initialValue: String(host.port))
            _user = State(initialValue: host.user)
            _identityFile = State(initialValue: host.identityFile ?? "")
            _useKeychain = State(initialValue: host.useKeychain)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(14)) {
            Text(editingHost != nil ? "Edit Remote Host" : "Add Remote Host")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))

            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                Text("Name").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                TextField("dev-server", text: $name)
                    .textFieldStyle(.roundedBorder)
            }

            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                Text("Host").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                TextField("192.168.1.100", text: $host)
                    .textFieldStyle(.roundedBorder)
            }

            HStack(spacing: UIMetrics.spacing4) {
                VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                    Text("Port").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                    TextField("22", text: $port)
                        .textFieldStyle(.roundedBorder)
                }
                VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                    Text("User").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                    TextField("root", text: $user)
                        .textFieldStyle(.roundedBorder)
                }
            }

            VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                Text("Authentication").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                Picker("", selection: $useKeychain) {
                    Text("SSH Key").tag(false)
                    Text("Password (Keychain)").tag(true)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
            }

            if useKeychain {
                VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                    Text("Password").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                    SecureField("Enter SSH password", text: $password)
                        .textFieldStyle(.roundedBorder)
                }
            } else {
                HStack(spacing: UIMetrics.spacing3) {
                    VStack(alignment: .leading, spacing: UIMetrics.spacing3) {
                        Text("Key File Path").font(.system(size: UIMetrics.fontFootnote)).foregroundStyle(MuxyTheme.fgMuted)
                        TextField("~/.ssh/id_rsa", text: $identityFile)
                            .textFieldStyle(.roundedBorder)
                    }
                    Button("...") {
                        let panel = NSOpenPanel()
                        panel.allowsMultipleSelection = false
                        panel.canChooseDirectories = false
                        panel.canChooseFiles = true
                        panel.directoryURL = URL(
                            fileURLWithPath: NSHomeDirectory() + "/.ssh"
                        )
                        if panel.runModal() == .OK {
                            identityFile = panel.url?.path ?? identityFile
                        }
                    }
                    .frame(width: 30)
                }
            }

            HStack {
                Spacer()
                Button("Cancel") { isPresented = false }
                    .keyboardShortcut(.cancelAction)
                Button {
                    saveHost()
                    isPresented = false
                } label: {
                    Text(editingHost != nil ? "Save" : "Add")
                        .frame(minWidth: 80)
                }
                .keyboardShortcut(.defaultAction)
                .buttonStyle(.borderedProminent)
                .disabled(name.isEmpty || host.isEmpty || user.isEmpty)
            }
        }
        .padding(UIMetrics.spacing8)
        .frame(width: UIMetrics.scaled(460))
    }

    private func saveHost() {
        let portValue = UInt16(port) ?? 22
        let newHost = RemoteHost(
            id: editingHost?.id ?? UUID(),
            name: name,
            host: host,
            port: portValue,
            user: user,
            identityFile: identityFile.isEmpty ? nil : (identityFile as NSString).expandingTildeInPath,
            useKeychain: useKeychain
        )

        if useKeychain && !password.isEmpty {
            KeychainSSHHelper.storePassword(password, host: host, user: user)
        } else if !useKeychain {
            KeychainSSHHelper.deletePassword(host: host, user: user)
        }

        if editingHost != nil {
            RemoteHostStore.shared.update(newHost)
        } else {
            RemoteHostStore.shared.add(newHost)
        }
    }
}
