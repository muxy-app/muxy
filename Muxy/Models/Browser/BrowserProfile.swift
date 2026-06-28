import Foundation

struct BrowserProfile: Identifiable, Codable, Hashable {
    static let defaultID = UUID(uuid: (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xB0))
    static let defaultName = "Default"

    let id: UUID
    var name: String

    var isDefault: Bool { id == Self.defaultID }

    init(id: UUID = UUID(), name: String) {
        self.id = id
        self.name = name
    }

    static let `default` = Self(id: defaultID, name: defaultName)
}
