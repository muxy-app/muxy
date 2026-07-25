import Foundation

struct AgentSessionSnapshot: Codable, Equatable {
    let providerID: String
    let sessionID: String?
    let cwd: String
}
