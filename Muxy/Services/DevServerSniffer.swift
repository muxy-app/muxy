import Foundation
import os

private let sniffLogger = Logger(subsystem: "app.muxy", category: "DevServerSniffer")

extension Notification.Name {
    static let devServerDetected = Notification.Name("muxy.devServerDetected")
}

enum DevServerSnifferKeys {
    static let urlKey = "url"
    static let paneIDKey = "paneID"
}

@MainActor
final class DevServerSniffer {
    static let shared = DevServerSniffer()

    static let candidatePorts: [Int] = [3000, 3001, 4000, 4173, 4200, 5000, 5173, 5174, 8000, 8080, 8888]

    private static let triggerPrefixes: [String] = [
        "npm run dev",
        "npm run start",
        "npm start",
        "pnpm dev",
        "pnpm run dev",
        "pnpm start",
        "yarn dev",
        "yarn start",
        "bun dev",
        "bun run dev",
        "next dev",
        "next start",
        "vite",
        "astro dev",
        "nuxt dev",
        "remix dev",
        "ng serve",
        "ionic serve",
        "rails s",
        "bundle exec rails s",
        "python -m http.server",
        "python3 -m http.server",
        "flask run",
        "fastapi dev",
        "uvicorn",
        "hugo server",
        "jekyll serve",
        "dotnet watch",
        "phoenix.server",
        "deno task",
    ]

    private static let globalScopeKey = UUID(uuid: (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1))

    var probeURL: @MainActor (URL, @escaping @MainActor (Bool) -> Void) -> Void = DevServerSniffer.defaultProbe
    var onDetect: ((String, UUID?) -> Void)?

    private var activeProbes: Set<UUID> = []
    private var detectedURLsByScope: [UUID: Set<String>] = [:]

    private init() {}

    func observe(command: String, paneID: UUID? = nil) {
        guard isDevServerCommand(command) else { return }
        let scope = paneID ?? Self.globalScopeKey
        detectedURLsByScope[scope] = []
        let probeID = UUID()
        activeProbes.insert(probeID)
        Task { @MainActor in
            await self.runProbes(probeID: probeID, paneID: paneID)
        }
    }

    static func isDevServerCommand(_ command: String) -> Bool {
        let trimmed = command.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !trimmed.isEmpty else { return false }
        return triggerPrefixes.contains { prefix in
            trimmed == prefix || trimmed.hasPrefix(prefix + " ")
        }
    }

    private func isDevServerCommand(_ command: String) -> Bool {
        Self.isDevServerCommand(command)
    }

    private func runProbes(probeID: UUID, paneID: UUID?) async {
        let totalAttempts = 6
        let initialDelay: UInt64 = 1_500_000_000
        let intervalDelay: UInt64 = 2_000_000_000
        let scope = paneID ?? Self.globalScopeKey

        try? await Task.sleep(nanoseconds: initialDelay)

        for _ in 0 ..< totalAttempts {
            guard activeProbes.contains(probeID) else { return }
            if let url = await probeAvailableURL(scope: scope) {
                activeProbes.remove(probeID)
                detectedURLsByScope[scope, default: []].insert(url)
                onDetect?(url, paneID)
                var userInfo: [String: Any] = [DevServerSnifferKeys.urlKey: url]
                if let paneID { userInfo[DevServerSnifferKeys.paneIDKey] = paneID }
                NotificationCenter.default.post(
                    name: .devServerDetected,
                    object: nil,
                    userInfo: userInfo
                )
                return
            }
            try? await Task.sleep(nanoseconds: intervalDelay)
        }
        activeProbes.remove(probeID)
    }

    func reset() {
        activeProbes.removeAll()
        detectedURLsByScope.removeAll()
    }

    func resetScope(paneID: UUID?) {
        let scope = paneID ?? Self.globalScopeKey
        detectedURLsByScope.removeValue(forKey: scope)
    }

    func detectedURLs(paneID: UUID?) -> Set<String> {
        let scope = paneID ?? Self.globalScopeKey
        return detectedURLsByScope[scope] ?? []
    }

    private func probeAvailableURL(scope: UUID) async -> String? {
        let alreadyDetected = detectedURLsByScope[scope] ?? []
        for port in Self.candidatePorts {
            let urlString = "http://localhost:\(port)"
            guard !alreadyDetected.contains(urlString) else { continue }
            guard let url = URL(string: urlString) else { continue }
            if await isReachable(url: url) {
                return urlString
            }
        }
        return nil
    }

    private func isReachable(url: URL) async -> Bool {
        await withCheckedContinuation { continuation in
            probeURL(url) { success in
                continuation.resume(returning: success)
            }
        }
    }

    @MainActor
    private static func defaultProbe(url: URL, completion: @escaping @MainActor (Bool) -> Void) {
        var request = URLRequest(url: url)
        request.httpMethod = "HEAD"
        request.timeoutInterval = 0.6
        let task = URLSession.shared.dataTask(with: request) { _, response, _ in
            let isReachable = (response as? HTTPURLResponse).map { $0.statusCode < 500 } ?? false
            Task { @MainActor in completion(isReachable) }
        }
        task.resume()
    }
}
