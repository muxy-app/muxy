import Foundation

enum ExtensionLogTail {
    static func read(url: URL, maxLines: Int) -> [String] {
        guard let data = try? Data(contentsOf: url),
              let text = String(data: data, encoding: .utf8)
        else { return [] }
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        let filtered = (lines.last?.isEmpty == true) ? Array(lines.dropLast()) : lines
        if filtered.count > maxLines {
            return Array(filtered.suffix(maxLines))
        }
        return filtered
    }
}
