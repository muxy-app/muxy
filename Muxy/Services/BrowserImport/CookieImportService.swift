import Foundation
import WebKit

@MainActor
enum CookieImportService {
    struct Result {
        let imported: Int
    }

    nonisolated static func importer(for source: BrowserImportSource) -> BrowserImporter {
        switch source {
        case .chrome: ChromeImporter()
        }
    }

    static func importCookies(
        from source: BrowserImportSource,
        profile: ImportableProfile,
        into targetProfileID: UUID
    ) async throws -> Result {
        let cookies = try await Task.detached(priority: .userInitiated) {
            try importer(for: source).importCookies(from: profile)
        }.value

        let cookieStore = BrowserDataStoreCache.shared.store(for: targetProfileID).httpCookieStore
        for cookie in cookies {
            await cookieStore.setCookie(cookie)
        }
        return Result(imported: cookies.count)
    }
}
