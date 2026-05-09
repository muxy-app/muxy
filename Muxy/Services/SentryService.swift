import Foundation
import os
import Sentry

private let logger = Logger(subsystem: "app.muxy", category: "Sentry")

@MainActor @Observable
final class SentryService {
    static let shared = SentryService()

    private(set) var consent: SentryConsent?
    private var started = false

    let hasDSN: Bool
    private let dsn: String?
    private let defaults: UserDefaults
    private let starter: (String) -> Void
    private let stopper: () -> Void

    var needsPrompt: Bool {
        hasDSN && consent == nil
    }

    convenience init() {
        self.init(
            dsn: Self.resolveBundledDSN(),
            defaults: .standard,
            starter: Self.defaultStarter,
            stopper: Self.defaultStopper
        )
    }

    init(
        dsn: String?,
        defaults: UserDefaults,
        starter: @escaping (String) -> Void,
        stopper: @escaping () -> Void
    ) {
        self.dsn = dsn
        hasDSN = dsn != nil
        self.defaults = defaults
        self.starter = starter
        self.stopper = stopper
        consent = Self.loadStoredConsent(from: defaults)
    }

    func start() {
        guard hasDSN, let dsn, consent == .allowed, !started else { return }
        starter(dsn)
        started = true
        logger.info("Sentry started")
    }

    func stop() {
        guard started else { return }
        stopper()
        started = false
        logger.info("Sentry stopped")
    }

    func setConsent(_ newValue: SentryConsent) {
        consent = newValue
        defaults.set(newValue.rawValue, forKey: SentryConsent.storageKey)
        switch newValue {
        case .allowed:
            start()
        case .denied:
            stop()
        }
    }

    private static func loadStoredConsent(from defaults: UserDefaults) -> SentryConsent? {
        guard let raw = defaults.string(forKey: SentryConsent.storageKey) else { return nil }
        return SentryConsent(rawValue: raw)
    }

    private static func resolveBundledDSN() -> String? {
        if let bundled = Bundle.main.object(forInfoDictionaryKey: "SentryDSN") as? String {
            let trimmed = bundled.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmed.isEmpty, trimmed != "__MUXY_SENTRY_DSN__" {
                return trimmed
            }
        }
        #if DEBUG
        return DotEnvLoader.value(for: "SENTRY_DSN")
        #else
        return nil
        #endif
    }

    private static var releaseName: String? {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
    }

    private static var environment: String {
        let channel = UserDefaults.standard.string(forKey: UpdateChannel.storageKey)
            .flatMap { UpdateChannel(rawValue: $0) } ?? .stable
        return channel == .beta ? "beta" : "production"
    }

    private static let defaultStarter: (String) -> Void = { dsn in
        SentrySDK.start { options in
            options.dsn = dsn
            options.releaseName = releaseName
            options.environment = environment
            options.sendDefaultPii = false
            options.enableAutoBreadcrumbTracking = false
            options.enableNetworkBreadcrumbs = false
            options.enableSwizzling = false
            options.attachStacktrace = true
            options.beforeSend = { event in
                event.user = nil
                event.serverName = nil
                return event
            }
        }
    }

    private static let defaultStopper: () -> Void = {
        SentrySDK.close()
    }
}
