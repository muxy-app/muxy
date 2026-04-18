import SwiftUI

struct ConnectView: View {
    @Environment(ConnectionManager.self) private var connection
    @State private var showAddSheet = false
    @State private var showSettings = false

    var body: some View {
        NavigationStack {
            List {
                ForEach(connection.savedDevices) { device in
                    Button {
                        connection.connect(host: device.host, port: device.port, name: device.name)
                    } label: {
                        HStack(spacing: 14) {
                            Image(systemName: "desktopcomputer")
                                .font(.title3)
                                .frame(width: 36, height: 36)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(device.name)
                                    .font(.body.weight(.medium))
                                Text("\(device.host):\(device.port)")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                    .foregroundStyle(.primary)
                    .swipeActions(edge: .trailing, allowsFullSwipe: true) {
                        Button(role: .destructive) {
                            connection.removeDevice(device)
                        } label: {
                            Label("Delete", systemImage: "trash")
                        }
                    }
                }
            }
            .navigationTitle("Devices")
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button {
                        showSettings = true
                    } label: {
                        Label("Settings", systemImage: "gearshape")
                            .labelStyle(.iconOnly)
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        showAddSheet = true
                    } label: {
                        Label("Add Device", systemImage: "plus")
                            .labelStyle(.iconOnly)
                    }
                }
            }
            .overlay {
                if connection.savedDevices.isEmpty {
                    ContentUnavailableView {
                        Label("No Devices", systemImage: "desktopcomputer")
                    } description: {
                        Text("Add your Mac to get started")
                    } actions: {
                        Button("Add Device") {
                            showAddSheet = true
                        }
                        .buttonStyle(.borderedProminent)
                    }
                }
            }
            .sheet(isPresented: $showAddSheet) {
                AddDeviceSheet()
            }
            .sheet(isPresented: $showSettings) {
                SettingsSheet()
            }
        }
    }
}

struct AddDeviceSheet: View {
    @Environment(ConnectionManager.self) private var connection
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var host = ""
    @State private var port = "4865"
    @FocusState private var nameFocused: Bool

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Name", text: $name, prompt: Text("My Mac"))
                        .focused($nameFocused)
                } header: {
                    Text("Device")
                }
                Section {
                    TextField("Host", text: $host, prompt: Text("192.168.1.10"))
                        .textContentType(.URL)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                    TextField("Port", text: $port)
                        .keyboardType(.numberPad)
                } header: {
                    Text("Connection")
                } footer: {
                    Text("Use the Mac's LAN IP (e.g. 192.168.1.10) or a VPN IP such as a Tailscale address (100.x.x.x).")
                }
            }
            .navigationTitle("Add Device")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Connect") {
                        let deviceName = name.isEmpty ? "Mac" : name
                        let portNumber = UInt16(port) ?? 4865
                        connection.connect(host: host, port: portNumber, name: deviceName)
                        dismiss()
                    }
                    .fontWeight(.semibold)
                    .disabled(host.isEmpty)
                }
            }
            .onAppear { nameFocused = true }
        }
        .presentationDetents([.medium])
        .presentationDragIndicator(.visible)
    }
}
