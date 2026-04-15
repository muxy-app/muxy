import SwiftUI

struct ConnectView: View {
    @Environment(ConnectionManager.self) private var connection
    @State private var host = ""
    @State private var port = "4865"
    @State private var showAddSheet = false
    @State private var showSettings = false

    var body: some View {
        NavigationStack {
            List {
                if let lastHost = connection.lastSavedHost {
                    Section {
                        Button {
                            connection.connect(host: lastHost, port: connection.lastSavedPort ?? 4865)
                        } label: {
                            HStack(spacing: 14) {
                                Image(systemName: "desktopcomputer")
                                    .font(.title3)
                                    .foregroundStyle(.tint)
                                    .frame(width: 36, height: 36)
                                    .background(.tint.opacity(0.15), in: RoundedRectangle(cornerRadius: 8))
                                VStack(alignment: .leading, spacing: 2) {
                                    Text("Mac")
                                        .font(.body.weight(.medium))
                                        .foregroundStyle(.primary)
                                    Text("\(lastHost):\(connection.lastSavedPort ?? 4865)")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                            }
                        }
                    }
                }
            }
            .navigationTitle("Servers")
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button {
                        showSettings = true
                    } label: {
                        Image(systemName: "gearshape")
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        showAddSheet = true
                    } label: {
                        Image(systemName: "plus")
                    }
                }
            }
            .overlay {
                if connection.lastSavedHost == nil {
                    ContentUnavailableView {
                        Label("No Servers", systemImage: "server.rack")
                    } description: {
                        Text("Add your Mac to get started")
                    } actions: {
                        Button("Add Server") {
                            showAddSheet = true
                        }
                        .buttonStyle(.borderedProminent)
                    }
                }
            }
            .sheet(isPresented: $showAddSheet) {
                AddServerSheet()
            }
            .sheet(isPresented: $showSettings) {
                SettingsSheet()
            }
        }
    }
}

struct AddServerSheet: View {
    @Environment(ConnectionManager.self) private var connection
    @Environment(\.dismiss) private var dismiss
    @State private var host = ""
    @State private var port = "4865"
    @FocusState private var hostFocused: Bool

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Host", text: $host, prompt: Text("192.168.1.10"))
                        .textContentType(.URL)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                        .focused($hostFocused)
                    TextField("Port", text: $port)
                        .keyboardType(.numberPad)
                } header: {
                    Text("Connection")
                }
            }
            .navigationTitle("Add Server")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Connect") {
                        let portNumber = UInt16(port) ?? 4865
                        connection.connect(host: host, port: portNumber)
                        dismiss()
                    }
                    .fontWeight(.semibold)
                    .disabled(host.isEmpty)
                }
            }
            .onAppear { hostFocused = true }
        }
        .presentationDetents([.medium])
        .presentationDragIndicator(.visible)
    }
}
