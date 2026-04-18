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
        case .awaitingApproval:
            AwaitingApprovalView()
        case .connected:
            ProjectPickerView()
        case let .error(message):
            ErrorView(message: message)
        }
    }
}

struct AwaitingApprovalView: View {
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        ContentUnavailableView {
            Label("Waiting for Approval", systemImage: "lock.shield")
        } description: {
            Text("Approve this device on your Mac to continue.")
        } actions: {
            Button("Cancel", role: .destructive) {
                connection.disconnect()
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(.systemGroupedBackground))
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
