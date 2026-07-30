import Combine
import Foundation
import os
import Sparkle

private let logger = Logger(subsystem: "app.muxy", category: "UpdateService")

enum UpdateSessionRestoration {
    static let storageKey = "muxy.update.pending-session-restoration"

    private struct State {
        let sourceBuild: String
        let targetBuild: String
        var armed: Bool

        init(sourceBuild: String, targetBuild: String, armed: Bool) {
            self.sourceBuild = sourceBuild
            self.targetBuild = targetBuild
            self.armed = armed
        }

        init?(_ dictionary: [String: Any]?) {
            guard let sourceBuild = dictionary?["sourceBuild"] as? String,
                  let targetBuild = dictionary?["targetBuild"] as? String,
                  let armed = dictionary?["armed"] as? Bool,
                  !sourceBuild.isEmpty,
                  !targetBuild.isEmpty
            else { return nil }
            self.sourceBuild = sourceBuild
            self.targetBuild = targetBuild
            self.armed = armed
        }

        var dictionary: [String: Any] {
            ["sourceBuild": sourceBuild, "targetBuild": targetBuild, "armed": armed]
        }
    }

    static func mark(
        targetBuild: String,
        currentBuild: String? = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String,
        defaults: UserDefaults = .standard
    ) {
        guard let currentBuild, !currentBuild.isEmpty, !targetBuild.isEmpty else {
            invalidate(defaults: defaults)
            return
        }
        let existing = State(defaults.dictionary(forKey: storageKey))
        let remainsArmed = existing?.sourceBuild == currentBuild
            && existing?.targetBuild == targetBuild
            && existing?.armed == true
        let state = State(sourceBuild: currentBuild, targetBuild: targetBuild, armed: remainsArmed)
        defaults.set(state.dictionary, forKey: storageKey)
    }

    static func armForTermination(defaults: UserDefaults = .standard) {
        guard var state = State(defaults.dictionary(forKey: storageKey)) else {
            invalidate(defaults: defaults)
            return
        }
        state.armed = true
        defaults.set(state.dictionary, forKey: storageKey)
    }

    static func invalidate(defaults: UserDefaults = .standard) {
        defaults.removeObject(forKey: storageKey)
    }

    static func consumeEligibility(
        currentBuild: String? = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String,
        defaults: UserDefaults = .standard
    ) -> Bool {
        guard let currentBuild, !currentBuild.isEmpty,
              var state = State(defaults.dictionary(forKey: storageKey))
        else {
            invalidate(defaults: defaults)
            return false
        }
        guard currentBuild == state.targetBuild else {
            if currentBuild == state.sourceBuild, state.armed {
                state.armed = false
                defaults.set(state.dictionary, forKey: storageKey)
            } else if currentBuild != state.sourceBuild {
                invalidate(defaults: defaults)
            }
            return false
        }
        invalidate(defaults: defaults)
        return state.armed
    }
}

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

    @ObservationIgnored private let controller: SPUStandardUpdaterController
    @ObservationIgnored private var cancellables = Set<AnyCancellable>()
    @ObservationIgnored private let feedDelegate: FeedDelegate

    private(set) var canCheckForUpdates = false
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
    private static let noUpdateErrorCode = 1001

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
    func updater(_: SPUUpdater, willInstallUpdate item: SUAppcastItem) {
        UpdateSessionRestoration.mark(targetBuild: item.versionString)
        logger.info("Installing update \(item.displayVersionString, privacy: .public)")
    }

    func updater(_: SPUUpdater, didAbortWithError error: Error) {
        guard (error as NSError).code != Self.noUpdateErrorCode else { return }
        UpdateSessionRestoration.invalidate()
        logger.error("Update cycle aborted: \(error.localizedDescription, privacy: .public)")
    }
}
