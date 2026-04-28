import MuxyShared
import SwiftUI
import UIKit

struct ContentView: View {
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        switch connection.state {
        case .disconnected:
            ConnectView()
        case .connecting:
            ConnectingView()
        case .awaitingApproval:
            AwaitingApprovalView()
        case .connected:
            ProjectPickerView()
        case let .error(issue):
            ErrorView(issue: issue)
        }
    }
}

struct ConnectingView: View {
    @Environment(ConnectionManager.self) private var connection

    var body: some View {
        VStack(spacing: 16) {
            ProgressView()
            Text("Connecting...")
                .foregroundStyle(.secondary)
            Button("Cancel", role: .destructive) {
                connection.disconnect()
            }
            .padding(.top, 8)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(.systemGroupedBackground))
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
    let issue: ConnectionManager.ConnectionIssue
    @Environment(ConnectionManager.self) private var connection
    @State private var showingDetails = false

    var body: some View {
        ContentUnavailableView {
            Label("Connection Failed", systemImage: "wifi.exclamationmark")
        } description: {
            Text(issue.message)
        } actions: {
            HStack(spacing: 12) {
                Button("Retry") {
                    connection.reconnect()
                }
                .buttonStyle(.borderedProminent)

                Button("Debug Info") {
                    showingDetails = true
                }
                .buttonStyle(.bordered)
                .tint(.accentColor)
            }

            Button("Disconnect", role: .destructive) {
                connection.disconnect()
            }
        }
        .sheet(isPresented: $showingDetails) {
            ConnectionIssueDetailsView(issue: issue)
        }
    }
}

struct ConnectionIssueDetailsView: View {
    let issue: ConnectionManager.ConnectionIssue
    @Environment(\.dismiss) private var dismiss
    @State private var didCopy = false

    var body: some View {
        NavigationStack {
            ScrollView {
                Text(issue.technicalDetails)
                    .font(.system(.footnote, design: .monospaced))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding()
                    .textSelection(.enabled)
            }
            .background(Color(.systemGroupedBackground))
            .navigationTitle("Connection Details")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") {
                        dismiss()
                    }
                }
            }
            .safeAreaInset(edge: .bottom) {
                HStack(spacing: 12) {
                    Button(didCopy ? "Copied" : "Copy Details") {
                        UIPasteboard.general.string = issue.technicalDetails
                        didCopy = true
                    }
                    .buttonStyle(.borderedProminent)

                    ShareLink(item: issue.technicalDetails) {
                        Label("Share", systemImage: "square.and.arrow.up")
                    }
                    .buttonStyle(.bordered)
                }
                .padding()
                .frame(maxWidth: .infinity)
                .background(.ultraThinMaterial)
            }
        }
    }
}
