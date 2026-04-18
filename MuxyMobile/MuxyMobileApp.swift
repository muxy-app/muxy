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
            if newPhase == .active {
                connectionManager.handleForeground()
            }
        }
    }
}
