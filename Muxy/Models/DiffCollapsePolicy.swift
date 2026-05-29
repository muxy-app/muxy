import Foundation

enum DiffCollapsePolicy {
    private static let largeChangedLineThreshold = 1000
    private static let noisyChangedLineThreshold = 200

    private static let lockFileNames: Set<String> = [
        "Cargo.lock",
        "Gemfile.lock",
        "Package.resolved",
        "Podfile.lock",
        "composer.lock",
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
    ]

    static func shouldCollapseByDefault(_ file: GitStatusFile) -> Bool {
        file.isBinary
            || isDeleted(file)
            || changedLines(file) >= largeChangedLineThreshold
            || isNoisyPath(file.path) && changedLines(file) >= noisyChangedLineThreshold
    }

    static func shouldCollapseByDefault(_ file: GitStatusFile, isStaged: Bool) -> Bool {
        file.isBinary
            || isDeleted(file)
            || changedLines(file, isStaged: isStaged) >= largeChangedLineThreshold
            || isNoisyPath(file.path) && changedLines(file, isStaged: isStaged) >= noisyChangedLineThreshold
    }

    private static func isDeleted(_ file: GitStatusFile) -> Bool {
        file.xStatus == "D" || file.yStatus == "D"
    }

    private static func changedLines(_ file: GitStatusFile) -> Int {
        (file.additions ?? 0) + (file.deletions ?? 0)
    }

    private static func changedLines(_ file: GitStatusFile, isStaged: Bool) -> Int {
        (file.additions(isStaged: isStaged) ?? 0) + (file.deletions(isStaged: isStaged) ?? 0)
    }

    private static func isNoisyPath(_ path: String) -> Bool {
        let fileName = URL(fileURLWithPath: path).lastPathComponent
        if lockFileNames.contains(fileName) { return true }

        let lowercased = path.lowercased()
        return lowercased.contains("/__snapshots__/")
            || lowercased.contains("/snapshots/")
            || lowercased.contains("/fixtures/")
            || lowercased.contains("/generated/")
            || lowercased.contains(".generated.")
            || lowercased.hasPrefix("dist/")
            || lowercased.hasPrefix("build/")
    }
}
