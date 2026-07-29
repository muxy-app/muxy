enum ProjectNavigationOrder {
    static func projects(homeProject: Project?, displayedProjects: [Project]) -> [Project] {
        guard let homeProject else { return displayedProjects }
        return [homeProject] + displayedProjects
    }

    static func shortcutIndices(in projects: [Project]) -> [Project.ID: Int] {
        var indices: [Project.ID: Int] = [:]
        for (index, project) in projects.prefix(9).enumerated() {
            indices[project.id] = index + 1
        }
        return indices
    }
}
