import CryptoKit
import Foundation

struct MarketplaceExtensionAuthor: Decodable, Equatable {
    let name: String?
    let github: String?
}

struct MarketplaceExtension: Decodable, Equatable {
    let name: String
    let description: String?
    let permissions: [String]
    let author: MarketplaceExtensionAuthor?
    let homepage: String?
    let repository: String?
    let categories: [String]
    let iconURL: String?
    let screenshotPaths: [String]
    let downloads: Int
    let currentVersion: String
    let sha256: String
    let size: Int
    let downloadURL: String

    private enum CodingKeys: String, CodingKey {
        case name
        case description
        case permissions
        case author
        case homepage
        case repository
        case categories
        case iconURL = "icon_url"
        case screenshotPaths = "screenshot_paths"
        case downloads
        case currentVersion = "current_version"
        case sha256
        case size
        case downloadURL = "download_url"
    }

    var resolvedPermissions: [ExtensionPermission] {
        permissions.compactMap(ExtensionPermission.init(rawValue:))
    }
}

enum MarketplaceError: LocalizedError, Equatable {
    case notFound
    case network(String)
    case hashMismatch
    case sizeMismatch
    case invalidArchive
    case unpackFailed(String)

    var errorDescription: String? {
        switch self {
        case .notFound:
            "This extension is no longer available in the marketplace."
        case let .network(message):
            "Could not reach the marketplace: \(message)"
        case .hashMismatch:
            "The downloaded extension failed integrity verification and was not installed."
        case .sizeMismatch:
            "The downloaded extension did not match the expected size and was not installed."
        case .invalidArchive:
            "The downloaded extension package is not a valid Muxy extension."
        case let .unpackFailed(message):
            "Could not unpack the extension: \(message)"
        }
    }
}

actor ExtensionMarketplaceService {
    static let shared = ExtensionMarketplaceService()

    private struct Envelope<T: Decodable>: Decodable {
        let data: T
    }

    private static let maximumDownloadBytes = 100 * 1024 * 1024

    private let baseURL: URL
    private let session: URLSession

    init(
        baseURL: URL = ExtensionMarketplaceService.productionBaseURL,
        session: URLSession = .shared
    ) {
        self.baseURL = baseURL
        self.session = session
    }

    private static var productionBaseURL: URL {
        var components = URLComponents()
        components.scheme = "https"
        components.host = "muxy.app"
        return components.url ?? URL(fileURLWithPath: "/")
    }

    func fetch(name: String) async throws -> MarketplaceExtension {
        let url = baseURL
            .appendingPathComponent("api")
            .appendingPathComponent("extensions")
            .appendingPathComponent(name)

        var request = URLRequest(url: url)
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        let (data, response) = try await data(for: request)
        if let http = response as? HTTPURLResponse, http.statusCode == 404 {
            throw MarketplaceError.notFound
        }
        try ensureSuccess(response)

        do {
            return try JSONDecoder().decode(Envelope<MarketplaceExtension>.self, from: data).data
        } catch {
            throw MarketplaceError.network(error.localizedDescription)
        }
    }

    func download(_ ext: MarketplaceExtension) async throws -> Data {
        guard ext.size > 0, ext.size <= Self.maximumDownloadBytes else {
            throw MarketplaceError.sizeMismatch
        }
        guard let url = URL(string: ext.downloadURL) else {
            throw MarketplaceError.invalidArchive
        }

        let (data, response) = try await bytes(for: URLRequest(url: url), limit: ext.size)
        if let http = response as? HTTPURLResponse, http.statusCode == 404 {
            throw MarketplaceError.notFound
        }
        try ensureSuccess(response)

        guard data.count == ext.size else {
            throw MarketplaceError.sizeMismatch
        }
        guard Self.sha256Hex(data) == ext.sha256.lowercased() else {
            throw MarketplaceError.hashMismatch
        }
        return data
    }

    private func data(for request: URLRequest) async throws -> (Data, URLResponse) {
        do {
            return try await session.data(for: request)
        } catch let error as MarketplaceError {
            throw error
        } catch {
            throw MarketplaceError.network(error.localizedDescription)
        }
    }

    private func bytes(for request: URLRequest, limit: Int) async throws -> (Data, URLResponse) {
        do {
            let (stream, response) = try await session.bytes(for: request)
            var data = Data()
            data.reserveCapacity(limit)
            for try await byte in stream {
                data.append(byte)
                guard data.count <= limit else {
                    throw MarketplaceError.sizeMismatch
                }
            }
            return (data, response)
        } catch let error as MarketplaceError {
            throw error
        } catch {
            throw MarketplaceError.network(error.localizedDescription)
        }
    }

    private func ensureSuccess(_ response: URLResponse) throws {
        guard let http = response as? HTTPURLResponse else { return }
        guard (200 ..< 300).contains(http.statusCode) else {
            throw MarketplaceError.network("HTTP \(http.statusCode)")
        }
    }

    private static func sha256Hex(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }
}
