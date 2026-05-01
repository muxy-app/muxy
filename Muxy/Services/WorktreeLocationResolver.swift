import Foundation

enum WorktreeLocationResolver {
    static func worktreeDirectory(for project: Project, slug: String) -> String {
        worktreeDirectory(
            for: project,
            slug: slug,
            defaultParentPath: UserDefaults.standard.string(forKey: GeneralSettingsKeys.defaultWorktreeParentPath)
        )
    }

    static func worktreeDirectory(for project: Project, slug: String, defaultParentPath: String?) -> String {
        parentDirectory(for: project, defaultParentPath: defaultParentPath)
            .appendingPathComponent(slug, isDirectory: true)
            .path
    }

    static func parentDirectory(for project: Project, defaultParentPath: String?) -> URL {
        if let parent = normalizedPath(project.preferredWorktreeParentPath) {
            return URL(fileURLWithPath: parent, isDirectory: true)
        }

        if let parent = normalizedPath(defaultParentPath) {
            return URL(fileURLWithPath: parent, isDirectory: true)
                .appendingPathComponent(sanitizedDirectoryName(from: project.name), isDirectory: true)
        }

        return MuxyFileStorage.worktreeRoot(forProjectID: project.id, create: false)
    }

    static func normalizedPath(_ path: String?) -> String? {
        guard let path else { return nil }
        let trimmed = path.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        return NSString(string: trimmed).expandingTildeInPath
    }

    static func sanitizedDirectoryName(from name: String) -> String {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "._-"))
        let scalars = name.unicodeScalars.map { allowed.contains($0) ? Character($0) : "-" }
        let collapsed = String(scalars)
            .split(separator: "-", omittingEmptySubsequences: true)
            .joined(separator: "-")
        return collapsed.isEmpty ? "project" : collapsed
    }
}
