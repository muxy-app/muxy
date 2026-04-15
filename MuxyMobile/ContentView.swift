import MuxyShared
import SwiftUI

struct ContentView: View {
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        Group {
            switch connection.state {
            case .disconnected:
                ConnectView()
            case .connecting:
                ProgressView("Connecting...")
            case .connected:
                RemoteWorkspaceView()
            case let .error(message):
                ErrorView(message: message)
            }
        }
    }
}

struct ErrorView: View {
    let message: String
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.largeTitle)
                .foregroundStyle(.secondary)
            Text(message)
                .foregroundStyle(.secondary)
            Button("Retry") {
                connection.reconnect()
            }
        }
    }
}
