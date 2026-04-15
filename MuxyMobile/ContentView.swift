import MuxyShared
import SwiftUI

struct ContentView: View {
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        switch connection.state {
        case .disconnected:
            ConnectView()
        case .connecting:
            ProgressView("Connecting...")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color(.systemGroupedBackground))
        case .connected:
            ProjectPickerView()
        case let .error(message):
            ErrorView(message: message)
        }
    }
}

struct ErrorView: View {
    let message: String
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        ContentUnavailableView {
            Label("Connection Failed", systemImage: "wifi.exclamationmark")
        } description: {
            Text(message)
        } actions: {
            Button("Retry") {
                connection.reconnect()
            }
            .buttonStyle(.borderedProminent)
            Button("Disconnect", role: .destructive) {
                connection.disconnect()
            }
        }
    }
}
