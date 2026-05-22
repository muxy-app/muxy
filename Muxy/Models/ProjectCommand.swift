import Foundation

struct ProjectCommand: Codable, Identifiable, Equatable, Hashable {
    enum Source: String, Codable, CaseIterable {
        case npm
        case composer
        case manual

        var title: String {
            switch self {
            case .npm: "npm"
            case .composer: "composer"
            case .manual: "manual"
            }
        }
    }

    let id: String
    var name: String
    var command: String
    var source: Source
}

struct ProjectCommandRunKey: Equatable, Hashable {
    let projectID: UUID
    let commandID: String
}

struct ProjectCommandRun: Equatable {
    enum State: Equatable {
        case running
        case stopped
    }

    let commandID: String
    let projectID: UUID
    let tabID: UUID
    let areaID: UUID
    let paneID: UUID
    var state: State
}
