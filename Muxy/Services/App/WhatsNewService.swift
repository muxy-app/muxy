import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "WhatsNewService")

enum WhatsNewError: Error {
    case missingVersion
    case releaseNotFound
    case emptyNotes
}

enum WhatsNewService {
    static func fetchReleaseNotes(
        version: String,
        session: URLSession = .shared
    ) async throws -> String {
        let url = releaseURL(for: version)
        var request = URLRequest(url: url, timeoutInterval: 15)
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")

        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
            logger.error("Release notes fetch failed for \(version, privacy: .public)")
            throw WhatsNewError.releaseNotFound
        }

        let release = try JSONDecoder().decode(GitHubRelease.self, from: data)
        let body = release.body?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard !body.isEmpty else { throw WhatsNewError.emptyNotes }
        return body
    }

    private static func releaseURL(for version: String) -> URL {
        let base = "https://api.github.com/repos/muxy-app/muxy/releases/tags/v"
        return URL(string: base + version) ?? URL(fileURLWithPath: "/")
    }
}

private struct GitHubRelease: Decodable {
    let body: String?
}
