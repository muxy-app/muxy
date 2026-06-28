import Foundation

enum DroppedPathsParser {
    static func parse(
        fileURLs: [URL],
        plainString: String?,
        fileExists: (String) -> Bool = { FileManager.default.fileExists(atPath: $0) }
    ) -> [String] {
        let urlPaths = fileURLs.compactMap { $0.isFileURL ? $0.path : nil }
        if !urlPaths.isEmpty { return urlPaths }

        guard let plainString else { return [] }

        let candidates = plainString
            .split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }

        guard !candidates.isEmpty else { return [] }

        var paths: [String] = []
        for candidate in candidates {
            if candidate.hasPrefix("file://"), let url = URL(string: candidate), url.isFileURL {
                paths.append(url.path)
                continue
            }
            if candidate.hasPrefix("/"), fileExists(candidate) {
                paths.append(candidate)
                continue
            }
            return []
        }
        return paths
    }
}
