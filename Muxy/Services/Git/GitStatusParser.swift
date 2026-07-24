import Foundation

enum GitStatusParser {
    static func parseRepositorySummary(_ output: String) -> GitRepositorySummary? {
        var branch: String?
        var headOID: String?
        var hasUpstream = false
        var ahead = 0
        var behind = 0
        var changedCount = 0
        var stagedCount = 0
        var unstagedCount = 0
        var untrackedCount = 0

        for line in output.split(separator: "\n", omittingEmptySubsequences: true) {
            if line.hasPrefix("# branch.head ") {
                branch = String(line.dropFirst("# branch.head ".count))
                continue
            }
            if line.hasPrefix("# branch.oid ") {
                headOID = String(line.dropFirst("# branch.oid ".count))
                continue
            }
            if line.hasPrefix("# branch.upstream ") {
                hasUpstream = true
                continue
            }
            if line.hasPrefix("# branch.ab ") {
                let counts = line.dropFirst("# branch.ab ".count).split(separator: " ")
                ahead = counts.first.flatMap { Int($0.dropFirst()) } ?? 0
                behind = counts.dropFirst().first.flatMap { Int($0.dropFirst()) } ?? 0
                continue
            }
            if line.hasPrefix("? ") {
                changedCount += 1
                untrackedCount += 1
                continue
            }
            if line.hasPrefix("u ") {
                changedCount += 1
                stagedCount += 1
                unstagedCount += 1
                continue
            }
            guard line.hasPrefix("1 ") || line.hasPrefix("2 ") else { continue }
            let fields = line.split(separator: " ", maxSplits: 2, omittingEmptySubsequences: true)
            guard fields.count >= 2, fields[1].count == 2 else { continue }
            let status = Array(fields[1])
            changedCount += 1
            if status[0] != "." {
                stagedCount += 1
            }
            if status[1] != "." {
                unstagedCount += 1
            }
        }

        guard let branch else { return nil }
        return GitRepositorySummary(
            branch: branch,
            headOID: headOID,
            isDetached: branch == "(detached)",
            aheadBehind: GitRepositoryService.AheadBehind(
                ahead: ahead,
                behind: behind,
                hasUpstream: hasUpstream
            ),
            changedCount: changedCount,
            stagedCount: stagedCount,
            unstagedCount: unstagedCount,
            untrackedCount: untrackedCount
        )
    }

    static func parseStatusPorcelain(
        _ data: Data,
        stats: [String: NumstatEntry]
    ) -> [GitStatusFile] {
        guard let decoded = String(data: data, encoding: .utf8), !decoded.isEmpty else { return [] }
        let tokens = decoded.split(separator: "\0", omittingEmptySubsequences: true).map(String.init)
        var files: [GitStatusFile] = []
        var index = 0

        while index < tokens.count {
            let token = tokens[index]
            guard token.count >= 4 else {
                index += 1
                continue
            }
            let marker = Array(token)
            let xStatus = marker[0]
            let yStatus = marker[1]
            let path = String(token.dropFirst(3))

            if xStatus == "R" || xStatus == "C" || yStatus == "R" || yStatus == "C" {
                let oldPath = index + 1 < tokens.count ? tokens[index + 1] : path
                let stat = stats[path]
                files.append(GitStatusFile(
                    path: path,
                    oldPath: oldPath,
                    xStatus: xStatus,
                    yStatus: yStatus,
                    additions: stat?.additions,
                    deletions: stat?.deletions,
                    isBinary: stat?.isBinary ?? false
                ))
                index += 2
                continue
            }

            let stat = stats[path]
            files.append(GitStatusFile(
                path: path,
                oldPath: nil,
                xStatus: xStatus,
                yStatus: yStatus,
                additions: stat?.additions,
                deletions: stat?.deletions,
                isBinary: stat?.isBinary ?? false
            ))
            index += 1
        }

        return files.sorted { $0.path.localizedStandardCompare($1.path) == .orderedAscending }
    }

    static func parseNumstat(_ output: String) -> [String: NumstatEntry] {
        var stats: [String: NumstatEntry] = [:]

        for line in output.split(separator: "\n", omittingEmptySubsequences: true) {
            let fields = line.split(separator: "\t", maxSplits: 2, omittingEmptySubsequences: false)
            guard fields.count == 3 else { continue }

            let addsToken = String(fields[0])
            let delsToken = String(fields[1])
            let rawPath = String(fields[2])

            let entry = NumstatEntry(
                additions: Int(addsToken),
                deletions: Int(delsToken),
                isBinary: addsToken == "-" || delsToken == "-"
            )

            let normalizedPath = normalizeNumstatPath(rawPath)
            stats[normalizedPath] = entry
            stats[rawPath] = entry
        }

        return stats
    }

    static func normalizeNumstatPath(_ rawPath: String) -> String {
        if let braceStart = rawPath.firstIndex(of: "{"),
           let braceEnd = rawPath.lastIndex(of: "}"),
           let arrowRange = rawPath.range(of: " => ")
        {
            let prefix = rawPath[..<braceStart]
            let suffix = rawPath[rawPath.index(after: braceEnd)...]
            let right = rawPath[arrowRange.upperBound ..< braceEnd]
            return String(prefix) + String(right) + String(suffix)
        }
        if let arrowRange = rawPath.range(of: " => ") {
            return String(rawPath[arrowRange.upperBound...])
        }
        return rawPath
    }
}
