import MuxyShared
import SwiftUI

@main
struct MuxyMobileApp: App {
    @State private var connectionManager = ConnectionManager()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(connectionManager)
        }
    }
}
