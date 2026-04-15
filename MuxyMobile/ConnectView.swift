import SwiftUI

struct ConnectView: View {
    @Environment(ConnectionManager.self) private var connection
    @State private var host = ""
    @State private var port = "4865"

    var body: some View {
        NavigationStack {
            VStack(spacing: 32) {
                Spacer()

                VStack(spacing: 8) {
                    Image(systemName: "terminal")
                        .font(.system(size: 48))
                        .foregroundStyle(.tint)
                    Text("Muxy")
                        .font(.largeTitle.bold())
                    Text("Connect to your Mac")
                        .foregroundStyle(.secondary)
                }

                VStack(spacing: 12) {
                    TextField("Host (e.g. 192.168.1.10)", text: $host)
                        .textFieldStyle(.roundedBorder)
                        .textContentType(.URL)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)

                    TextField("Port", text: $port)
                        .textFieldStyle(.roundedBorder)
                        .keyboardType(.numberPad)
                }
                .padding(.horizontal, 32)

                Button {
                    let portNumber = UInt16(port) ?? 4865
                    connection.connect(host: host, port: portNumber)
                } label: {
                    Text("Connect")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .padding(.horizontal, 32)
                .disabled(host.isEmpty)

                Spacer()
                Spacer()
            }
            .navigationTitle("")
        }
    }
}
