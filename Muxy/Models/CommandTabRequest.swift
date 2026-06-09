import Foundation

struct CommandTabRequest: Equatable {
    let projectID: UUID
    let areaID: UUID?
    let name: String
    let command: String
    let closesOnCommandExit: Bool
    let directory: String?

    init(
        projectID: UUID,
        areaID: UUID?,
        name: String,
        command: String,
        closesOnCommandExit: Bool,
        directory: String? = nil
    ) {
        self.projectID = projectID
        self.areaID = areaID
        self.name = name
        self.command = command
        self.closesOnCommandExit = closesOnCommandExit
        self.directory = directory
    }
}
