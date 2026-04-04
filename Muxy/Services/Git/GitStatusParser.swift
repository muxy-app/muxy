import Foundation

enum GitStatusParser {
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

            if xStatus == "R" || xStatus == "C" {
                let newPath = index + 1 < tokens.count ? tokens[index + 1] : path
                let stat = stats[newPath]
                files.append(GitStatusFile(
                    path: newPath,
                    oldPath: path,
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
