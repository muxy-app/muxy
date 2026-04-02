import Foundation
import Sparkle
import Combine
import os

private let logger = Logger(subsystem: "app.muxy", category: "UpdateService")

@MainActor @Observable
final class UpdateService: NSObject {
    static let shared = UpdateService()

    @ObservationIgnored private let controller: SPUStandardUpdaterController
    @ObservationIgnored private var cancellables = Set<AnyCancellable>()

    private(set) var canCheckForUpdates = false

    private var updater: SPUUpdater {
        controller.updater
    }

    private override init() {
        controller = SPUStandardUpdaterController(
            startingUpdater: false,
            updaterDelegate: nil,
            userDriverDelegate: nil
        )
        super.init()
        controller.updater.publisher(for: \.canCheckForUpdates)
            .assign(to: \.canCheckForUpdates, on: self)
            .store(in: &cancellables)
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
}
