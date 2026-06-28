import Foundation
import MuxyShared

@MainActor
final class ClientThemeStore {
    static let shared = ClientThemeStore()

    private var themes: [UUID: ClientThemeDTO] = [:]

    private init() {}

    func setTheme(_ theme: ClientThemeDTO?, for clientID: UUID) {
        themes[clientID] = theme?.capped()
    }

    func theme(for clientID: UUID) -> ClientThemeDTO? {
        themes[clientID]
    }

    func clear(for clientID: UUID) {
        themes.removeValue(forKey: clientID)
    }
}
