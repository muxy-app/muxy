import Foundation

enum WorktreeLocationError: LocalizedError, Equatable {
    case pathTemplateRequired
    case branchVariableRequired
    case branchVariableMustAffectPath
    case parentFolderRequired

    var errorDescription: String? {
        switch self {
        case .pathTemplateRequired:
            "Path template is required."
        case .branchVariableRequired:
            "Path template must include {branch}."
        case .branchVariableMustAffectPath:
            "Path template must keep {branch} in the resolved path."
        case .parentFolderRequired:
            "Folder is required."
        }
    }
}

enum WorktreeLocationResolver {
    static let suggestedPathTemplate = "../{base-dir}.{branch}"

    static func worktreeDirectory(for project: Project, slug: String, branch: String) throws -> String {
        try worktreeDirectory(
            for: project,
            slug: slug,
            branch: branch,
            defaultPathTemplate: UserDefaults.standard.string(forKey: GeneralSettingsKeys.defaultWorktreePathTemplate),
            defaultParentPath: UserDefaults.standard.string(forKey: GeneralSettingsKeys.defaultWorktreeParentPath)
        )
    }

    static func worktreeDirectory(
        for project: Project,
        slug: String,
        branch: String,
        defaultPathTemplate: String?,
        defaultParentPath: String?
    ) throws -> String {
        guard !project.isRemote else {
            return remoteWorktreeDirectory(for: project, slug: slug)
        }

        if let template = normalizedLocation(project.preferredWorktreePathTemplate) {
            return try resolve(template: template, for: project, branch: branch)
        }

        if let parent = normalizedLocation(project.preferredWorktreeParentPath) {
            return directoryURL(for: parent, relativeTo: project)
                .appendingPathComponent(slug, isDirectory: true)
                .standardizedFileURL
                .path
        }

        if let template = normalizedLocation(defaultPathTemplate) {
            return try resolve(template: template, for: project, branch: branch)
        }

        if let parent = normalizedLocation(defaultParentPath) {
            return directoryURL(for: parent, relativeTo: project)
                .appendingPathComponent(sanitizedDirectoryName(from: project.name), isDirectory: true)
                .appendingPathComponent(slug, isDirectory: true)
                .standardizedFileURL
                .path
        }

        return MuxyFileStorage.worktreeRoot(forProjectID: project.id, create: false)
            .appendingPathComponent(slug, isDirectory: true)
            .path
    }

    static func remoteWorktreeDirectory(for project: Project, slug: String) -> String {
        let path = project.path.hasSuffix("/") ? String(project.path.dropLast()) : project.path
        guard let slashIndex = path.lastIndex(of: "/") else {
            return ".muxy-worktrees/\(slug)"
        }
        let parent = String(path[..<slashIndex])
        let base = parent.isEmpty ? "" : parent
        return "\(base)/.muxy-worktrees/\(sanitizedDirectoryName(from: project.name))/\(slug)"
    }

    static func normalizedLocation(_ location: String?) -> String? {
        guard let location else { return nil }
        let trimmed = location.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        return NSString(string: trimmed).expandingTildeInPath
    }

    static func slug(from name: String) -> String {
        sanitizedPathComponent(from: name) ?? UUID().uuidString
    }

    static func sanitizedDirectoryName(from name: String) -> String {
        sanitizedPathComponent(from: name) ?? "project"
    }

    static func validatedPathTemplate(_ template: String?) throws -> String {
        guard let template = normalizedLocation(template) else {
            throw WorktreeLocationError.pathTemplateRequired
        }
        guard template.contains("{branch}") else {
            throw WorktreeLocationError.branchVariableRequired
        }

        let firstPath = validationPath(for: template, branch: "muxy-validation-a")
        let secondPath = validationPath(for: template, branch: "muxy-validation-b")
        guard firstPath != secondPath else {
            throw WorktreeLocationError.branchVariableMustAffectPath
        }
        return template
    }

    static func pathTemplateValidationMessage(_ template: String?) -> String? {
        do {
            _ = try validatedPathTemplate(template)
            return nil
        } catch {
            return error.localizedDescription
        }
    }

    private static func resolve(template: String, for project: Project, branch: String) throws -> String {
        let template = try validatedPathTemplate(template)
        let baseDirectoryName = URL(fileURLWithPath: project.path, isDirectory: true).lastPathComponent
        let replacements = [
            "{project-name}": sanitizedDirectoryName(from: project.name),
            "{base-dir}": sanitizedDirectoryName(from: baseDirectoryName),
            "{branch}": sanitizedPathComponent(from: branch) ?? "branch",
        ]
        let resolved = replacements.reduce(template) { value, replacement in
            value.replacingOccurrences(of: replacement.key, with: replacement.value)
        }
        return directoryURL(for: resolved, relativeTo: project).standardizedFileURL.path
    }

    private static func validationPath(for template: String, branch: String) -> String {
        let replacements = [
            "{project-name}": "project",
            "{base-dir}": "base",
            "{branch}": branch,
        ]
        let resolved = replacements.reduce(template) { value, replacement in
            value.replacingOccurrences(of: replacement.key, with: replacement.value)
        }
        let baseURL = URL(fileURLWithPath: "/muxy/project", isDirectory: true)
        return URL(fileURLWithPath: resolved, isDirectory: true, relativeTo: baseURL)
            .standardizedFileURL
            .path
    }

    private static func directoryURL(for location: String, relativeTo project: Project) -> URL {
        let projectURL = URL(fileURLWithPath: project.path, isDirectory: true)
        return URL(fileURLWithPath: location, isDirectory: true, relativeTo: projectURL)
    }

    private static func sanitizedPathComponent(from name: String) -> String? {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "._-"))
        let scalars = name.unicodeScalars.map { allowed.contains($0) ? Character($0) : "-" }
        let collapsed = String(scalars)
            .split(separator: "-", omittingEmptySubsequences: true)
            .joined(separator: "-")
        guard !collapsed.isEmpty, collapsed != ".", collapsed != ".." else { return nil }
        return collapsed
    }
}
