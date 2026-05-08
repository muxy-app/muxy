import Foundation

@MainActor
@Observable
final class PaneBranchObserver {
    typealias BranchResolver = @Sendable (String) async -> String?

    private(set) var branch: String?

    @ObservationIgnored private var repoPath: String?
    @ObservationIgnored private var refreshTask: Task<Void, Never>?
    @ObservationIgnored private var pollingTask: Task<Void, Never>?
    @ObservationIgnored private let resolver: BranchResolver
    @ObservationIgnored private let refreshInterval: TimeInterval

    init(
        refreshInterval: TimeInterval = 5,
        resolver: @escaping BranchResolver = PaneBranchObserver.defaultResolver
    ) {
        self.refreshInterval = refreshInterval
        self.resolver = resolver
    }

    deinit {
        pollingTask?.cancel()
        refreshTask?.cancel()
    }

    func update(repoPath path: String?) {
        guard repoPath != path else { return }
        repoPath = path
        guard path != nil else {
            branch = nil
            return
        }
        refresh()
    }

    func start() {
        guard pollingTask == nil else { return }
        let interval = refreshInterval
        pollingTask = Task { @MainActor [weak self] in
            while !Task.isCancelled {
                self?.refresh()
                try? await Task.sleep(for: .seconds(interval))
            }
        }
    }

    func stop() {
        pollingTask?.cancel()
        pollingTask = nil
        refreshTask?.cancel()
        refreshTask = nil
    }

    func refresh() {
        guard let path = repoPath else { return }
        refreshTask?.cancel()
        refreshTask = Task { @MainActor [weak self, resolver] in
            let resolved = await resolver(path)
            guard !Task.isCancelled, let self else { return }
            if self.branch != resolved {
                self.branch = resolved
            }
        }
    }

    static let defaultResolver: BranchResolver = { path in
        let service = GitRepositoryService()
        guard let result = try? await service.currentBranch(repoPath: path) else { return nil }
        let trimmed = result.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed != "HEAD" else { return nil }
        return trimmed
    }
}
