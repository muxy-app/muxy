enum WorktreeLocationMode: Hashable {
    case defaultLocation
    case pathTemplate
    case parentFolder
}

struct WorktreeLocationSelection: Equatable {
    private(set) var mode: WorktreeLocationMode
    private(set) var pathTemplate: String
    private(set) var parentPath: String

    init(pathTemplate: String? = nil, parentPath: String? = nil) {
        self.pathTemplate = pathTemplate ?? ""
        self.parentPath = parentPath ?? ""
        if WorktreeLocationResolver.normalizedLocation(pathTemplate) != nil {
            mode = .pathTemplate
        } else if WorktreeLocationResolver.normalizedLocation(parentPath) != nil {
            mode = .parentFolder
        } else {
            mode = .defaultLocation
        }
    }

    var value: String {
        get {
            switch mode {
            case .defaultLocation: ""
            case .pathTemplate: pathTemplate
            case .parentFolder: parentPath
            }
        }
        set {
            switch mode {
            case .defaultLocation: break
            case .pathTemplate: pathTemplate = newValue
            case .parentFolder: parentPath = newValue
            }
        }
    }

    var selectedPathTemplate: String? {
        mode == .pathTemplate ? pathTemplate : nil
    }

    var selectedParentPath: String? {
        mode == .parentFolder ? parentPath : nil
    }

    mutating func select(_ mode: WorktreeLocationMode) {
        self.mode = mode
    }
}
