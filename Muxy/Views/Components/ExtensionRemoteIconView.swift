import AppKit
import SwiftUI

struct ExtensionRemoteIconView: View {
    let urlString: String?
    var placeholderSize: CGFloat = 20

    @State private var image: NSImage?

    var body: some View {
        Group {
            if let image {
                Image(nsImage: image)
                    .resizable()
                    .interpolation(.high)
                    .scaledToFit()
                    .padding(6)
            } else {
                placeholder
            }
        }
        .task(id: urlString) { await load() }
    }

    private var placeholder: some View {
        Image(systemName: "puzzlepiece.extension")
            .font(.system(size: placeholderSize))
            .foregroundStyle(MuxyTheme.fgDim)
    }

    private func load() async {
        guard let urlString, let url = URL(string: urlString) else {
            image = nil
            return
        }
        let loaded = await ExtensionRemoteIconCache.shared.image(for: url)
        guard !Task.isCancelled else { return }
        image = loaded
    }
}

@MainActor
final class ExtensionRemoteIconCache {
    static let shared = ExtensionRemoteIconCache()

    nonisolated private static let maximumIconBytes = 2 * 1024 * 1024
    nonisolated private static let totalCostLimit = 32 * 1024 * 1024

    private let cache: NSCache<NSURL, NSImage> = {
        let cache = NSCache<NSURL, NSImage>()
        cache.totalCostLimit = ExtensionRemoteIconCache.totalCostLimit
        return cache
    }()

    private var failed: Set<URL> = []
    private var inFlight: [URL: Task<Data?, Never>] = [:]
    private let session: URLSession

    init(session: URLSession = .shared) {
        self.session = session
    }

    func image(for url: URL) async -> NSImage? {
        if let cached = cache.object(forKey: url as NSURL) {
            return cached
        }
        if failed.contains(url) {
            return nil
        }

        let data = await fetchData(for: url)
        if let cached = cache.object(forKey: url as NSURL) {
            return cached
        }
        guard let data, let image = NSImage(data: data) else {
            failed.insert(url)
            return nil
        }
        cache.setObject(image, forKey: url as NSURL, cost: data.count)
        return image
    }

    private func fetchData(for url: URL) async -> Data? {
        if let existing = inFlight[url] {
            return await existing.value
        }
        let task = Task { [session] in
            await Self.download(url: url, session: session)
        }
        inFlight[url] = task
        let data = await task.value
        inFlight[url] = nil
        return data
    }

    nonisolated private static func download(url: URL, session: URLSession) async -> Data? {
        var request = URLRequest(url: url)
        request.setValue("image/*", forHTTPHeaderField: "Accept")
        guard let (data, response) = try? await session.data(for: request) else { return nil }
        if let http = response as? HTTPURLResponse, !(200 ..< 300).contains(http.statusCode) {
            return nil
        }
        guard data.count <= maximumIconBytes else { return nil }
        return data
    }
}
