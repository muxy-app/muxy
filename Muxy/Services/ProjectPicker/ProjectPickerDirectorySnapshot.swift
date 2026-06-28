import Foundation

struct ProjectPickerDirectoryItem: Equatable, Hashable {
    enum Kind: Equatable, Hashable {
        case parent
        case directory
        case directorySymlink
    }

    let name: String
    let kind: Kind

    var isParent: Bool {
        kind == .parent
    }

    var isDirectorySymlink: Bool {
        kind == .directorySymlink
    }

    static let parent = ProjectPickerDirectoryItem(
        name: ProjectPickerPathService.parentDirectoryRow,
        kind: .parent
    )

    static func directory(_ name: String) -> ProjectPickerDirectoryItem {
        ProjectPickerDirectoryItem(name: name, kind: .directory)
    }

    static func directorySymlink(_ name: String) -> ProjectPickerDirectoryItem {
        ProjectPickerDirectoryItem(name: name, kind: .directorySymlink)
    }
}

struct ProjectPickerDirectorySnapshot: Equatable {
    let rows: [ProjectPickerDirectoryItem]
    let readFailed: Bool

    init(rows: [ProjectPickerDirectoryItem], readFailed: Bool) {
        self.rows = rows
        self.readFailed = readFailed
    }

    init(rows: [String], readFailed: Bool) {
        self.rows = rows.map {
            $0 == ProjectPickerPathService.parentDirectoryRow ? .parent : .directory($0)
        }
        self.readFailed = readFailed
    }
}
