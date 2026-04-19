import Foundation

enum GitCommitLogParser {
    static let fieldSeparator = "\u{1F}"
    static let recordSeparator = "\u{1E}"

    static let logFormat = [
        "%H", "%h", "%s", "%an", "%aI", "%D", "%P",
    ].joined(separator: fieldSeparator) + recordSeparator

    static func parseCommitLog(_ raw: String) -> [GitCommit] {
        let records = raw.split(separator: Character(recordSeparator), omittingEmptySubsequences: true)
        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime]

        return records.compactMap { record in
            let fields = record.trimmingCharacters(in: .whitespacesAndNewlines)
                .split(separator: Character(fieldSeparator), maxSplits: 6, omittingEmptySubsequences: false)
            guard fields.count >= 7 else { return nil }

            let hash = String(fields[0])
            let shortHash = String(fields[1])
            let subject = String(fields[2])
            let authorName = String(fields[3])
            let dateString = String(fields[4])
            let refsRaw = String(fields[5])
            let parentsRaw = String(fields[6])

            let date = dateFormatter.date(from: dateString) ?? Date.distantPast
            let refs = parseRefs(refsRaw)
            let parents = parentsRaw.split(separator: " ").map(String.init)

            return GitCommit(
                hash: hash,
                shortHash: shortHash,
                subject: subject,
                authorName: authorName,
                authorDate: date,
                refs: refs,
                parentHashes: parents
            )
        }
    }

    static func parseRefs(_ raw: String) -> [GitRef] {
        guard !raw.isEmpty else { return [] }
        return raw.split(separator: ",").compactMap { segment in
            let trimmed = segment.trimmingCharacters(in: .whitespaces)
            if trimmed == "HEAD" {
                return GitRef(name: "HEAD", kind: .head)
            }
            if trimmed.hasPrefix("HEAD -> ") {
                let branch = String(trimmed.dropFirst("HEAD -> ".count))
                    .replacingOccurrences(of: "refs/heads/", with: "")
                return GitRef(name: branch, kind: .localBranch)
            }
            if trimmed.hasPrefix("tag: ") {
                let tag = String(trimmed.dropFirst("tag: ".count))
                    .replacingOccurrences(of: "refs/tags/", with: "")
                return GitRef(name: tag, kind: .tag)
            }
            if trimmed.hasPrefix("refs/heads/") {
                let name = String(trimmed.dropFirst("refs/heads/".count))
                return GitRef(name: name, kind: .localBranch)
            }
            if trimmed.hasPrefix("refs/remotes/") {
                let name = String(trimmed.dropFirst("refs/remotes/".count))
                return GitRef(name: name, kind: .remoteBranch)
            }
            if trimmed.hasPrefix("refs/tags/") {
                let name = String(trimmed.dropFirst("refs/tags/".count))
                return GitRef(name: name, kind: .tag)
            }
            return GitRef(name: trimmed, kind: .localBranch)
        }
    }
}
