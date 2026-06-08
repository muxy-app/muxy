import Foundation

struct RemoteProjectConfig: Codable, Equatable, Hashable {
    var hostID: UUID
    var remotePath: String
    var displayName: String
    var icon: String?
    var iconColor: String?
}
