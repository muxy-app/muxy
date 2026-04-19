import MuxyShared
import SwiftUI

@main
struct MuxyMobileApp: App {
    @State private var connectionManager = ConnectionManager()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(connectionManager)
        }
        .onChange(of: scenePhase) { _, newPhase in
            switch newPhase {
            case .active:
                connectionManager.handleForeground()
            case .background:
                connectionManager.handleBackground()
            case .inactive:
                break
            @unknown default:
                break
            }
        }
    }
}
