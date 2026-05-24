import Foundation

struct CommandTabRequest: Equatable {
    let projectID: UUID
    let areaID: UUID?
    let name: String
    let command: String
    let closesOnCommandExit: Bool
}
