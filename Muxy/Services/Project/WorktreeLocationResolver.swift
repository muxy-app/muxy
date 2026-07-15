import Foundation

enum WorktreeLocationResolver {
    static func worktreeDirectory(for project: Project, slug: String, branch: String) -> String {
        worktreeDirectory(
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
    ) -> String {
        guard !project.isRemote else {
            return remoteWorktreeDirectory(for: project, slug: slug)
        }

        if let template = normalizedLocation(project.preferredWorktreePathTemplate) {
            return resolve(template: template, for: project, branch: branch)
        }

        if let parent = normalizedLocation(project.preferredWorktreeParentPath) {
            return directoryURL(for: parent, relativeTo: project)
                .appendingPathComponent(slug, isDirectory: true)
                .standardizedFileURL
                .path
        }

        if let template = normalizedLocation(defaultPathTemplate) {
            return resolve(template: template, for: project, branch: branch)
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

    private static func resolve(template: String, for project: Project, branch: String) -> String {
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
