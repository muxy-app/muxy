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
        image = nil
        guard let urlString, let url = URL(string: urlString) else { return }
        let loaded = await ExtensionRemoteIconCache.shared.image(for: url)
        guard !Task.isCancelled else { return }
        image = loaded
    }
}

actor ExtensionRemoteIconCache {
    static let shared = ExtensionRemoteIconCache()

    private static let maximumIconBytes = 2 * 1024 * 1024

    private let cache = NSCache<NSURL, NSImage>()
    private var inFlight: [URL: Task<NSImage?, Never>] = [:]
    private let session: URLSession

    init(session: URLSession = .shared) {
        self.session = session
    }

    func image(for url: URL) async -> NSImage? {
        if let cached = cache.object(forKey: url as NSURL) {
            return cached
        }
        if let existing = inFlight[url] {
            return await existing.value
        }

        let task = Task<NSImage?, Never> { [session] in
            await Self.fetch(url: url, session: session)
        }
        inFlight[url] = task
        let image = await task.value
        inFlight[url] = nil
        if let image {
            cache.setObject(image, forKey: url as NSURL)
        }
        return image
    }

    private static func fetch(url: URL, session: URLSession) async -> NSImage? {
        var request = URLRequest(url: url)
        request.setValue("image/*", forHTTPHeaderField: "Accept")
        guard let (data, response) = try? await session.data(for: request) else { return nil }
        if let http = response as? HTTPURLResponse, !(200 ..< 300).contains(http.statusCode) {
            return nil
        }
        guard data.count <= maximumIconBytes else { return nil }
        return NSImage(data: data)
    }
}
