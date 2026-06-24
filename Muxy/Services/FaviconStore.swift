import AppKit
import Foundation

@MainActor
final class FaviconStore {
    static let shared = FaviconStore()

    private var cache: [String: NSImage] = [:]
    private var inFlight: Set<String> = []
    private let session: URLSession

    private init() {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 5
        session = URLSession(configuration: configuration)
    }

    nonisolated static func cacheKey(for url: URL) -> String {
        url.host ?? url.absoluteString
    }

    func favicon(for pageURL: URL) -> NSImage? {
        cache[Self.cacheKey(for: pageURL)]
    }

    func load(for pageURL: URL, iconURL: URL, completion: @escaping @MainActor (NSImage?) -> Void) {
        let key = Self.cacheKey(for: pageURL)
        if let cached = cache[key] {
            completion(cached)
            return
        }
        guard !inFlight.contains(key) else { return }
        inFlight.insert(key)
        Task { [weak self] in
            let image = await Self.fetchImage(from: iconURL, session: self?.session)
            self?.finishLoad(key: key, image: image, completion: completion)
        }
    }

    nonisolated private static func fetchImage(from iconURL: URL, session: URLSession?) async -> NSImage? {
        guard let session else { return nil }
        guard let data = try? await session.data(from: iconURL).0 else { return nil }
        return NSImage(data: data)
    }

    private func finishLoad(key: String, image: NSImage?, completion: @MainActor (NSImage?) -> Void) {
        inFlight.remove(key)
        guard let image, image.isValid else {
            completion(nil)
            return
        }
        cache[key] = image
        completion(image)
    }
}
