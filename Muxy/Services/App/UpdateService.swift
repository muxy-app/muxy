import Combine
import Foundation
import os
import Sparkle

private let logger = Logger(subsystem: "app.muxy", category: "UpdateService")

enum UpdateChannel: String, CaseIterable, Identifiable {
    case stable
    case beta

    static let storageKey = "muxy.update.channel"

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .stable: "Stable"
        case .beta: "Beta"
        }
    }

    var feedURL: String {
        switch self {
        case .stable:
            "https://github.com/muxy-app/muxy/releases/latest/download/appcast-\(Self.archSlug).xml"
        case .beta:
            "https://github.com/muxy-app/muxy/releases/download/beta-channel/appcast-beta-\(Self.archSlug).xml"
        }
    }

    private static var archSlug: String {
        #if arch(arm64)
        "arm64"
        #else
        "x86_64"
        #endif
    }
}

@MainActor @Observable
final class UpdateService: NSObject {
    static let shared = UpdateService()
    static let automaticallyUpdatesKey = "SUAutomaticallyUpdate"

    @ObservationIgnored private let controller: SPUStandardUpdaterController
    @ObservationIgnored private var cancellables = Set<AnyCancellable>()
    @ObservationIgnored private let feedDelegate: FeedDelegate

    private(set) var canCheckForUpdates = false
    private(set) var allowsAutomaticUpdates = false
    private(set) var automaticallyDownloadsUpdates = false
    private(set) var availableUpdateVersion: String?

    var channel: UpdateChannel {
        get { feedDelegate.channel }
        set {
            guard newValue != feedDelegate.channel else { return }
            feedDelegate.channel = newValue
            UserDefaults.standard.set(newValue.rawValue, forKey: UpdateChannel.storageKey)
            availableUpdateVersion = nil
            updater.checkForUpdatesInBackground()
        }
    }

    private var updater: SPUUpdater {
        controller.updater
    }

    override private init() {
        let stored = UserDefaults.standard.string(forKey: UpdateChannel.storageKey)
            .flatMap { UpdateChannel(rawValue: $0) } ?? .stable
        let delegate = FeedDelegate(channel: stored)
        feedDelegate = delegate
        controller = SPUStandardUpdaterController(
            startingUpdater: false,
            updaterDelegate: delegate,
            userDriverDelegate: nil
        )
        super.init()
        controller.updater.publisher(for: \.canCheckForUpdates)
            .assign(to: \.canCheckForUpdates, on: self)
            .store(in: &cancellables)
        controller.updater.publisher(for: \.allowsAutomaticUpdates)
            .assign(to: \.allowsAutomaticUpdates, on: self)
            .store(in: &cancellables)
        controller.updater.publisher(for: \.automaticallyDownloadsUpdates)
            .assign(to: \.automaticallyDownloadsUpdates, on: self)
            .store(in: &cancellables)
        observeUpdateNotifications()
        applyFeatureFlags()
    }

    func start() {
        do {
            try updater.start()
        } catch {
            logger.warning("Sparkle updater failed to start: \(error.localizedDescription)")
        }
    }

    func checkForUpdates() {
        controller.checkForUpdates(nil)
    }

    func setAutomaticallyDownloadsUpdates(_ enabled: Bool) {
        guard enabled != automaticallyDownloadsUpdates else { return }
        updater.automaticallyDownloadsUpdates = enabled
    }

    func resetAutomaticallyDownloadsUpdates() {
        UserDefaults.standard.removeObject(forKey: Self.automaticallyUpdatesKey)
        automaticallyDownloadsUpdates = updater.automaticallyDownloadsUpdates
    }

    private func applyFeatureFlags() {
        #if DEBUG
        if ProcessInfo.processInfo.environment["FF_UPDATE_AVAILABLE"] != nil {
            availableUpdateVersion = "0.0.0-dev"
        }
        #endif
    }

    private func observeUpdateNotifications() {
        NotificationCenter.default.publisher(for: .SUUpdaterDidFindValidUpdate)
            .compactMap { $0.userInfo?[SUUpdaterAppcastItemNotificationKey] as? SUAppcastItem }
            .receive(on: DispatchQueue.main)
            .sink { [weak self] item in
                self?.availableUpdateVersion = item.displayVersionString
            }
            .store(in: &cancellables)

        NotificationCenter.default.publisher(for: .SUUpdaterDidNotFindUpdate)
            .receive(on: DispatchQueue.main)
            .sink { [weak self] _ in
                self?.availableUpdateVersion = nil
            }
            .store(in: &cancellables)
    }
}

private final class FeedDelegate: NSObject, SPUUpdaterDelegate {
    private static let noUpdateErrorCode = Int(SUError.noUpdateError.rawValue)

    var channel: UpdateChannel

    init(channel: UpdateChannel) {
        self.channel = channel
        super.init()
    }

    func feedURLString(for _: SPUUpdater) -> String? {
        channel.feedURL
    }

    func allowedChannels(for _: SPUUpdater) -> Set<String> {
        switch channel {
        case .stable: []
        case .beta: [channel.rawValue]
        }
    }

    func updater(_: SPUUpdater, didDownloadUpdate item: SUAppcastItem) {
        logger.info("Downloaded update \(item.displayVersionString, privacy: .public)")
    }

    func updater(_: SPUUpdater, failedToDownloadUpdate item: SUAppcastItem, error: Error) {
        logger
            .error(
                "Failed to download update \(item.displayVersionString, privacy: .public): \(error.localizedDescription, privacy: .public)"
            )
    }

    func updater(_: SPUUpdater, willInstallUpdate item: SUAppcastItem) {
        logger.info("Installing update \(item.displayVersionString, privacy: .public)")
    }

    func updater(_: SPUUpdater, willInstallUpdateOnQuit item: SUAppcastItem, immediateInstallationBlock _: @escaping () -> Void) -> Bool {
        logger.info("Scheduling update \(item.displayVersionString, privacy: .public) for installation on quit")
        return false
    }

    func updater(_: SPUUpdater, didAbortWithError error: Error) {
        let error = error as NSError
        guard error.domain != SUSparkleErrorDomain || error.code != Self.noUpdateErrorCode else { return }
        logger.error("Update cycle aborted: \(error.localizedDescription, privacy: .public)")
    }
}
